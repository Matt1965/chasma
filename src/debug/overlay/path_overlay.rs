//! Path debug overlay — active and retained navigation paths (IN-11eO).

use bevy::prelude::*;

use crate::client::selection::WorldSelectionState;
use crate::debug::InspectorOverlayFocus;
use crate::debug::path_trace::{PathTraceStatus, RetainedUnitPath, UnitPathDiagnosticStore};
use crate::debug::settings::{DebugOverlayCategory, DebugOverlaySettings};
use crate::terrain::TerrainRenderAssets;
use crate::units::input::SelectedUnits;
use crate::world::{NavigationPath, UnitState, WorldConfig, WorldData, WorldPosition};

use super::helpers::{render_position, xz_to_render_y};

/// Draw waypoint polylines and highlight the active segment for selected units.
pub fn draw_path_debug_overlay(
    mut gizmos: Gizmos,
    world: Res<WorldData>,
    config: Res<WorldConfig>,
    selection: Res<SelectedUnits>,
    world_selection: Res<WorldSelectionState>,
    settings: Res<DebugOverlaySettings>,
    focus: Res<InspectorOverlayFocus>,
    path_store: Res<UnitPathDiagnosticStore>,
    render_assets: Option<Res<TerrainRenderAssets>>,
) {
    if !settings.category_enabled(DebugOverlayCategory::Path) {
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
        if let Some(trace) = path_store.latest_for_unit(unit_id) {
            draw_path_trace(&mut gizmos, &world, trace, layout, vertical_scale);
            drawn += 1;
            continue;
        }
        if let Some(record) = world.get_unit(unit_id) {
            if let UnitState::Moving {
                path,
                waypoint_index,
                target,
                ..
            } = &record.state
            {
                let trace = RetainedUnitPath {
                    authority_sequence: 0,
                    sequence: 0,
                    unit_id,
                    start: record.placement.position,
                    goal: *target,
                    start_space: record.current_space_id,
                    goal_space: record.current_space_id,
                    path: path.clone(),
                    waypoint_index: *waypoint_index,
                    status: PathTraceStatus::Active,
                    failure_reason: None,
                    blocked_position: None,
                    blocked_reason: None,
                };
                draw_path_trace(&mut gizmos, &world, &trace, layout, vertical_scale);
                drawn += 1;
            }
        }
    }

    if drawn == 0 {
        let unit_id = world_selection
            .primary_unit(&selection)
            .or_else(|| selection.iter().next());
        if let Some(unit_id) = unit_id {
            if let Some(trace) = path_store.latest_for_unit(unit_id) {
                draw_path_trace(&mut gizmos, &world, trace, layout, vertical_scale);
            }
        }
    }

    if let Some(focus_id) = focus.unit_id {
        if focus.is_focused(focus_id) && !selection.contains(focus_id) {
            draw_focus_path(
                &mut gizmos,
                &world,
                focus_id,
                focus.path_waypoint_index,
                layout,
                vertical_scale,
            );
        }
    }
}

