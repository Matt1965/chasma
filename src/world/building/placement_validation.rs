//! Pure building placement validation (ADR-081 B4).
//!
//! Does not mutate [`WorldData`].

use bevy::prelude::*;

use super::catalog::{BuildingCatalog, BuildingDefinitionId};
use super::id::BuildingId;
use super::ownership::BuildingOwnership;
use super::placement_plan::quantize_placement_anchor_xz;
use crate::world::{
    ChunkCoord, ChunkId, DoodadCatalog, FootprintCatalog, OccupancySource, OccupancyState,
    QuantizedRotation, SlopeWalkability, UnitCatalog, WorldData, WorldPosition,
    agent_overlaps_footprint, chunk_for_occupancy_cell, classify_slope_walkability,
    conservative_block_radius_for_kind, default_space_id, effective_building_footprint,
    ground_world_position, occupied_cells_for_footprint,
};

/// Configurable placement policy knobs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BuildingPlacementConfig {
    /// Maximum height delta across footprint support cells (meters).
    pub max_height_variation_meters: f32,
}

impl Default for BuildingPlacementConfig {
    fn default() -> Self {
        Self {
            max_height_variation_meters: 2.0,
        }
    }
}

/// Structured rejection reasons for placement validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildingPlacementRejectReason {
    MissingDefinition,
    DisabledDefinition,
    MissingFootprint,
    DisabledFootprint,
    TerrainUnavailable,
    SlopeTooSteep,
    HeightVariationTooLarge,
    OccupiedByBuilding,
    OccupiedByDoodad,
    OccupiedByUnit,
    UnsupportedRotation,
    OutOfBounds,
    NotAuthorized,
    CorruptFootprint,
}

impl BuildingPlacementRejectReason {
    pub fn label(self) -> &'static str {
        match self {
            Self::MissingDefinition => "Missing building definition",
            Self::DisabledDefinition => "Building disabled",
            Self::MissingFootprint => "Missing footprint",
            Self::DisabledFootprint => "Footprint disabled",
            Self::TerrainUnavailable => "Terrain unavailable",
            Self::SlopeTooSteep => "Slope too steep",
            Self::HeightVariationTooLarge => "Terrain too uneven",
            Self::OccupiedByBuilding => "Blocked by building",
            Self::OccupiedByDoodad => "Blocked by doodad",
            Self::OccupiedByUnit => "Blocked by unit",
            Self::UnsupportedRotation => "Unsupported rotation",
            Self::OutOfBounds => "Out of bounds",
            Self::NotAuthorized => "Not authorized",
            Self::CorruptFootprint => "Invalid footprint data",
        }
    }
}

/// Outcome of validating one building placement candidate.
#[derive(Debug, Clone, PartialEq)]
pub struct BuildingPlacementValidation {
    pub valid: bool,
    pub primary_reason: Option<BuildingPlacementRejectReason>,
    pub reasons: Vec<BuildingPlacementRejectReason>,
    pub grounded_anchor: Option<WorldPosition>,
}

impl BuildingPlacementValidation {
    pub fn rejected(reason: BuildingPlacementRejectReason) -> Self {
        Self {
            valid: false,
            primary_reason: Some(reason),
            reasons: vec![reason],
            grounded_anchor: None,
        }
    }

    pub fn accepted(anchor: WorldPosition) -> Self {
        Self {
            valid: true,
            primary_reason: None,
            reasons: Vec::new(),
            grounded_anchor: Some(anchor),
        }
    }

    fn push_reason(&mut self, reason: BuildingPlacementRejectReason) {
        if self.primary_reason.is_none() {
            self.primary_reason = Some(reason);
        }
        if !self.reasons.contains(&reason) {
            self.reasons.push(reason);
        }
        self.valid = false;
        self.grounded_anchor = None;
    }
}

