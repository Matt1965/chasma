//! Build mode ghost validation and anchor updates (ADR-081 B4).

use bevy::prelude::*;

use crate::camera::RtsCamera;
use crate::terrain::TerrainRenderAssets;
use crate::units::input::{cursor_world_ray, terrain_click_to_world_position};
use crate::world::{
    BuildingCatalog, BuildingOwnership, BuildingPlacementConfig, BuildingPlacementContext,
    DoodadCatalog, FootprintCatalog, UnitCatalog, WorldConfig, WorldData,
    anchor_from_terrain_position, build_building_placement_plan, rotation_from_quadrants,
    validate_building_placement,
};

use super::state::BuildModeState;

/// Client-local terrain anchor under cursor for ghost preview.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Default)]
pub struct BuildModeCursorAnchor {
    pub position: Option<crate::world::WorldPosition>,
}

/// Update sample authoritative terrain anchor and validate the armed ghost.
pub fn update_build_mode_ghost(
    mut build_mode: ResMut<BuildModeState>,
    mut anchor: ResMut<BuildModeCursorAnchor>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform), With<RtsCamera>>,
    world: Res<WorldData>,
    config: Res<WorldConfig>,
    render_assets: Option<Res<TerrainRenderAssets>>,
    building_catalog: Res<BuildingCatalog>,
    footprint_catalog: Res<FootprintCatalog>,
    doodad_catalog: Res<DoodadCatalog>,
    unit_catalog: Res<UnitCatalog>,
) {
    anchor.position = None;
    if !build_mode.is_ghost_placing() {
        build_mode.last_validation = None;
        build_mode.last_plan = None;
        return;
    }

    let Some(definition_id) = build_mode.ghost_definition_id().cloned() else {
        return;
    };

    let Some(ray) = cursor_world_ray(&windows, &camera) else {
        build_mode.last_validation = Some(crate::world::BuildingPlacementValidation::rejected(
            crate::world::BuildingPlacementRejectReason::TerrainUnavailable,
        ));
        return;
    };

    let layout = config.chunk_layout();
    let vertical_scale = render_assets
        .as_ref()
        .map(|assets| assets.vertical_scale)
        .unwrap_or(1.0);
    let click = terrain_click_to_world_position(&ray, &world, layout, vertical_scale);
    let terrain_pos = click.map(|c| c.world_position);
    let snapped = terrain_pos.and_then(|pos| anchor_from_terrain_position(&world, pos));
    anchor.position = snapped;

    let rotation = rotation_from_quadrants(build_mode.ghost_rotation_quadrants());
    let ownership = BuildingOwnership::with_affiliation(crate::world::Affiliation::Player);
    let ctx = BuildingPlacementContext {
        world: &world,
        building_catalog: &building_catalog,
        footprint_catalog: &footprint_catalog,
        doodad_catalog: &doodad_catalog,
        unit_catalog: &unit_catalog,
        config: BuildingPlacementConfig::default(),
        player_authorized: true,
    };
    let validation = if let Some(pos) = snapped {
        validate_building_placement(&ctx, &definition_id, pos, rotation, ownership)
    } else {
        crate::world::BuildingPlacementValidation::rejected(
            crate::world::BuildingPlacementRejectReason::TerrainUnavailable,
        )
    };
    build_mode.last_plan = if snapped.is_some() {
        Some(build_building_placement_plan(
            &ctx,
            &definition_id,
            snapped.unwrap(),
            rotation,
            ownership,
        ))
    } else {
        None
    };
    build_mode.last_validation = Some(validation);
}
