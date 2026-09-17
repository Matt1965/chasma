//! Canonical building terrain placement resolver.

use bevy::prelude::*;

use super::foundation::{
    FoundationSkirtSpec, footprint_horizontal_span_meters, presentation_foundation_depth_meters,
};
use super::mode::{
    MAX_FOUNDATION_VISIBLE_DEPTH_METERS, PRESENTATION_TERRAIN_CLEARANCE_METERS,
    TerrainPlacementDefaults, TerrainPlacementMode, terrain_clearance_sim,
};
use crate::terrain::render_height;
use super::sample::{
    base_height_on_plane_at, presentation_plane_normal, sample_terrain_under_footprint,
    TerrainFootprintReport,
};
use crate::world::building::catalog::BuildingDefinition;
use crate::world::building::placement_plan::quantize_placement_anchor_xz;
use crate::world::building::placement_validation::BuildingPlacementRejectReason;
use crate::world::occupancy::FootprintShape;
use crate::world::terrain::{classify_slope_walkability, SlopeWalkability, TerrainQueryError};
use crate::world::{
    ChunkLayout, FootprintCatalog, QuantizedRotation, WorldData, WorldPosition,
    effective_building_footprint_for_placement,
};

/// Fully resolved terrain-aware building placement pose.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedBuildingPlacement {
    pub anchor: WorldPosition,
    pub rotation: Quat,
    pub quantized_yaw: QuantizedRotation,
    pub foundation: Option<FoundationSkirtSpec>,
    pub terrain_report: TerrainFootprintReport,
}

/// Resolve terrain geometry for a building placement candidate.
pub fn resolve_building_placement(
    world: &WorldData,
    layout: ChunkLayout,
    definition: &BuildingDefinition,
    footprint_catalog: &FootprintCatalog,
    candidate_anchor: WorldPosition,
    yaw: Quat,
    uniform_scale: f32,
    terrain_vertical_scale: f32,
) -> Result<ResolvedBuildingPlacement, BuildingPlacementRejectReason> {
    let quantized_yaw = QuantizedRotation::from_quat(yaw)
        .map_err(|_| BuildingPlacementRejectReason::UnsupportedRotation)?;
    let yaw_radians = quantized_yaw.radians();

    let shape = effective_building_footprint_for_placement(
        definition,
        footprint_catalog,
        uniform_scale,
    )
    .map_err(|_| BuildingPlacementRejectReason::CorruptFootprint)?;

    let anchor_global = candidate_anchor.to_global(layout);
    if !anchor_global.is_finite() {
        return Err(BuildingPlacementRejectReason::OutOfBounds);
    }

    let quantized_xz = quantize_placement_anchor_xz(Vec2::new(anchor_global.x, anchor_global.z));
    let anchor_xz = quantized_xz;

    let terrain = match sample_terrain_under_footprint(
        world,
        layout,
        shape.as_ref(),
        anchor_xz,
        yaw_radians,
    ) {
        Ok(report) => report,
        Err(TerrainQueryError::ChunkNotResident) | Err(TerrainQueryError::InvalidTerrainCoordinate) => {
            return Err(BuildingPlacementRejectReason::TerrainUnavailable);
        }
        Err(TerrainQueryError::SlopeUnavailable) => {
            return Err(BuildingPlacementRejectReason::TerrainUnavailable);
        }
    };

    let defaults = TerrainPlacementDefaults::for_mode(definition.terrain_placement_mode);
    let max_slope = definition.max_slope_degrees;

    validate_terrain_samples(world, layout, &terrain, max_slope)?;

    if terrain.height_range > defaults.max_height_variation_meters {
        return Err(BuildingPlacementRejectReason::HeightVariationTooLarge);
    }

    match definition.terrain_placement_mode {
        TerrainPlacementMode::LevelFoundation => resolve_level_foundation(
            world,
            layout,
            anchor_xz,
            yaw,
            quantized_yaw,
            shape.as_ref(),
            &terrain,
            defaults,
            terrain_vertical_scale,
        ),
        TerrainPlacementMode::ConformToTerrain => resolve_conform_to_terrain(
            layout,
            anchor_xz,
            yaw,
            quantized_yaw,
            &terrain,
            defaults,
            terrain_vertical_scale,
        ),
    }
}