/// Read-only context for placement validation.
pub struct BuildingPlacementContext<'a> {
    pub world: &'a WorldData,
    pub building_catalog: &'a BuildingCatalog,
    pub footprint_catalog: &'a FootprintCatalog,
    pub doodad_catalog: &'a DoodadCatalog,
    pub unit_catalog: &'a UnitCatalog,
    pub config: BuildingPlacementConfig,
    pub player_authorized: bool,
}

/// Quantized yaw from 0..4 quadrant steps.
pub fn rotation_from_quadrants(quadrants: u8) -> Quat {
    Quat::from_rotation_y((quadrants % 4) as f32 * std::f32::consts::FRAC_PI_2)
}

/// Validate a building placement without mutating world data.
pub fn validate_building_placement(
    ctx: &BuildingPlacementContext<'_>,
    definition_id: &BuildingDefinitionId,
    anchor: WorldPosition,
    rotation: Quat,
    _ownership: BuildingOwnership,
) -> BuildingPlacementValidation {
    if !ctx.player_authorized {
        return BuildingPlacementValidation::rejected(BuildingPlacementRejectReason::NotAuthorized);
    }

    let Some(definition) = ctx.building_catalog.get(definition_id) else {
        return BuildingPlacementValidation::rejected(
            BuildingPlacementRejectReason::MissingDefinition,
        );
    };
    if !definition.enabled {
        return BuildingPlacementValidation::rejected(
            BuildingPlacementRejectReason::DisabledDefinition,
        );
    }

    let quantized = match QuantizedRotation::from_quat(rotation) {
        Ok(value) => value,
        Err(_) => {
            return BuildingPlacementValidation::rejected(
                BuildingPlacementRejectReason::UnsupportedRotation,
            );
        }
    };

    let shape = match effective_building_footprint(definition, ctx.footprint_catalog) {
        Ok(shape) => shape,
        Err(_) => {
            return BuildingPlacementValidation::rejected(
                BuildingPlacementRejectReason::CorruptFootprint,
            );
        }
    };

    let layout = ctx.world.layout();
    let anchor_global = anchor.to_global(layout);
    if !anchor_global.is_finite() {
        return BuildingPlacementValidation::rejected(BuildingPlacementRejectReason::OutOfBounds);
    }

    let quantized_xz = quantize_placement_anchor_xz(Vec2::new(anchor_global.x, anchor_global.z));
    let quantized_position = WorldPosition::from_global(
        Vec3::new(quantized_xz.x, anchor_global.y, quantized_xz.y),
        layout,
    );

    let Some(grounded) = ground_world_position(ctx.world, quantized_position) else {
        return BuildingPlacementValidation::rejected(
            BuildingPlacementRejectReason::TerrainUnavailable,
        );
    };

    let g = grounded.to_global(layout);
    let final_xz = quantize_placement_anchor_xz(Vec2::new(g.x, g.z));
    let grounded = WorldPosition::from_global(Vec3::new(final_xz.x, g.y, final_xz.y), layout);

    let anchor_xz = Vec2::new(final_xz.x, final_xz.y);
    let cells = occupied_cells_for_footprint(shape.as_ref(), anchor_xz, quantized);
    if cells.is_empty() {
        return BuildingPlacementValidation::rejected(
            BuildingPlacementRejectReason::CorruptFootprint,
        );
    }

    let mut validation = BuildingPlacementValidation {
        valid: true,
        primary_reason: None,
        reasons: Vec::new(),
        grounded_anchor: Some(grounded),
    };

    let mut support_heights: Vec<f32> = Vec::with_capacity(cells.len());
    let max_slope = definition.max_slope_degrees;

    for cell in &cells {
        let center = cell.center_global();
        let sample = WorldPosition::from_global(Vec3::new(center.x, 0.0, center.y), layout);
        let Some(cell_grounded) = ground_world_position(ctx.world, sample) else {
            validation.push_reason(BuildingPlacementRejectReason::TerrainUnavailable);
            continue;
        };
        match classify_slope_walkability(ctx.world, cell_grounded, max_slope) {
            SlopeWalkability::Walkable => {
                let h = cell_grounded.to_global(layout).y;
                if h.is_finite() {
                    support_heights.push(h);
                }
            }
            SlopeWalkability::TooSteep => {
                validation.push_reason(BuildingPlacementRejectReason::SlopeTooSteep);
            }
            SlopeWalkability::Unavailable => {
                validation.push_reason(BuildingPlacementRejectReason::TerrainUnavailable);
            }
        }
    }

    if validation.valid && support_heights.len() >= 2 {
        let min_h = support_heights
            .iter()
            .copied()
            .fold(f32::INFINITY, f32::min);
        let max_h = support_heights
            .iter()
            .copied()
            .fold(f32::NEG_INFINITY, f32::max);
        if max_h - min_h > ctx.config.max_height_variation_meters {
            validation.push_reason(BuildingPlacementRejectReason::HeightVariationTooLarge);
        }
    }

    if let Some(source) = footprint_occupancy_conflict(ctx, &cells, None) {
        match source {
            OccupancySource::Building(_) => {
                validation.push_reason(BuildingPlacementRejectReason::OccupiedByBuilding);
            }
            OccupancySource::Doodad(_) => {
                validation.push_reason(BuildingPlacementRejectReason::OccupiedByDoodad);
            }
        }
    }

    if building_record_overlap(ctx, shape.as_ref(), anchor_xz, quantized) {
        validation.push_reason(BuildingPlacementRejectReason::OccupiedByBuilding);
    }

    if doodad_footprint_overlap(ctx, shape.as_ref(), anchor_xz, quantized) {
        validation.push_reason(BuildingPlacementRejectReason::OccupiedByDoodad);
    }

    if unit_overlaps_footprint(ctx, shape.as_ref(), anchor_xz, quantized) {
        validation.push_reason(BuildingPlacementRejectReason::OccupiedByUnit);
    }

    validation
}

