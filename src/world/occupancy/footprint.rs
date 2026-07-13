//! Shared footprint definitions and shapes (ADR-080 B3).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::BTreeSet;

use super::OccupancyError;
use super::cell::{
    OCCUPANCY_CELL_SIZE_METERS, QuantizedRotation, circle_intersects_cell,
    occupancy_cell_at_global_xz,
};
use crate::world::building::footprint::{FootprintSpec, FootprintType};
use crate::world::{BuildingDefinition, FootprintId};

/// Authoritative footprint shape geometry (no render meshes).
#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub enum FootprintShape {
    Circle {
        radius_meters: f32,
    },
    Rectangle {
        width_meters: f32,
        depth_meters: f32,
    },
    BakedCellMask(BakedCellMask),
}

/// Offline-baked horizontal occupancy mask.
#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub struct BakedCellMask {
    pub cell_size_meters: f32,
    pub width_cells: u32,
    pub depth_cells: u32,
    /// Local origin offset from building anchor in footprint-local XZ (meters).
    pub local_origin: Vec2,
    /// Blocked cells in row-major order indices: `z * width + x`.
    pub blocked_cells: BTreeSet<u32>,
    #[serde(default)]
    pub forced_open_cells: BTreeSet<u32>,
    #[serde(default)]
    pub forced_blocked_cells: BTreeSet<u32>,
    /// Future B6 seam: height band / space id placeholder.
    #[serde(default)]
    pub space_id: u32,
}

/// Catalog footprint definition.
#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub struct FootprintDefinition {
    pub id: FootprintId,
    pub shape: FootprintShape,
    pub rotation_step_degrees: u16,
    pub enabled: bool,
    /// Optional bake metadata for stale detection.
    #[serde(default)]
    pub source_asset: Option<String>,
    #[serde(default)]
    pub source_hash: Option<String>,
    #[serde(default)]
    pub bake_cell_size_meters: Option<f32>,
}

impl FootprintDefinition {
    pub fn new(id: FootprintId, shape: FootprintShape) -> Self {
        Self {
            id,
            shape,
            rotation_step_degrees: 90,
            enabled: true,
            source_asset: None,
            source_hash: None,
            bake_cell_size_meters: None,
        }
    }

    pub fn validate(&self) -> Result<(), OccupancyError> {
        if !self.enabled {
            return Err(OccupancyError::DisabledFootprint(self.id.clone()));
        }
        if self.rotation_step_degrees != 90 {
            return Err(OccupancyError::InvalidRotation {
                yaw_degrees: self.rotation_step_degrees as f32,
            });
        }
        match &self.shape {
            FootprintShape::Circle { radius_meters } => {
                if !radius_meters.is_finite() || *radius_meters < 0.0 {
                    return Err(OccupancyError::InvalidBlockingRadius {
                        radius_meters: *radius_meters,
                    });
                }
            }
            FootprintShape::Rectangle {
                width_meters,
                depth_meters,
            } => {
                if !width_meters.is_finite()
                    || !depth_meters.is_finite()
                    || *width_meters <= 0.0
                    || *depth_meters <= 0.0
                {
                    return Err(OccupancyError::InvalidMaskDimensions {
                        width_cells: 0,
                        depth_cells: 0,
                    });
                }
            }
            FootprintShape::BakedCellMask(mask) => mask.validate()?,
        }
        Ok(())
    }
}

