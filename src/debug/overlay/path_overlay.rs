//! Path debug overlay — active navigation paths and current segment.

use bevy::prelude::*;

use crate::debug::settings::{DebugOverlayCategory, DebugOverlaySettings};
use crate::terrain::TerrainRenderAssets;
use crate::units::input::SelectedUnits;
use crate::world::{UnitState, WorldConfig, WorldData, WorldPosition};

use super::helpers::{render_position, xz_to_render_y};

/// Draw waypoint polylines and highlight the active segment for selected units.
pub fn draw_path_debug_overlay(
    mut gizmos: Gizmos,
    world: Res<WorldData>,
    config: Res<WorldConfig>,
    selection: Res<SelectedUnits>,
    settings: Res<DebugOverlaySettings>,
    interaction_settings: Res<crate::units::input::PlayerInteractionSettings>,
    render_assets: Option<Res<TerrainRenderAssets>>,
) {
    if !settings.category_enabled(DebugOverlayCategory::Path)
        && !interaction_settings.debug_unit_interaction
    {
        return;
    }

    let layout = config.chunk_layout();
    let vertical_scale = render_assets
        .as_ref()
        .map(|assets| assets.vertical_scale)
        .unwrap_or(1.0);
    let mut drawn = 0_u32;

    for unit_id in selection.iter() {
        if drawn >= settings.max_draw_units {
            break;
        }
        let Some(record) = world.get_unit(unit_id) else {
            continue;
        };
        let UnitState::Moving {
            ref path,
            waypoint_index,
            ..
        } = record.state
        else {
            continue;
        };

        let mut points: Vec<Vec3> = path
            .waypoints
            .iter()
            .map(|waypoint| render_position(*waypoint, layout, vertical_scale))
            .collect();
        if points.is_empty() {
            continue;
        }
        points.insert(
            0,
            render_position(record.placement.position, layout, vertical_scale),
        );

        for window in points.windows(2) {
            gizmos.line(
                xz_to_render_y(window[0], 0.15),
                xz_to_render_y(window[1], 0.15),
                Color::srgba(0.2, 1.0, 0.35, 0.85),
            );
        }

        if let (Some(start), Some(end)) = (
            active_segment_start(record.placement.position, path, waypoint_index, layout, vertical_scale),
            active_segment_end(path, waypoint_index, layout, vertical_scale),
        ) {
            gizmos.line(
                xz_to_render_y(start, 0.22),
                xz_to_render_y(end, 0.22),
                Color::srgba(1.0, 0.95, 0.2, 0.95),
            );
        }

        drawn += 1;
    }
}

fn active_segment_start(
    unit_position: WorldPosition,
    path: &crate::world::NavigationPath,
    waypoint_index: usize,
    layout: crate::world::ChunkLayout,
    vertical_scale: f32,
) -> Option<Vec3> {
    if waypoint_index == 0 {
        Some(render_position(unit_position, layout, vertical_scale))
    } else {
        path.waypoints
            .get(waypoint_index.saturating_sub(1) as usize)
            .copied()
            .map(|waypoint| render_position(waypoint, layout, vertical_scale))
    }
}

fn active_segment_end(
    path: &crate::world::NavigationPath,
    waypoint_index: usize,
    layout: crate::world::ChunkLayout,
    vertical_scale: f32,
) -> Option<Vec3> {
    path.waypoints
        .get(waypoint_index as usize)
        .copied()
        .map(|waypoint| render_position(waypoint, layout, vertical_scale))
}