/// Validate a dev building transform candidate, excluding the edited building from overlap checks.
pub fn validate_building_transform_placement(
    ctx: &BuildingPlacementContext<'_>,
    definition_id: &BuildingDefinitionId,
    placement: super::placement::BuildingPlacement,
    exclude_building_id: BuildingId,
) -> BuildingPlacementValidation {
    if !ctx.player_authorized {
        return BuildingPlacementValidation::rejected(BuildingPlacementRejectReason::NotAuthorized);
    }

    let Some(definition) = ctx.building_catalog.get(definition_id) else {
        return BuildingPlacementValidation::rejected(
            BuildingPlacementRejectReason::MissingDefinition,
        );
    };
    if !definition.enabled {
        return BuildingPlacementValidation::rejected(
            BuildingPlacementRejectReason::DisabledDefinition,
        );
    }

    let quantized = match QuantizedRotation::from_quat(placement.rotation) {
        Ok(value) => value,
        Err(_) => {
            return BuildingPlacementValidation::rejected(
                BuildingPlacementRejectReason::UnsupportedRotation,
            );
        }
    };

    let shape = match crate::world::effective_building_footprint_for_placement(
        definition,
        ctx.footprint_catalog,
        placement.uniform_scale_f32(),
    ) {
        Ok(shape) => shape,
        Err(_) => {
            return BuildingPlacementValidation::rejected(
                BuildingPlacementRejectReason::CorruptFootprint,
            );
        }
    };

    let layout = ctx.world.layout();
    let anchor_global = placement.position.to_global(layout);
    if !anchor_global.is_finite() {
        return BuildingPlacementValidation::rejected(BuildingPlacementRejectReason::OutOfBounds);
    }

    let quantized_xz = quantize_placement_anchor_xz(Vec2::new(anchor_global.x, anchor_global.z));
    let quantized_position = WorldPosition::from_global(
        Vec3::new(quantized_xz.x, anchor_global.y, quantized_xz.y),
        layout,
    );

    let Some(grounded) = ground_world_position(ctx.world, quantized_position) else {
        return BuildingPlacementValidation::rejected(
            BuildingPlacementRejectReason::TerrainUnavailable,
        );
    };

    let g = grounded.to_global(layout);
    let final_xz = quantize_placement_anchor_xz(Vec2::new(g.x, g.z));
    let grounded = WorldPosition::from_global(Vec3::new(final_xz.x, g.y, final_xz.y), layout);

    let anchor_xz = Vec2::new(final_xz.x, final_xz.y);
    let cells = occupied_cells_for_footprint(shape.as_ref(), anchor_xz, quantized);
    if cells.is_empty() {
        return BuildingPlacementValidation::rejected(
            BuildingPlacementRejectReason::CorruptFootprint,
        );
    }

    let mut validation = BuildingPlacementValidation {
        valid: true,
        primary_reason: None,
        reasons: Vec::new(),
        grounded_anchor: Some(grounded),
    };

    let exclude = Some(OccupancySource::Building(exclude_building_id));
    if let Some(source) = footprint_occupancy_conflict(ctx, &cells, exclude) {
        match source {
            OccupancySource::Building(_) => {
                validation.push_reason(BuildingPlacementRejectReason::OccupiedByBuilding);
            }
            OccupancySource::Doodad(_) => {
                validation.push_reason(BuildingPlacementRejectReason::OccupiedByDoodad);
            }
        }
    }

    if building_record_overlap_excluding(
        ctx,
        shape.as_ref(),
        anchor_xz,
        quantized,
        exclude_building_id,
    ) {
        validation.push_reason(BuildingPlacementRejectReason::OccupiedByBuilding);
    }

    if doodad_footprint_overlap(ctx, shape.as_ref(), anchor_xz, quantized) {
        validation.push_reason(BuildingPlacementRejectReason::OccupiedByDoodad);
    }

    if unit_overlaps_footprint(ctx, shape.as_ref(), anchor_xz, quantized) {
        validation.push_reason(BuildingPlacementRejectReason::OccupiedByUnit);
    }

    validation
}