impl BakedCellMask {
    pub fn validate(&self) -> Result<(), OccupancyError> {
        if self.width_cells == 0
            || self.depth_cells == 0
            || self.width_cells > super::cell::MAX_MASK_CELLS_PER_AXIS
            || self.depth_cells > super::cell::MAX_MASK_CELLS_PER_AXIS
        {
            return Err(OccupancyError::InvalidMaskDimensions {
                width_cells: self.width_cells,
                depth_cells: self.depth_cells,
            });
        }
        if !self.cell_size_meters.is_finite() || self.cell_size_meters <= 0.0 {
            return Err(OccupancyError::InvalidMaskDimensions {
                width_cells: self.width_cells,
                depth_cells: self.depth_cells,
            });
        }
        let max_index = self.width_cells.saturating_mul(self.depth_cells);
        for index in self
            .blocked_cells
            .iter()
            .chain(self.forced_open_cells.iter())
            .chain(self.forced_blocked_cells.iter())
        {
            if *index >= max_index {
                let (x, z) = self.cell_coords(*index);
                return Err(OccupancyError::OverrideOutOfBounds {
                    cell_x: x,
                    cell_z: z,
                });
            }
        }
        for index in self
            .forced_open_cells
            .intersection(&self.forced_blocked_cells)
        {
            let (x, z) = self.cell_coords(*index);
            return Err(OccupancyError::OverrideConflict {
                cell_x: x,
                cell_z: z,
            });
        }
        Ok(())
    }

    pub fn cell_coords(&self, index: u32) -> (i32, i32) {
        let x = (index % self.width_cells) as i32;
        let z = (index / self.width_cells) as i32;
        (x, z)
    }

    pub fn cell_index(&self, x: u32, z: u32) -> u32 {
        z * self.width_cells + x
    }

    pub fn is_blocked_local(&self, local_x: i32, local_z: i32) -> bool {
        if local_x < 0
            || local_z < 0
            || local_x >= self.width_cells as i32
            || local_z >= self.depth_cells as i32
        {
            return false;
        }
        let index = self.cell_index(local_x as u32, local_z as u32);
        if self.forced_open_cells.contains(&index) {
            return false;
        }
        if self.forced_blocked_cells.contains(&index) {
            return true;
        }
        self.blocked_cells.contains(&index)
    }

    pub fn apply_overrides(mut self) -> Self {
        for index in &self.forced_blocked_cells {
            self.blocked_cells.insert(*index);
        }
        for index in &self.forced_open_cells {
            self.blocked_cells.remove(index);
        }
        self
    }
}

/// Resolve a building definition to its footprint shape.
pub fn resolve_building_footprint<'a>(
    definition: &BuildingDefinition,
    catalog: &'a super::catalog::FootprintCatalog,
) -> Result<&'a FootprintShape, OccupancyError> {
    if let Some(footprint_id) = &definition.footprint_id {
        let footprint = catalog
            .get(footprint_id)
            .ok_or_else(|| OccupancyError::MissingFootprint(footprint_id.clone()))?;
        if !footprint.enabled {
            return Err(OccupancyError::DisabledFootprint(footprint_id.clone()));
        }
        return Ok(&footprint.shape);
    }

    match &definition.footprint {
        FootprintSpec::Circle { radius_meters } => {
            // Inline footprints are synthesized at query time via catalog helper.
            Err(OccupancyError::MissingFootprint(FootprintId::new(format!(
                "inline:{}",
                definition.id.as_str()
            ))))
        }
        FootprintSpec::Rectangle { .. } => Err(OccupancyError::MissingFootprint(FootprintId::new(
            format!("inline:{}", definition.id.as_str()),
        ))),
        FootprintSpec::MeshDerived => Err(OccupancyError::MeshDerivedRequiresFootprintId),
    }
}

/// Inline footprint shape from a building definition (when no FootprintId is set).
pub fn inline_building_footprint(
    definition: &BuildingDefinition,
) -> Result<FootprintShape, OccupancyError> {
    match &definition.footprint {
        FootprintSpec::Circle { radius_meters } => Ok(FootprintShape::Circle {
            radius_meters: *radius_meters,
        }),
        FootprintSpec::Rectangle {
            width_meters,
            depth_meters,
        } => Ok(FootprintShape::Rectangle {
            width_meters: *width_meters,
            depth_meters: *depth_meters,
        }),
        FootprintSpec::MeshDerived => Err(OccupancyError::MeshDerivedRequiresFootprintId),
    }
}