fn validate_terrain_samples(
    world: &WorldData,
    layout: ChunkLayout,
    terrain: &TerrainFootprintReport,
    max_slope: f32,
) -> Result<(), BuildingPlacementRejectReason> {
    for sample in &terrain.samples {
        let position = WorldPosition::from_global(
            Vec3::new(sample.global_xz.x, sample.height, sample.global_xz.y),
            layout,
        );
        match classify_slope_walkability(world, position, max_slope) {
            SlopeWalkability::Walkable => {}
            SlopeWalkability::TooSteep => {
                return Err(BuildingPlacementRejectReason::SlopeTooSteep);
            }
            SlopeWalkability::Unavailable => {
                return Err(BuildingPlacementRejectReason::TerrainUnavailable);
            }
        }
    }
    Ok(())
}

fn resolve_level_foundation(
    world: &WorldData,
    layout: ChunkLayout,
    anchor_xz: Vec2,
    yaw: Quat,
    quantized_yaw: QuantizedRotation,
    shape: &crate::world::occupancy::FootprintShape,
    terrain: &TerrainFootprintReport,
    defaults: TerrainPlacementDefaults,
    terrain_vertical_scale: f32,
) -> Result<ResolvedBuildingPlacement, BuildingPlacementRejectReason> {
    let clearance_sim = terrain_clearance_sim(terrain_vertical_scale);
    let floor_world_y = terrain.max_height + clearance_sim;
    let footprint_span = footprint_horizontal_span_meters(shape);
    let presentation_depth = presentation_foundation_depth_meters(
        terrain,
        footprint_span,
        terrain_vertical_scale,
    );
    if presentation_depth > MAX_FOUNDATION_VISIBLE_DEPTH_METERS {
        return Err(BuildingPlacementRejectReason::FoundationTooDeep);
    }
    if presentation_depth > defaults.max_foundation_depth_meters * terrain_vertical_scale.max(1.0) {
        return Err(BuildingPlacementRejectReason::FoundationTooDeep);
    }

    let anchor = world_position_at_height(world, layout, anchor_xz, floor_world_y)?;
    let foundation = FoundationSkirtSpec::from_level_placement(
        anchor_xz,
        floor_world_y,
        &terrain.perimeter,
        terrain.min_height,
        quantized_yaw.radians(),
        terrain,
        footprint_span,
        terrain_vertical_scale,
    );

    Ok(ResolvedBuildingPlacement {
        anchor,
        rotation: yaw,
        quantized_yaw,
        foundation,
        terrain_report: terrain.clone(),
    })
}

fn resolve_conform_to_terrain(
    layout: ChunkLayout,
    anchor_xz: Vec2,
    _yaw: Quat,
    quantized_yaw: QuantizedRotation,
    terrain: &TerrainFootprintReport,
    defaults: TerrainPlacementDefaults,
    terrain_vertical_scale: f32,
) -> Result<ResolvedBuildingPlacement, BuildingPlacementRejectReason> {
    if terrain.plane_rms_residual > defaults.max_plane_fit_rms_meters {
        return Err(BuildingPlacementRejectReason::TerrainTooRough);
    }
    if terrain.plane_peak_residual > defaults.max_plane_fit_peak_meters {
        return Err(BuildingPlacementRejectReason::TerrainTooRough);
    }

    let simulation_up = terrain.plane_normal;
    if simulation_up.y < 1e-4 {
        return Err(BuildingPlacementRejectReason::TerrainTooRough);
    }
    let presentation_up = if terrain_vertical_scale > 1.0 + 1e-4 {
        presentation_plane_normal(terrain, terrain_vertical_scale)
    } else {
        simulation_up
    };

    let rotation = rotation_from_yaw_and_normal(quantized_yaw.radians(), presentation_up);
    let clearance_sim = terrain_clearance_sim(terrain_vertical_scale);

    let mut required_anchor_y = f32::NEG_INFINITY;
    for sample in &terrain.samples {
        let dx = sample.global_xz.x - anchor_xz.x;
        let dz = sample.global_xz.y - anchor_xz.y;
        let needed = sample.height
            + clearance_sim
            + (dx * simulation_up.x + dz * simulation_up.z) / simulation_up.y;
        required_anchor_y = required_anchor_y.max(needed);
    }

    if !required_anchor_y.is_finite() {
        return Err(BuildingPlacementRejectReason::TerrainUnavailable);
    }

    for sample in &terrain.samples {
        let base_y =
            base_height_on_plane_at(anchor_xz, required_anchor_y, simulation_up, sample.global_xz);
        if base_y < sample.height + clearance_sim - 1e-3 {
            return Err(BuildingPlacementRejectReason::TerrainTooRough);
        }
    }

    required_anchor_y = lift_conform_anchor_for_presentation_clearance(
        anchor_xz,
        required_anchor_y,
        simulation_up,
        terrain,
        terrain_vertical_scale,
    );

    let anchor = WorldPosition::from_global(
        Vec3::new(anchor_xz.x, required_anchor_y, anchor_xz.y),
        layout,
    );

    Ok(ResolvedBuildingPlacement {
        anchor,
        rotation,
        quantized_yaw,
        foundation: None,
        terrain_report: terrain.clone(),
    })
}