fn building_record_overlap_excluding(
    ctx: &BuildingPlacementContext<'_>,
    shape: &crate::world::FootprintShape,
    anchor_xz: Vec2,
    rotation: QuantizedRotation,
    exclude_building_id: BuildingId,
) -> bool {
    let layout = ctx.world.layout();
    let probe_cells = occupied_cells_for_footprint(shape, anchor_xz, rotation);
    let mut chunks: Vec<ChunkCoord> = Vec::new();
    for cell in probe_cells {
        let chunk = chunk_for_occupancy_cell(cell, layout);
        if !chunks.contains(&chunk) {
            chunks.push(chunk);
        }
    }
    let base_chunks = chunks.clone();
    for chunk in base_chunks {
        for dz in -1..=1 {
            for dx in -1..=1 {
                let neighbor = ChunkCoord::new(chunk.x + dx, chunk.z + dz);
                if !chunks.contains(&neighbor) {
                    chunks.push(neighbor);
                }
            }
        }
    }
    chunks.sort_by_key(|c| (c.x, c.z));

    for chunk_coord in chunks {
        let chunk_id = ChunkId::new(chunk_coord);
        let Some(store) = ctx.world.buildings_in_chunk(chunk_id) else {
            continue;
        };
        for record in store.records() {
            if record.id == exclude_building_id {
                continue;
            }
            let definition = match ctx.building_catalog.get(&record.definition_id) {
                Some(def) => def,
                None => continue,
            };
            let other_shape = match crate::world::effective_building_footprint_for_placement(
                definition,
                ctx.footprint_catalog,
                record.placement.uniform_scale_f32(),
            ) {
                Ok(s) => s,
                Err(_) => continue,
            };
            let other_rot = match QuantizedRotation::from_quat(record.placement.rotation) {
                Ok(r) => r,
                Err(_) => continue,
            };
            let other_global = record.placement.position.to_global(layout);
            let other_xz = Vec2::new(other_global.x, other_global.z);
            if footprints_overlap(
                shape,
                anchor_xz,
                rotation,
                other_shape.as_ref(),
                other_xz,
                other_rot,
            ) {
                return true;
            }
        }
    }
    false
}