/// Effective footprint shape for a building definition.
pub fn effective_building_footprint<'a>(
    definition: &BuildingDefinition,
    catalog: &'a super::catalog::FootprintCatalog,
) -> Result<Cow<'a, FootprintShape>, OccupancyError> {
    if let Some(footprint_id) = &definition.footprint_id {
        let footprint = catalog
            .get(footprint_id)
            .ok_or_else(|| OccupancyError::MissingFootprint(footprint_id.clone()))?;
        if !footprint.enabled {
            return Err(OccupancyError::DisabledFootprint(footprint_id.clone()));
        }
        footprint.validate()?;
        return Ok(Cow::Borrowed(&footprint.shape));
    }
    Ok(Cow::Owned(inline_building_footprint(definition)?))
}

/// Enumerate global occupancy cells occupied by a footprint at a world pose.
pub fn occupied_cells_for_footprint(
    shape: &FootprintShape,
    anchor_global_xz: Vec2,
    rotation: QuantizedRotation,
) -> Vec<super::cell::OccupancyCellCoord> {
    match shape {
        FootprintShape::Circle { radius_meters } => {
            cells_for_circle(anchor_global_xz, *radius_meters)
        }
        FootprintShape::Rectangle {
            width_meters,
            depth_meters,
        } => cells_for_rectangle(anchor_global_xz, *width_meters, *depth_meters, rotation),
        FootprintShape::BakedCellMask(mask) => {
            cells_for_baked_mask(anchor_global_xz, mask, rotation)
        }
    }
}

fn cells_for_circle(center: Vec2, radius: f32) -> Vec<super::cell::OccupancyCellCoord> {
    if radius <= 0.0 {
        return Vec::new();
    }
    let size = OCCUPANCY_CELL_SIZE_METERS;
    let min_x = ((center.x - radius) / size).floor() as i32;
    let max_x = ((center.x + radius) / size).floor() as i32;
    let min_z = ((center.y - radius) / size).floor() as i32;
    let max_z = ((center.y + radius) / size).floor() as i32;
    let mut cells = Vec::new();
    for z in min_z..=max_z {
        for x in min_x..=max_x {
            let cell = super::cell::OccupancyCellCoord::new(x, z);
            if circle_intersects_cell(center, radius, cell) {
                cells.push(cell);
            }
        }
    }
    cells.sort_unstable();
    cells.dedup();
    cells
}

fn cells_for_rectangle(
    anchor: Vec2,
    width: f32,
    depth: f32,
    rotation: QuantizedRotation,
) -> Vec<super::cell::OccupancyCellCoord> {
    let half = Vec2::new(width * 0.5, depth * 0.5);
    let corners = [
        Vec2::new(-half.x, -half.y),
        Vec2::new(half.x, -half.y),
        Vec2::new(half.x, half.y),
        Vec2::new(-half.x, half.y),
    ];
    let yaw = rotation.radians();
    let (sin, cos) = yaw.sin_cos();
    let mut min = Vec2::splat(f32::INFINITY);
    let mut max = Vec2::splat(f32::NEG_INFINITY);
    for corner in corners {
        let rotated = Vec2::new(
            corner.x * cos - corner.y * sin,
            corner.x * sin + corner.y * cos,
        ) + anchor;
        min = min.min(rotated);
        max = max.max(rotated);
    }
    let size = OCCUPANCY_CELL_SIZE_METERS;
    let min_x = (min.x / size).floor() as i32;
    let max_x = (max.x / size).floor() as i32;
    let min_z = (min.y / size).floor() as i32;
    let max_z = (max.y / size).floor() as i32;
    let mut cells = Vec::new();
    for z in min_z..=max_z {
        for x in min_x..=max_x {
            let cell = super::cell::OccupancyCellCoord::new(x, z);
            let center = cell.center_global();
            if point_in_oriented_rectangle(center, anchor, width, depth, rotation) {
                cells.push(cell);
            }
        }
    }
    cells.sort_unstable();
    cells.dedup();
    cells
}

