//! StarCraft II-style unit selection and move commands (ADR-033 U8).

use bevy::input::mouse::MouseButton;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::terrain::TerrainRenderAssets;
use crate::units::UnitRenderEntity;
use crate::world::{
    issue_unit_order, DoodadCatalog, NavigationConfig, NavigationPath, UnitCatalog, UnitOrder,
    UnitOrderError, UnitState, WorldConfig, WorldData, xz_distance,
};

use super::pick::{cursor_world_ray, pick_unit_along_ray};
use super::selection::PlayerUnitSelection;
use super::settings::PlayerInteractionSettings;
use super::terrain_click::terrain_click_to_world_position;

/// Handle left/right mouse unit interaction (SC2 baseline).
pub fn handle_player_unit_input(
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform), With<crate::camera::RtsCamera>>,
    settings: Res<PlayerInteractionSettings>,
    mut selection: ResMut<PlayerUnitSelection>,
    mut world: ResMut<WorldData>,
    config: Res<WorldConfig>,
    unit_catalog: Res<UnitCatalog>,
    doodad_catalog: Res<DoodadCatalog>,
    nav_config: Res<NavigationConfig>,
    render_assets: Option<Res<TerrainRenderAssets>>,
    units: Query<(&UnitRenderEntity, &GlobalTransform)>,
) {
    let Some(ray) = cursor_world_ray(&windows, &camera) else {
        return;
    };

    let layout = config.chunk_layout();
    let vertical_scale = render_assets
        .as_ref()
        .map(|assets| assets.vertical_scale)
        .unwrap_or(1.0);

    if mouse_buttons.just_pressed(MouseButton::Left) {
        if let Some(unit_id) = pick_unit_along_ray(&ray, &world, &unit_catalog, &units) {
            selection.select(unit_id);
            return;
        }

        if terrain_click_to_world_position(&ray, &world, layout, vertical_scale).is_some() {
            selection.clear();
        }
    }

    if mouse_buttons.just_pressed(MouseButton::Right) {
        let Some(selected) = selection.selected else {
            return;
        };
        let Some(click) = terrain_click_to_world_position(&ray, &world, layout, vertical_scale)
        else {
            return;
        };
        let target = click.world_position;

        if settings.debug_unit_interaction {
            log_move_click(&click.render_hit, &target, layout);
        }

        let start = world.get_unit(selected).map(|record| record.placement.position);
        if let Err(error) = issue_unit_order(
            &mut world,
            &unit_catalog,
            &doodad_catalog,
            &nav_config,
            selected,
            UnitOrder::MoveTo { target },
        ) {
            log_move_order_failure(selected, error);
            return;
        }

        if settings.debug_unit_interaction {
            if let (Some(start), Some(record)) = (start, world.get_unit(selected)) {
                if let UnitState::Moving { ref path, .. } = record.state {
                    log_generated_path(start, target, path, layout);
                }
            }
        }
    }
}

fn log_move_click(
    render_hit: &Vec3,
    target: &crate::world::WorldPosition,
    layout: crate::world::ChunkLayout,
) {
    info!(
        "move click render_hit=({:.2}, {:.2}, {:.2}) target chunk=({}, {}) local=({:.2}, {:.2}) grounded_y={:.2}",
        render_hit.x,
        render_hit.y,
        render_hit.z,
        target.chunk.x,
        target.chunk.z,
        target.local.0.x,
        target.local.0.z,
        target.local.0.y,
    );
    let global = target.to_global(layout);
    info!(
        "move click authoritative_global=({:.2}, {:.2}, {:.2})",
        global.x, global.y, global.z
    );
}

fn log_generated_path(
    start: crate::world::WorldPosition,
    goal: crate::world::WorldPosition,
    path: &NavigationPath,
    layout: crate::world::ChunkLayout,
) {
    let straight = xz_distance(start, goal, layout);
    let path_len = path.length_meters(layout);
    let ratio = if straight > 1e-4 {
        path_len / straight
    } else {
        1.0
    };
    let first = path.waypoints.first().copied();
    let last = path.waypoints.last().copied();
    info!(
        "path start=({:.2}, {:.2}) goal=({:.2}, {:.2}) waypoints={} first={:?} last={:?} length={:.2} straight={:.2} ratio={:.3}",
        start.to_global(layout).x,
        start.to_global(layout).z,
        goal.to_global(layout).x,
        goal.to_global(layout).z,
        path.len(),
        first.map(|p| p.to_global(layout)),
        last.map(|p| p.to_global(layout)),
        path_len,
        straight,
        ratio,
    );
}

fn log_move_order_failure(unit_id: crate::world::UnitId, error: UnitOrderError) {
    match error {
        UnitOrderError::NoPath => {
            warn!("move order for unit {} failed: no path", unit_id.raw());
        }
        UnitOrderError::PathGoalBlocked | UnitOrderError::PathStartBlocked => {
            warn!("move order for unit {} failed: blocked", unit_id.raw());
        }
        UnitOrderError::PathTerrainUnavailable => {
            warn!(
                "move order for unit {} failed: terrain unavailable",
                unit_id.raw()
            );
        }
        UnitOrderError::UnitNotFound => {}
        UnitOrderError::DefinitionNotFound => {
            warn!(
                "move order for unit {} failed: missing definition",
                unit_id.raw()
            );
        }
    }
}
