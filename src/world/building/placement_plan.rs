//! Canonical building placement preview/commit contract (ADR-096 BP-CLEANUP).
//!
//! One pure plan drives ghost footprint, ghost model, validation, occupancy registration,
//! and runtime render transforms.

use bevy::prelude::*;

use super::catalog::{BuildingCatalog, BuildingDefinition, BuildingDefinitionId};
use super::ownership::BuildingOwnership;
use super::placement::BuildingPlacement;
use super::placement_validation::{
    BuildingPlacementConfig, BuildingPlacementContext, BuildingPlacementValidation,
    rotation_from_quadrants,
};
use crate::world::asset_sizing::{
    building_effective_model_offset, building_visual_scale, sizing_rotation_correction,
};
use crate::world::{
    FootprintCatalog, QuantizedRotation, WorldData, WorldPosition,
    effective_building_footprint_for_placement, ground_world_position,
    occupied_cells_for_footprint,
};

/// Fine deterministic placement quantization (meters). Smaller than occupancy cells (2 m).
pub const PLACEMENT_QUANTIZE_METERS: f32 = 0.1;

/// Quantize global XZ to a fine grid for stable, visually continuous placement.
pub fn quantize_placement_anchor_xz(global_xz: Vec2) -> Vec2 {
    let q = PLACEMENT_QUANTIZE_METERS;
    Vec2::new((global_xz.x / q).round() * q, (global_xz.y / q).round() * q)
}

/// Legacy name retained for callers migrating off occupancy-cell snapping.
#[inline]
pub fn snap_anchor_global_xz(global_xz: Vec2) -> Vec2 {
    quantize_placement_anchor_xz(global_xz)
}

/// Ground a terrain click and quantize XZ (continuous anchor, not cell centers).
pub fn ground_and_quantize_building_anchor(
    world: &WorldData,
    click: WorldPosition,
) -> Option<WorldPosition> {
    let layout = world.layout();
    let global = click.to_global(layout);
    if !global.is_finite() {
        return None;
    }
    let q_xz = quantize_placement_anchor_xz(Vec2::new(global.x, global.z));
    let candidate = WorldPosition::from_global(Vec3::new(q_xz.x, global.y, q_xz.y), layout);
    let grounded = ground_world_position(world, candidate)?;
    let g = grounded.to_global(layout);
    let final_xz = quantize_placement_anchor_xz(Vec2::new(g.x, g.z));
    Some(WorldPosition::from_global(
        Vec3::new(final_xz.x, g.y, final_xz.y),
        layout,
    ))
}

/// Alias used by build-mode preview.
pub fn anchor_from_terrain_position(
    world: &WorldData,
    click: WorldPosition,
) -> Option<WorldPosition> {
    ground_and_quantize_building_anchor(world, click)
}

/// Pure placement preview/commit payload shared by player and dev paths.
#[derive(Debug, Clone, PartialEq)]
pub struct BuildingPlacementPlan {
    pub grounded_anchor: WorldPosition,
    pub rotation: Quat,
    pub quantized_rotation: QuantizedRotation,
    pub anchor_global_xz: Vec2,
    pub occupied_cells: Vec<crate::world::OccupancyCellCoord>,
    pub validation: BuildingPlacementValidation,
}

impl BuildingPlacementPlan {
    pub fn is_valid(&self) -> bool {
        self.validation.valid
    }

    pub fn placement(&self) -> BuildingPlacement {
        BuildingPlacement::new(self.grounded_anchor, self.rotation)
    }

    pub fn placement_with_scale(
        self,
        uniform_scale: crate::world::FixedScale,
    ) -> BuildingPlacement {
        BuildingPlacement::new(self.grounded_anchor, self.rotation)
            .with_uniform_scale(uniform_scale)
    }
}