fn cells_for_baked_mask(
    anchor: Vec2,
    mask: &BakedCellMask,
    rotation: QuantizedRotation,
) -> Vec<super::cell::OccupancyCellCoord> {
    let mut cells = Vec::new();
    let cell_size = mask.cell_size_meters;
    for z in 0..mask.depth_cells {
        for x in 0..mask.width_cells {
            if !mask.is_blocked_local(x as i32, z as i32) {
                continue;
            }
            let local = mask.local_origin
                + Vec2::new((x as f32 + 0.5) * cell_size, (z as f32 + 0.5) * cell_size);
            let global = rotate_local_xz(local, rotation) + anchor;
            cells.push(occupancy_cell_at_global_xz(global));
        }
    }
    cells.sort_unstable();
    cells.dedup();
    cells
}

pub fn rotate_local_xz(local: Vec2, rotation: QuantizedRotation) -> Vec2 {
    let yaw = rotation.radians();
    let (sin, cos) = yaw.sin_cos();
    Vec2::new(local.x * cos - local.y * sin, local.x * sin + local.y * cos)
}

pub fn world_to_footprint_local(world_xz: Vec2, anchor: Vec2, rotation: QuantizedRotation) -> Vec2 {
    let delta = world_xz - anchor;
    let yaw = rotation.radians();
    let (sin, cos) = yaw.sin_cos();
    Vec2::new(
        delta.x * cos + delta.y * sin,
        -delta.x * sin + delta.y * cos,
    )
}

pub fn point_in_oriented_rectangle(
    point: Vec2,
    anchor: Vec2,
    width: f32,
    depth: f32,
    rotation: QuantizedRotation,
) -> bool {
    let local = world_to_footprint_local(point, anchor, rotation);
    local.x.abs() <= width * 0.5 && local.y.abs() <= depth * 0.5
}

/// Whether an agent circle overlaps a footprint at the given pose.
pub fn agent_overlaps_footprint(
    agent_center: Vec2,
    agent_radius: f32,
    shape: &FootprintShape,
    anchor: Vec2,
    rotation: QuantizedRotation,
) -> bool {
    match shape {
        FootprintShape::Circle { radius_meters } => {
            super::cell::circle_overlap_blocked(agent_center, anchor, agent_radius, *radius_meters)
        }
        FootprintShape::Rectangle {
            width_meters,
            depth_meters,
        } => circle_intersects_oriented_rectangle(
            agent_center,
            agent_radius,
            anchor,
            *width_meters,
            *depth_meters,
            rotation,
        ),
        FootprintShape::BakedCellMask(mask) => {
            let local =
                world_to_footprint_local(agent_center, anchor, rotation) - mask.local_origin;
            let half = agent_radius;
            let min_x = ((local.x - half) / mask.cell_size_meters).floor() as i32;
            let max_x = ((local.x + half) / mask.cell_size_meters).floor() as i32;
            let min_z = ((local.y - half) / mask.cell_size_meters).floor() as i32;
            let max_z = ((local.y + half) / mask.cell_size_meters).floor() as i32;
            for z in min_z..=max_z {
                for x in min_x..=max_x {
                    if x < 0
                        || z < 0
                        || x >= mask.width_cells as i32
                        || z >= mask.depth_cells as i32
                    {
                        continue;
                    }
                    if !mask.is_blocked_local(x, z) {
                        continue;
                    }
                    let cell_min = mask.local_origin
                        + Vec2::new(
                            x as f32 * mask.cell_size_meters,
                            z as f32 * mask.cell_size_meters,
                        );
                    let cell_max = cell_min + Vec2::splat(mask.cell_size_meters);
                    let closest = Vec2::new(
                        agent_center
                            .x
                            .clamp(cell_min.x + anchor.x, cell_max.x + anchor.x),
                        agent_center
                            .y
                            .clamp(cell_min.y + anchor.y, cell_max.y + anchor.y),
                    );
                    if closest.distance(agent_center) <= agent_radius {
                        return true;
                    }
                }
            }
            false
        }
    }
}