fn footprint_occupancy_conflict(
    ctx: &BuildingPlacementContext<'_>,
    cells: &[crate::world::OccupancyCellCoord],
    exclude: Option<OccupancySource>,
) -> Option<OccupancySource> {
    let layout = ctx.world.layout();
    let space_id = default_space_id();
    for cell in cells {
        let chunk = ChunkId::new(chunk_for_occupancy_cell(*cell, layout));
        let Some(entry) = ctx.world.occupancy_cell(chunk, *cell, space_id) else {
            continue;
        };
        if exclude.is_some_and(|source| source == entry.source) {
            continue;
        }
        if matches!(
            entry.state,
            OccupancyState::Blocked | OccupancyState::Reserved
        ) {
            return Some(entry.source);
        }
    }
    None
}

fn building_record_overlap(
    ctx: &BuildingPlacementContext<'_>,
    shape: &crate::world::FootprintShape,
    anchor_xz: Vec2,
    rotation: QuantizedRotation,
) -> bool {
    let layout = ctx.world.layout();
    let probe_cells = occupied_cells_for_footprint(shape, anchor_xz, rotation);
    let mut chunks: Vec<ChunkCoord> = Vec::new();
    for cell in probe_cells {
        let chunk = chunk_for_occupancy_cell(cell, layout);
        if !chunks.contains(&chunk) {
            chunks.push(chunk);
        }
    }
    let base_chunks = chunks.clone();
    for chunk in base_chunks {
        for dz in -1..=1 {
            for dx in -1..=1 {
                let neighbor = ChunkCoord::new(chunk.x + dx, chunk.z + dz);
                if !chunks.contains(&neighbor) {
                    chunks.push(neighbor);
                }
            }
        }
    }
    chunks.sort_by_key(|c| (c.x, c.z));

    for chunk_coord in chunks {
        let chunk_id = ChunkId::new(chunk_coord);
        let Some(store) = ctx.world.buildings_in_chunk(chunk_id) else {
            continue;
        };
        for record in store.records() {
            let definition = match ctx.building_catalog.get(&record.definition_id) {
                Some(def) => def,
                None => continue,
            };
            let other_shape = match effective_building_footprint(definition, ctx.footprint_catalog)
            {
                Ok(s) => s,
                Err(_) => continue,
            };
            let other_rot = match QuantizedRotation::from_quat(record.placement.rotation) {
                Ok(r) => r,
                Err(_) => continue,
            };
            let other_global = record.placement.position.to_global(layout);
            let other_xz = Vec2::new(other_global.x, other_global.z);
            if footprints_overlap(
                shape,
                anchor_xz,
                rotation,
                other_shape.as_ref(),
                other_xz,
                other_rot,
            ) {
                return true;
            }
        }
    }
    false
}

fn footprints_overlap_continuous(
    a_shape: &crate::world::FootprintShape,
    a_anchor: Vec2,
    a_yaw_radians: f32,
    b_shape: &crate::world::FootprintShape,
    b_anchor: Vec2,
    b_yaw_radians: f32,
) -> bool {
    use crate::world::{agent_overlaps_footprint_continuous, occupied_cells_for_footprint_yaw};
    let cells = occupied_cells_for_footprint_yaw(a_shape, a_anchor, a_yaw_radians);
    for cell in cells {
        let center = cell.center_global();
        if agent_overlaps_footprint_continuous(center, 0.01, b_shape, b_anchor, b_yaw_radians) {
            return true;
        }
    }
    false
}

fn footprints_overlap(
    a_shape: &crate::world::FootprintShape,
    a_anchor: Vec2,
    a_rot: QuantizedRotation,
    b_shape: &crate::world::FootprintShape,
    b_anchor: Vec2,
    b_rot: QuantizedRotation,
) -> bool {
    use crate::world::agent_overlaps_footprint;
    let cells = occupied_cells_for_footprint(a_shape, a_anchor, a_rot);
    for cell in cells {
        let center = cell.center_global();
        if agent_overlaps_footprint(center, 0.01, b_shape, b_anchor, b_rot) {
            return true;
        }
    }
    false
}