/// Build a placement plan without mutating world data.
pub fn build_building_placement_plan(
    ctx: &BuildingPlacementContext<'_>,
    definition_id: &BuildingDefinitionId,
    candidate_anchor: WorldPosition,
    rotation: Quat,
    ownership: BuildingOwnership,
) -> BuildingPlacementPlan {
    let validation = super::placement_validation::validate_building_placement(
        ctx,
        definition_id,
        candidate_anchor,
        rotation,
        ownership,
    );
    let Some(definition) = ctx.building_catalog.get(definition_id) else {
        return BuildingPlacementPlan {
            grounded_anchor: candidate_anchor,
            rotation,
            quantized_rotation: QuantizedRotation::Deg0,
            anchor_global_xz: Vec2::ZERO,
            occupied_cells: Vec::new(),
            validation,
        };
    };
    let Ok(quantized) = QuantizedRotation::from_quat(rotation) else {
        return BuildingPlacementPlan {
            grounded_anchor: candidate_anchor,
            rotation,
            quantized_rotation: QuantizedRotation::Deg0,
            anchor_global_xz: Vec2::ZERO,
            occupied_cells: Vec::new(),
            validation,
        };
    };
    let Ok(shape) =
        effective_building_footprint_for_placement(definition, ctx.footprint_catalog, 1.0)
    else {
        return BuildingPlacementPlan {
            grounded_anchor: candidate_anchor,
            rotation,
            quantized_rotation: quantized,
            anchor_global_xz: Vec2::ZERO,
            occupied_cells: Vec::new(),
            validation,
        };
    };

    let (grounded_anchor, anchor_global_xz, occupied_cells) =
        if let Some(anchor) = validation.grounded_anchor {
            let layout = ctx.world.layout();
            let g = anchor.to_global(layout);
            let anchor_xz = Vec2::new(g.x, g.z);
            let cells = occupied_cells_for_footprint(shape.as_ref(), anchor_xz, quantized);
            (anchor, anchor_xz, cells)
        } else {
            (candidate_anchor, Vec2::ZERO, Vec::new())
        };

    BuildingPlacementPlan {
        grounded_anchor,
        rotation,
        quantized_rotation: quantized,
        anchor_global_xz,
        occupied_cells,
        validation,
    }
}

/// Authoritative anchor transform (footprint pivot) — placement pose only.
///
/// AT2: definition rotation correction and visual scale live on the model child / flat
/// composed transform so yaw is not applied twice.
pub fn building_anchor_world_transform(
    _definition: &BuildingDefinition,
    placement: &BuildingPlacement,
    layout: crate::world::ChunkLayout,
) -> Transform {
    Transform {
        translation: placement.position.to_global(layout),
        rotation: placement.rotation,
        scale: Vec3::ONE,
    }
}

/// Local model correction for off-origin GLBs (child transform under anchor).
///
/// Prefer [`crate::world::building_model_child_local_transform`] which includes rotation
/// correction and composed visual scale (AT2).
pub fn building_model_correction_local_transform(definition: &BuildingDefinition) -> Transform {
    Transform {
        translation: building_effective_model_offset(definition),
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
    }
}

pub fn building_has_model_correction(definition: &BuildingDefinition) -> bool {
    building_effective_model_offset(definition) != Vec3::ZERO
}

/// Anchor transform in render space (terrain vertical scale on ground Y only).
pub fn building_anchor_render_transform(
    definition: &BuildingDefinition,
    placement: &BuildingPlacement,
    layout: crate::world::ChunkLayout,
    vertical_scale: f32,
) -> Transform {
    let mut transform = building_anchor_world_transform(definition, placement, layout);
    transform.translation.y =
        crate::terrain::render_height(transform.translation.y, vertical_scale);
    transform
}

/// Composed model presentation transform in world space (before render vertical scale).
///
/// Scale = definition baseline × instance (AT2). Rotation = placement × definition correction.
/// Offset is local and not multiplied by visual scale (matches model-child TRS).
pub fn building_model_world_transform(
    definition: &BuildingDefinition,
    placement: &BuildingPlacement,
    layout: crate::world::ChunkLayout,
) -> Transform {
    let anchor = building_anchor_world_transform(definition, placement, layout);
    let correction = sizing_rotation_correction(definition.asset_sizing.rotation_correction);
    let world_rotation = anchor.rotation * correction;
    let offset = world_rotation * building_effective_model_offset(definition);
    Transform {
        translation: anchor.translation + offset,
        rotation: world_rotation,
        scale: building_visual_scale(definition, placement.uniform_scale_f32()),
    }
}