fn lift_conform_anchor_for_presentation_clearance(
    anchor_xz: Vec2,
    anchor_y: f32,
    simulation_up: Vec3,
    terrain: &TerrainFootprintReport,
    terrain_vertical_scale: f32,
) -> f32 {
    let scale = terrain_vertical_scale.max(1.0);
    let mut lifted = anchor_y;
    for sample in &terrain.samples {
        let base_sim =
            base_height_on_plane_at(anchor_xz, lifted, simulation_up, sample.global_xz);
        let base_render = render_height(base_sim, scale);
        let terrain_render = render_height(sample.height, scale);
        let gap = terrain_render + PRESENTATION_TERRAIN_CLEARANCE_METERS - base_render;
        if gap > 0.0 {
            lifted += gap / scale;
        }
    }
    lifted
}

fn world_position_at_height(
    _world: &WorldData,
    layout: ChunkLayout,
    anchor_xz: Vec2,
    height: f32,
) -> Result<WorldPosition, BuildingPlacementRejectReason> {
    let candidate = WorldPosition::from_global(Vec3::new(anchor_xz.x, height, anchor_xz.y), layout);
    if candidate.to_global(layout).is_finite() {
        Ok(candidate)
    } else {
        Err(BuildingPlacementRejectReason::TerrainUnavailable)
    }
}

/// Re-derive a level foundation skirt for a placed building (presentation only).
pub fn derive_foundation_skirt_for_placement(
    world: &WorldData,
    layout: ChunkLayout,
    definition: &BuildingDefinition,
    footprint_catalog: &FootprintCatalog,
    placement: &crate::world::building::placement::BuildingPlacement,
    terrain_vertical_scale: f32,
) -> Option<FoundationSkirtSpec> {
    if definition.terrain_placement_mode != TerrainPlacementMode::LevelFoundation {
        return None;
    }
    let quantized_yaw = QuantizedRotation::yaw_for_occupancy(placement.rotation).ok()?;
    let shape = effective_building_footprint_for_placement(
        definition,
        footprint_catalog,
        placement.uniform_scale_f32(),
    )
    .ok()?;
    let anchor_global = placement.position.to_global(layout);
    let anchor_xz = Vec2::new(anchor_global.x, anchor_global.z);
    let terrain = sample_terrain_under_footprint(
        world,
        layout,
        shape.as_ref(),
        anchor_xz,
        quantized_yaw.radians(),
    )
    .ok()?;
    let floor_world_y = placement.position.to_global(layout).y;
    let footprint_span = footprint_horizontal_span_meters(shape.as_ref());
    FoundationSkirtSpec::from_level_placement(
        anchor_xz,
        floor_world_y,
        &terrain.perimeter,
        terrain.min_height,
        quantized_yaw.radians(),
        &terrain,
        footprint_span,
        terrain_vertical_scale,
    )
}

/// Preserve player yaw while aligning local +Y to terrain normal.
pub fn rotation_from_yaw_and_normal(yaw_radians: f32, normal: Vec3) -> Quat {
    let up = normal.normalize_or_zero();
    if up.length_squared() < 1e-8 {
        return Quat::from_rotation_y(yaw_radians);
    }
    let yaw_forward = Vec3::new(yaw_radians.sin(), 0.0, -yaw_radians.cos());
    let mut forward = yaw_forward - up * yaw_forward.dot(up);
    if forward.length_squared() < 1e-8 {
        forward = Vec3::new(1.0, 0.0, 0.0) - up * up.x;
    }
    forward = forward.normalize_or_zero();
    let right = forward.cross(up).normalize_or_zero();
    forward = up.cross(right).normalize_or_zero();
    Quat::from_mat3(&Mat3::from_cols(right, up, -forward))
}