fn doodad_footprint_overlap(
    ctx: &BuildingPlacementContext<'_>,
    shape: &crate::world::FootprintShape,
    anchor_xz: Vec2,
    rotation: QuantizedRotation,
) -> bool {
    use crate::world::{FootprintShape, agent_overlaps_footprint, default_blocks_movement};

    let layout = ctx.world.layout();
    let cells = occupied_cells_for_footprint(shape, anchor_xz, rotation);
    let mut chunk_coords: Vec<ChunkCoord> = Vec::new();
    for cell in &cells {
        let chunk = chunk_for_occupancy_cell(*cell, layout);
        if !chunk_coords.contains(&chunk) {
            chunk_coords.push(chunk);
        }
    }

    for chunk_coord in chunk_coords {
        let chunk_id = ChunkId::new(chunk_coord);
        let Some(store) = ctx.world.doodads_in_chunk(chunk_id) else {
            continue;
        };
        for record in store.records() {
            let collision =
                crate::world::resolve_doodad_collision_from_catalog(record, ctx.doodad_catalog);
            if !collision.blocks_movement {
                continue;
            }
            let doodad_global = record.placement.position.to_global(layout);
            let doodad_xz = Vec2::new(doodad_global.x, doodad_global.z);
            if footprints_overlap_continuous(
                shape,
                anchor_xz,
                rotation.radians(),
                &collision.shape,
                doodad_xz,
                collision.yaw_radians,
            ) {
                return true;
            }
        }
    }
    false
}