fn draw_path_trace(
    gizmos: &mut Gizmos,
    world: &WorldData,
    trace: &RetainedUnitPath,
    layout: crate::world::ChunkLayout,
    vertical_scale: f32,
) {
    let path = &trace.path;
    let waypoint_index = trace.waypoint_index;
    let completed =
        trace.status == PathTraceStatus::Completed || trace.status == PathTraceStatus::Failed;

    let mut points: Vec<Vec3> = path
        .waypoints
        .iter()
        .map(|waypoint| render_position(waypoint.position, layout, vertical_scale))
        .collect();
    if points.is_empty() {
        return;
    }
    points.insert(0, render_position(trace.start, layout, vertical_scale));

    for window in points.windows(2) {
        gizmos.line(
            xz_to_render_y(window[0], 0.08),
            xz_to_render_y(window[1], 0.08),
            Color::srgba(0.2, 1.0, 0.35, 0.25),
        );
    }

    for (index, window) in points.windows(2).enumerate() {
        let segment_index = index;
        let is_current = !completed && segment_index == waypoint_index;
        let is_past = completed || segment_index < waypoint_index;
        let alpha = if is_current {
            0.95
        } else if is_past {
            0.35
        } else {
            0.75
        };
        let color = if trace.status == PathTraceStatus::Failed
            && trace.blocked_position.is_some()
            && segment_index + 1 == waypoint_index
        {
            Color::srgba(1.0, 0.2, 0.2, 0.95)
        } else if is_current {
            Color::srgba(1.0, 0.95, 0.2, alpha)
        } else if is_past {
            Color::srgba(0.35, 0.9, 0.95, alpha)
        } else {
            Color::srgba(0.2, 1.0, 0.35, alpha)
        };
        gizmos.line(
            xz_to_render_y(window[0], 0.12),
            xz_to_render_y(window[1], 0.12),
            color,
        );
    }

    if let Some(start) = points.first() {
        gizmos.sphere(
            xz_to_render_y(*start, 0.2),
            0.28,
            Color::srgba(0.25, 0.55, 1.0, 0.9),
        );
    }
    let goal_pos = render_position(trace.goal, layout, vertical_scale);
    gizmos.sphere(
        xz_to_render_y(goal_pos, 0.25),
        0.32,
        Color::srgba(1.0, 0.25, 0.2, 0.95),
    );

    for (index, waypoint) in path.waypoints.iter().enumerate() {
        let pos = render_position(waypoint.position, layout, vertical_scale);
        let color = if waypoint.portal_id.is_some() {
            Color::srgba(0.85, 0.35, 1.0, 0.9)
        } else if index == waypoint_index && !completed {
            Color::srgba(1.0, 0.95, 0.2, 0.95)
        } else if index < waypoint_index || completed {
            Color::srgba(0.35, 0.9, 0.95, 0.45)
        } else {
            Color::srgba(0.35, 0.9, 0.95, 0.75)
        };
        gizmos.sphere(xz_to_render_y(pos, 0.18), 0.18, color);

        if let Some(portal_id) = waypoint.portal_id {
            if let Some(portal) = world.space_registry().get_portal(portal_id) {
                let center = Vec3::new(
                    portal.from_center_global_xz.x,
                    portal.to_position.to_global(layout).y.max(0.15),
                    portal.from_center_global_xz.y,
                );
                gizmos.circle(
                    xz_to_render_y(center, 0.3),
                    portal.from_radius_meters,
                    Color::srgba(0.95, 0.35, 1.0, 0.85),
                );
            }
        }
    }

    if let Some(blocked) = trace.blocked_position {
        let pos = render_position(blocked, layout, vertical_scale);
        gizmos.sphere(
            xz_to_render_y(pos, 0.28),
            0.35,
            Color::srgba(1.0, 0.15, 0.15, 0.95),
        );
    }

    if let (Some(start), Some(end)) = (
        active_segment_start(trace.start, path, waypoint_index, layout, vertical_scale),
        active_segment_end(path, waypoint_index, layout, vertical_scale),
    ) && !completed
    {
        gizmos.line(
            xz_to_render_y(start, 0.22),
            xz_to_render_y(end, 0.22),
            Color::srgba(1.0, 0.95, 0.2, 0.95),
        );
    }
}

fn draw_focus_path(
    gizmos: &mut Gizmos,
    world: &WorldData,
    unit_id: crate::world::UnitId,
    highlight_index: Option<usize>,
    layout: crate::world::ChunkLayout,
    vertical_scale: f32,
) {
    let Some(record) = world.get_unit(unit_id) else {
        return;
    };
    let UnitState::Moving {
        ref path,
        waypoint_index,
        ..
    } = record.state
    else {
        return;
    };
    let idx = highlight_index.unwrap_or(waypoint_index);
    if let Some(waypoint) = path.waypoints.get(idx) {
        let center = xz_to_render_y(
            render_position(waypoint.position, layout, vertical_scale),
            0.35,
        );
        gizmos.sphere(center, 0.35, Color::srgba(1.0, 0.55, 0.1, 0.95));
    }
}

fn active_segment_start(
    unit_position: WorldPosition,
    path: &NavigationPath,
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
            .map(|waypoint| render_position(waypoint.position, layout, vertical_scale))
    }
}

fn active_segment_end(
    path: &NavigationPath,
    waypoint_index: usize,
    layout: crate::world::ChunkLayout,
    vertical_scale: f32,
) -> Option<Vec3> {
    path.waypoints
        .get(waypoint_index as usize)
        .copied()
        .map(|waypoint| render_position(waypoint.position, layout, vertical_scale))
}