/// Render-space model transform (terrain vertical scale on Y).
pub fn building_model_render_transform(
    definition: &BuildingDefinition,
    placement: &BuildingPlacement,
    layout: crate::world::ChunkLayout,
    vertical_scale: f32,
) -> Transform {
    let world = building_model_world_transform(definition, placement, layout);
    let mut translation = world.translation;
    translation.y = crate::terrain::render_height(translation.y, vertical_scale);
    Transform {
        translation,
        rotation: world.rotation,
        scale: world.scale,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        BuildingCatalog, BuildingDefinition, BuildingOwnership, ChunkCoord, ChunkData, ChunkId,
        ChunkLayout, DoodadCatalog, FootprintCatalog, Heightfield, LocalPosition, UnitCatalog,
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
    fn quantize_uses_fine_grid_not_occupancy_cells() {
        let q = quantize_placement_anchor_xz(Vec2::new(10.37, 20.83));
        assert!((q.x - 10.4).abs() < 0.001);
        assert!((q.y - 20.8).abs() < 0.001);
        assert!((q.x % 2.0).abs() > 0.01);
    }

    #[test]
    fn nearby_subcell_anchors_remain_distinct() {
        let a = quantize_placement_anchor_xz(Vec2::new(10.04, 10.0));
        let b = quantize_placement_anchor_xz(Vec2::new(10.14, 10.0));
        assert_ne!(a, b);
    }

    #[test]
    fn plan_cells_match_validation_anchor() {
        let world = flat_world();
        let building = BuildingCatalog::default();
        let footprint = FootprintCatalog::default();
        let doodad = DoodadCatalog::default();
        let unit = UnitCatalog::default();
        let plan = build_building_placement_plan(
            &ctx(&world, &building, &footprint, &doodad, &unit),
            &BuildingDefinitionId::new("hut"),
            pos(64.37, 64.52),
            Quat::IDENTITY,
            BuildingOwnership::neutral(),
        );
        assert!(plan.is_valid());
        assert!(!plan.occupied_cells.is_empty());
        let recomputed = occupied_cells_for_footprint(
            &crate::world::effective_building_footprint(
                building.get(&BuildingDefinitionId::new("hut")).unwrap(),
                &footprint,
            )
            .unwrap(),
            plan.anchor_global_xz,
            plan.quantized_rotation,
        );
        assert_eq!(plan.occupied_cells, recomputed);
    }

    #[test]
    fn asymmetric_footprint_rotations_share_anchor() {
        let world = flat_world();
        let building = BuildingCatalog::default();
        let footprint = FootprintCatalog::default();
        let doodad = DoodadCatalog::default();
        let unit = UnitCatalog::default();
        let definition_id = BuildingDefinitionId::new("barn");
        let anchor = pos(80.3, 80.7);
        for quadrants in 0..4u8 {
            let rotation = rotation_from_quadrants(quadrants);
            let plan = build_building_placement_plan(
                &ctx(&world, &building, &footprint, &doodad, &unit),
                &definition_id,
                anchor,
                rotation,
                BuildingOwnership::neutral(),
            );
            assert!(plan.is_valid(), "quadrant {quadrants}");
            assert!(!plan.occupied_cells.is_empty());
        }
    }

    #[test]
    fn model_offset_applied_once_in_local_space() {
        let definition = BuildingDefinition::new(
            BuildingDefinitionId::new("hut"),
            "Hut",
            crate::world::BuildingCategoryId::new("residential"),
            crate::world::BuildingRenderKey::reserved("hut"),
            crate::world::BuildingRenderKey::reserved("hut_collision"),
            100,
            1.0,
            crate::world::FootprintSpec::Rectangle {
                width_meters: 4.0,
                depth_meters: 4.0,
            },
            35.0,
            true,
        )
        .with_model_local_offset(Vec3::new(1.0, 0.0, 0.5));
        let placement = BuildingPlacement::new(pos(10.0, 20.0), Quat::IDENTITY);
        let layout = ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        };
        let transform = building_model_world_transform(&definition, &placement, layout);
        assert!((transform.translation.x - 11.0).abs() < 0.001);
        assert!((transform.translation.z - 20.5).abs() < 0.001);
    }

    #[test]
    fn barn_builtin_offset_shifts_model_to_anchor() {
        let definition = BuildingDefinition::new(
            BuildingDefinitionId::new("barn"),
            "Barn",
            crate::world::BuildingCategoryId::new("storage"),
            crate::world::BuildingRenderKey::reserved("barn"),
            crate::world::BuildingRenderKey::reserved("barn"),
            400,
            90.0,
            crate::world::FootprintSpec::Rectangle {
                width_meters: 8.0,
                depth_meters: 6.0,
            },
            35.0,
            true,
        );
        let placement = BuildingPlacement::new(pos(10.0, 20.0), Quat::IDENTITY);
        let layout = ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        };
        let transform = building_model_world_transform(&definition, &placement, layout);
        assert!((transform.translation.x - 17.05).abs() < 0.01);
        assert!((transform.translation.z - 1.35).abs() < 0.01);
    }
}