fn unit_overlaps_footprint(
    ctx: &BuildingPlacementContext<'_>,
    shape: &crate::world::FootprintShape,
    anchor_xz: Vec2,
    rotation: QuantizedRotation,
) -> bool {
    use crate::world::agent_overlaps_footprint;

    let layout = ctx.world.layout();
    let cells = occupied_cells_for_footprint(shape, anchor_xz, rotation);
    let cell_size = crate::world::OCCUPANCY_CELL_SIZE_METERS;

    for cell in cells {
        let center = cell.center_global();
        let sample = WorldPosition::from_global(Vec3::new(center.x, 0.0, center.y), layout);
        let nearby = ctx
            .world
            .query_units_in_radius(sample, cell_size * 0.75 + 2.0, None);
        for unit_id in nearby {
            let Some(record) = ctx.world.get_unit(unit_id) else {
                continue;
            };
            let radius = ctx
                .unit_catalog
                .get(&record.definition_id)
                .map(|def| def.collision_radius_meters)
                .unwrap_or(0.5);
            let unit_global = record.placement.position.to_global(layout);
            let unit_xz = Vec2::new(unit_global.x, unit_global.z);
            if agent_overlaps_footprint(unit_xz, radius, shape, anchor_xz, rotation) {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        BuildingCatalog, BuildingOwnership, BuildingSource, ChunkCoord, ChunkData, ChunkId,
        ChunkLayout, DoodadCatalog, DoodadDefinitionId, DoodadPlacementOverrides, DoodadSource,
        FootprintCatalog, Heightfield, LocalPosition, OccupancyCatalogs, UnitCatalog,
        create_building, create_doodad, create_unit, register_building_occupancy,
    };

    fn flat_world() -> WorldData {
        let layout = ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        };
        let mut world = WorldData::new(layout);
        let heightfield = Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
        world.insert(
            ChunkId::new(ChunkCoord::new(0, 0)),
            ChunkData::new(heightfield, Vec::new()),
        );
        world
    }

    fn pos(x: f32, z: f32) -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(x, 0.0, z)),
        )
    }

    fn ctx<'a>(
        world: &'a WorldData,
        building: &'a BuildingCatalog,
        footprint: &'a FootprintCatalog,
        doodad: &'a DoodadCatalog,
        unit: &'a UnitCatalog,
    ) -> BuildingPlacementContext<'a> {
        BuildingPlacementContext {
            world,
            building_catalog: building,
            footprint_catalog: footprint,
            doodad_catalog: doodad,
            unit_catalog: unit,
            config: BuildingPlacementConfig::default(),
            player_authorized: true,
        }
    }

    #[test]
    fn valid_flat_placement_accepted() {
        let world = flat_world();
        let building = BuildingCatalog::default();
        let footprint = FootprintCatalog::default();
        let doodad = DoodadCatalog::default();
        let unit = UnitCatalog::default();
        let validation = validate_building_placement(
            &ctx(&world, &building, &footprint, &doodad, &unit),
            &BuildingDefinitionId::new("hut"),
            pos(64.0, 64.0),
            Quat::IDENTITY,
            BuildingOwnership::neutral(),
        );
        assert!(validation.valid);
    }

    #[test]
    fn terrain_unavailable_rejected() {
        let world = WorldData::new(ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        });
        let building = BuildingCatalog::default();
        let footprint = FootprintCatalog::default();
        let doodad = DoodadCatalog::default();
        let unit = UnitCatalog::default();
        let validation = validate_building_placement(
            &ctx(&world, &building, &footprint, &doodad, &unit),
            &BuildingDefinitionId::new("hut"),
            pos(64.0, 64.0),
            Quat::IDENTITY,
            BuildingOwnership::neutral(),
        );
        assert_eq!(
            validation.primary_reason,
            Some(BuildingPlacementRejectReason::TerrainUnavailable)
        );
    }

    #[test]
    fn unsupported_rotation_rejected() {
        let world = flat_world();
        let building = BuildingCatalog::default();
        let footprint = FootprintCatalog::default();
        let doodad = DoodadCatalog::default();
        let unit = UnitCatalog::default();
        let validation = validate_building_placement(
            &ctx(&world, &building, &footprint, &doodad, &unit),
            &BuildingDefinitionId::new("hut"),
            pos(64.0, 64.0),
            Quat::from_rotation_y(0.3),
            BuildingOwnership::neutral(),
        );
        assert_eq!(
            validation.primary_reason,
            Some(BuildingPlacementRejectReason::UnsupportedRotation)
        );
    }

    #[test]
    fn building_overlap_rejected() {
        let building = BuildingCatalog::default();
        let footprint = FootprintCatalog::default();
        let doodad = DoodadCatalog::default();
        let unit = UnitCatalog::default();
        let mut world = flat_world();
        let occ = OccupancyCatalogs {
            doodad: &doodad,
            building: &building,
            footprint: &footprint,
        };
        create_building(
            &building,
            &mut world,
            &BuildingDefinitionId::new("hut"),
            pos(64.0, 64.0),
            Quat::IDENTITY,
            BuildingSource::Authored,
            BuildingOwnership::neutral(),
            Some(occ),
        )
        .unwrap();

        let validation = validate_building_placement(
            &ctx(&world, &building, &footprint, &doodad, &unit),
            &BuildingDefinitionId::new("hut"),
            pos(64.0, 64.0),
            Quat::IDENTITY,
            BuildingOwnership::neutral(),
        );
        assert_eq!(
            validation.primary_reason,
            Some(BuildingPlacementRejectReason::OccupiedByBuilding)
        );
    }

    #[test]
    fn deterministic_validation() {
        let world = flat_world();
        let building = BuildingCatalog::default();
        let footprint = FootprintCatalog::default();
        let doodad = DoodadCatalog::default();
        let unit = UnitCatalog::default();
        let c = ctx(&world, &building, &footprint, &doodad, &unit);
        let a = validate_building_placement(
            &c,
            &BuildingDefinitionId::new("hut"),
            pos(80.0, 80.0),
            Quat::IDENTITY,
            BuildingOwnership::neutral(),
        );
        let b = validate_building_placement(
            &c,
            &BuildingDefinitionId::new("hut"),
            pos(80.0, 80.0),
            Quat::IDENTITY,
            BuildingOwnership::neutral(),
        );
        assert_eq!(a, b);
    }
}