fn circle_intersects_oriented_rectangle(
    circle_center: Vec2,
    circle_radius: f32,
    anchor: Vec2,
    width: f32,
    depth: f32,
    rotation: QuantizedRotation,
) -> bool {
    let local = world_to_footprint_local(circle_center, anchor, rotation);
    let half = Vec2::new(width * 0.5, depth * 0.5);
    let closest = Vec2::new(
        local.x.clamp(-half.x, half.x),
        local.y.clamp(-half.y, half.y),
    );
    closest.distance(local) <= circle_radius
}

/// Synthesize inline [`FootprintDefinition`] entries from starter building specs.
pub fn inline_footprint_from_building(
    definition: &BuildingDefinition,
) -> Option<FootprintDefinition> {
    let shape = match &definition.footprint {
        FootprintSpec::Circle { radius_meters } => FootprintShape::Circle {
            radius_meters: *radius_meters,
        },
        FootprintSpec::Rectangle {
            width_meters,
            depth_meters,
        } => FootprintShape::Rectangle {
            width_meters: *width_meters,
            depth_meters: *depth_meters,
        },
        FootprintSpec::MeshDerived => return None,
    };
    Some(FootprintDefinition::new(
        FootprintId::new(definition.id.as_str()),
        shape,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::building::catalog::BuildingDefinitionId;
    use crate::world::building::catalog::BuildingRenderKey;
    use crate::world::building::category::BuildingCategoryId;

    #[test]
    fn rectangle_rotation_changes_occupied_cells() {
        let shape = FootprintShape::Rectangle {
            width_meters: 6.0,
            depth_meters: 2.0,
        };
        let anchor = Vec2::new(11.0, 10.0);
        let deg0 = occupied_cells_for_footprint(&shape, anchor, QuantizedRotation::Deg0);
        let deg90 = occupied_cells_for_footprint(&shape, anchor, QuantizedRotation::Deg90);
        assert_ne!(deg0, deg90);
        assert!(!deg0.is_empty());
    }

    #[test]
    fn baked_mask_overrides_applied() {
        let mut mask = BakedCellMask {
            cell_size_meters: 2.0,
            width_cells: 2,
            depth_cells: 2,
            local_origin: Vec2::ZERO,
            blocked_cells: BTreeSet::from([0]),
            forced_open_cells: BTreeSet::from([0]),
            forced_blocked_cells: BTreeSet::from([1]),
            space_id: 0,
        };
        mask = mask.apply_overrides();
        assert!(!mask.is_blocked_local(0, 0));
        assert!(mask.is_blocked_local(1, 0));
    }

    #[test]
    fn malformed_mask_rejected() {
        let mask = BakedCellMask {
            cell_size_meters: 2.0,
            width_cells: 1,
            depth_cells: 1,
            local_origin: Vec2::ZERO,
            blocked_cells: BTreeSet::from([5]),
            forced_open_cells: BTreeSet::new(),
            forced_blocked_cells: BTreeSet::new(),
            space_id: 0,
        };
        assert!(mask.validate().is_err());
    }

    #[test]
    fn inline_building_footprint_from_starter_hut() {
        let definition = crate::world::BuildingDefinition::new(
            BuildingDefinitionId::new("hut"),
            "Hut",
            BuildingCategoryId::new("residential"),
            BuildingRenderKey::reserved("hut"),
            BuildingRenderKey::reserved("hut"),
            100,
            30.0,
            FootprintSpec::Rectangle {
                width_meters: 4.0,
                depth_meters: 4.0,
            },
            35.0,
            true,
        );
        let shape = inline_building_footprint(&definition).unwrap();
        assert!(matches!(shape, FootprintShape::Rectangle { .. }));
    }
}
