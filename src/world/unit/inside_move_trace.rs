//! Bounded dev diagnostic trace for inside-region player move commands (IN-11gG-T).
//!
//! One coherent log per player move attempt; does not mutate navigation behavior.

use bevy::prelude::*;

use super::id::UnitId;
use crate::world::InteractionType;
use crate::world::{
    BuildingNavigationRuntimeStore, ChunkLayout, SpaceId, SpaceRegistry, UnitCatalog, WorldData,
    WorldPosition, resolve_navigation_space_at_position,
};

const FLOOR_ELEVATION_TOLERANCE_METERS: f32 = 1.5;

/// One bounded inside-move diagnostic session.
#[derive(Debug, Clone, Default)]
pub struct InsideMoveTrace {
    active: Option<InsideMoveSession>,
}

#[derive(Debug, Clone)]
pub struct InsideMoveSession {
    pub unit_id: UnitId,
    // Event 1 — unit state before click resolution
    pub unit_position: WorldPosition,
    pub tracked_space: SpaceId,
    pub positional_space: SpaceId,
    pub inside_runtime_region: bool,
    pub region_label: Option<String>,
    pub region_floor_y: Option<f32>,
    pub collision_radius_meters: f32,
    // Event 2 — click resolution
    pub click_raw: Option<WorldPosition>,
    pub click_terrain_grounded: Option<WorldPosition>,
    pub interior_click_attempted: bool,
    pub interior_click_skipped_reason: Option<String>,
    pub interior_nav_move_target_space: Option<SpaceId>,
    pub interior_floor_y_delta: Option<f32>,
    pub interaction_type: Option<String>,
    pub interaction_valid: Option<bool>,
    pub resolved_order: Option<String>,
    // Event 3 — order issuance
    pub order_issued: Option<bool>,
    pub order_target: Option<WorldPosition>,
    pub order_error: Option<String>,
    // Event 4 — path resolution
    pub path_tracked_space_before: Option<SpaceId>,
    pub path_resolved_start_space: Option<SpaceId>,
    pub path_resolved_goal_space: Option<SpaceId>,
    pub path_cross_space: Option<bool>,
    pub path_result: Option<String>,
    pub path_waypoint_count: Option<u32>,
    // Event 5 — first movement step
    pub move_current_space: Option<SpaceId>,
    pub move_position: Option<WorldPosition>,
    pub move_waypoint_position: Option<WorldPosition>,
    pub move_waypoint_space: Option<SpaceId>,
    pub move_point_legality: Option<String>,
    pub move_segment_legality: Option<String>,
    pub move_grounding_ok: Option<bool>,
    pub move_position_changed: Option<bool>,
    pub move_block_reason: Option<String>,
    pub first_failure: Option<String>,
    pub emitted: bool,
}

impl InsideMoveTrace {
    pub fn is_active_for(&self, unit_id: UnitId) -> bool {
        self.active.as_ref().is_some_and(|s| s.unit_id == unit_id)
    }

    pub fn clear_active(&mut self) {
        self.active = None;
    }
}

#[cfg(feature = "dev")]
pub fn maybe_begin_session(
    world: &mut WorldData,
    unit_id: UnitId,
    raw_click: WorldPosition,
    unit_catalog: &UnitCatalog,
) {
    let record = match world.get_unit(unit_id) {
        Some(record) => record,
        None => return,
    };
    let layout = world.layout();
    let position = record.placement.position;
    let tracked_space = record.current_space_id;
    let runtime = world.building_navigation_runtime();
    let registry = world.space_registry();
    let positional_space =
        resolve_navigation_space_at_position(runtime, registry, layout, position);
    let region_probe = probe_runtime_region(runtime, registry, layout, position);
    let collision_radius_meters = unit_catalog
        .get(&record.definition_id)
        .map(|def| def.collision_radius_meters)
        .unwrap_or(0.0);

    world.inside_move_trace_mut().active = Some(InsideMoveSession {
        unit_id,
        unit_position: position,
        tracked_space,
        positional_space,
        inside_runtime_region: region_probe.inside,
        region_label: region_probe.label,
        region_floor_y: region_probe.floor_y,
        collision_radius_meters,
        click_raw: Some(raw_click),
        click_terrain_grounded: None,
        interior_click_attempted: false,
        interior_click_skipped_reason: None,
        interior_nav_move_target_space: None,
        interior_floor_y_delta: None,
        interaction_type: None,
        interaction_valid: None,
        resolved_order: None,
        order_issued: None,
        order_target: None,
        order_error: None,
        path_tracked_space_before: None,
        path_resolved_start_space: None,
        path_resolved_goal_space: None,
        path_cross_space: None,
        path_result: None,
        path_waypoint_count: None,
        move_current_space: None,
        move_position: None,
        move_waypoint_position: None,
        move_waypoint_space: None,
        move_point_legality: None,
        move_segment_legality: None,
        move_grounding_ok: None,
        move_position_changed: Option::None,
        move_block_reason: None,
        first_failure: None,
        emitted: false,
    });
}

#[cfg(feature = "dev")]
struct RegionProbe {
    inside: bool,
    label: Option<String>,
    floor_y: Option<f32>,
}

#[cfg(feature = "dev")]
fn probe_runtime_region(
    store: &BuildingNavigationRuntimeStore,
    registry: &SpaceRegistry,
    layout: ChunkLayout,
    position: WorldPosition,
) -> RegionProbe {
    let global = position.to_global(layout);
    let point = global.xz();
    for runtime in store.iter() {
        for region in &runtime.regions {
            if !crate::world::point_in_polygon_xz(&region.world_outline_xz, point) {
                continue;
            }
            let floor_y = registry
                .get_space(region.space_id)
                .map(|space| space.floor_y_global)
                .unwrap_or(region.elevation_meters);
            let label = format!(
                "building#{} {}/{}",
                runtime.building_id.raw(),
                region.floor_key,
                region.region_key
            );
            return RegionProbe {
                inside: true,
                label: Some(label),
                floor_y: Some(floor_y),
            };
        }
    }
    RegionProbe {
        inside: false,
        label: None,
        floor_y: None,
    }
}

#[cfg(feature = "dev")]
pub fn record_interior_click_skipped(world: &mut WorldData, unit_id: UnitId, reason: &str) {
    let session = world.inside_move_trace_mut().active.as_mut();
    if session.as_ref().is_none_or(|s| s.unit_id != unit_id) {
        return;
    }
    let session = session.unwrap();
    session.interior_click_attempted = false;
    session.interior_click_skipped_reason = Some(reason.to_string());
}

#[cfg(feature = "dev")]
pub fn record_interior_click_attempt(world: &mut WorldData, unit_id: UnitId) {
    let session = world.inside_move_trace_mut().active.as_mut();
    if session.as_ref().is_none_or(|s| s.unit_id != unit_id) {
        return;
    }
    session.unwrap().interior_click_attempted = true;
}

#[cfg(feature = "dev")]
pub fn record_click_terrain_grounded(
    world: &mut WorldData,
    unit_id: UnitId,
    grounded: WorldPosition,
) {
    let session = world.inside_move_trace_mut().active.as_mut();
    if session.as_ref().is_none_or(|s| s.unit_id != unit_id) {
        return;
    }
    session.unwrap().click_terrain_grounded = Some(grounded);
}

#[cfg(feature = "dev")]
pub fn record_interior_nav_probe(
    world: &mut WorldData,
    unit_id: UnitId,
    probe_position: WorldPosition,
) {
    if !world.inside_move_trace().is_active_for(unit_id) {
        return;
    }
    let layout = world.layout();
    let registry = world.space_registry();
    let runtime = world.building_navigation_runtime();
    let space = crate::world::interior_navigation_move_target_at_position(
        runtime,
        registry,
        layout,
        probe_position,
    );
    let global = probe_position.to_global(layout);
    let floor_y_delta = space.and_then(|space_id| {
        registry
            .get_space(space_id)
            .map(|space| (global.y - space.floor_y_global).abs())
    });
    let session = world.inside_move_trace_mut().active.as_mut();
    if session.as_ref().is_none_or(|s| s.unit_id != unit_id) {
        return;
    }
    let session = session.unwrap();
    session.interior_nav_move_target_space = space;
    session.interior_floor_y_delta = floor_y_delta;
}

#[cfg(feature = "dev")]
pub fn record_interaction_query(
    world: &mut WorldData,
    unit_id: UnitId,
    interaction_type: InteractionType,
    valid: bool,
) {
    let session = world.inside_move_trace_mut().active.as_mut();
    if session.as_ref().is_none_or(|s| s.unit_id != unit_id) {
        return;
    }
    let session = session.unwrap();
    session.interaction_type = Some(interaction_type.label().to_string());
    session.interaction_valid = Some(valid);
}

#[cfg(feature = "dev")]
pub fn record_resolved_order_plan(world: &mut WorldData, unit_id: UnitId, plan_label: &str) {
    let session = world.inside_move_trace_mut().active.as_mut();
    if session.as_ref().is_none_or(|s| s.unit_id != unit_id) {
        return;
    }
    session.unwrap().resolved_order = Some(plan_label.to_string());
}

#[cfg(feature = "dev")]
pub fn finish_command_resolution_failure(world: &mut WorldData, unit_id: UnitId, detail: &str) {
    let session = world.inside_move_trace_mut().active.as_mut();
    if session.as_ref().is_none_or(|s| s.unit_id != unit_id) {
        return;
    }
    let session = session.unwrap();
    session.first_failure = Some(format!("COMMAND_RESOLUTION: {detail}"));
    emit_session(world, unit_id);
}

#[cfg(feature = "dev")]
pub fn record_order_issuance(
    world: &mut WorldData,
    unit_id: UnitId,
    issued: bool,
    target: Option<WorldPosition>,
    error: Option<String>,
) {
    let session = world.inside_move_trace_mut().active.as_mut();
    if session.as_ref().is_none_or(|s| s.unit_id != unit_id) {
        return;
    }
    let session = session.unwrap();
    session.order_issued = Some(issued);
    session.order_target = target;
    session.order_error = error;
    if !issued {
        session.first_failure = Some(format!(
            "ORDER_ISSUANCE: {}",
            session.order_error.as_deref().unwrap_or("not issued")
        ));
        emit_session(world, unit_id);
    }
}

#[cfg(feature = "dev")]
pub fn record_path_resolution(
    world: &mut WorldData,
    unit_id: UnitId,
    tracked_before: SpaceId,
    start_space: SpaceId,
    goal_space: SpaceId,
    result: &str,
    waypoint_count: Option<u32>,
) {
    let session = world.inside_move_trace_mut().active.as_mut();
    if session.as_ref().is_none_or(|s| s.unit_id != unit_id) {
        return;
    }
    let session = session.unwrap();
    session.path_tracked_space_before = Some(tracked_before);
    session.path_resolved_start_space = Some(start_space);
    session.path_resolved_goal_space = Some(goal_space);
    session.path_cross_space = Some(start_space != goal_space);
    session.path_result = Some(result.to_string());
    session.path_waypoint_count = waypoint_count;
    if result != "success" {
        session.first_failure = Some(format!("ORDER_PATH_RESOLUTION: {result}"));
        emit_session(world, unit_id);
    }
}

#[cfg(feature = "dev")]
pub fn record_first_movement_step(
    world: &mut WorldData,
    unit_id: UnitId,
    current_space: SpaceId,
    position: WorldPosition,
    waypoint_position: WorldPosition,
    waypoint_space: SpaceId,
    point_legality: &str,
    segment_legality: &str,
    grounding_ok: bool,
    position_changed: bool,
    block_reason: Option<String>,
) {
    let session = world.inside_move_trace_mut().active.as_mut();
    if session.as_ref().is_none_or(|s| s.unit_id != unit_id) {
        return;
    }
    let session = session.unwrap();
    if session.move_position.is_some() {
        return;
    }
    session.move_current_space = Some(current_space);
    session.move_position = Some(position);
    session.move_waypoint_position = Some(waypoint_position);
    session.move_waypoint_space = Some(waypoint_space);
    session.move_point_legality = Some(point_legality.to_string());
    session.move_segment_legality = Some(segment_legality.to_string());
    session.move_grounding_ok = Some(grounding_ok);
    session.move_position_changed = Some(position_changed);
    session.move_block_reason = block_reason.clone();
    if block_reason.is_some() {
        session.first_failure = Some(format!(
            "MOVEMENT_STEP: {}",
            block_reason.unwrap_or_default()
        ));
    }
    emit_session(world, unit_id);
}

#[cfg(feature = "dev")]
fn emit_session(world: &mut WorldData, unit_id: UnitId) {
    let trace = world.inside_move_trace_mut();
    let session = match trace.active.as_ref() {
        Some(s) if s.unit_id == unit_id => s.clone(),
        _ => return,
    };
    if session.emitted {
        return;
    }
    if let Some(active) = trace.active.as_mut() {
        active.emitted = true;
    }
    crate::logging::append_log_block(
        crate::logging::NAVIGATION_TRACE_LOG_PATH,
        "# chasma navigation trace",
        &format_session_log(&session),
    );
    trace.clear_active();
}

#[cfg(feature = "dev")]
fn format_session_log(session: &InsideMoveSession) -> String {
    let layout = ChunkLayout {
        chunk_size_meters: 256.0,
        units_per_meter: 1.0,
    };
    let pos = session.unit_position.to_global(layout);
    let click_raw = session
        .click_raw
        .map(|p| format_global(p, layout))
        .unwrap_or_else(|| "none".to_string());
    let click_grounded = session
        .click_terrain_grounded
        .map(|p| format_global(p, layout))
        .unwrap_or_else(|| "none".to_string());
    let interior_attempted = if session.interior_click_attempted {
        "true"
    } else {
        "false"
    };
    let skip_reason = session
        .interior_click_skipped_reason
        .as_deref()
        .unwrap_or("none");
    let interior_space = session
        .interior_nav_move_target_space
        .map(|s| s.raw().to_string())
        .unwrap_or_else(|| "none".to_string());
    let floor_delta = session
        .interior_floor_y_delta
        .map(|d| format!("{d:.3}"))
        .unwrap_or_else(|| "none".to_string());
    let interaction = session.interaction_type.as_deref().unwrap_or("none");
    let resolved_order = session.resolved_order.as_deref().unwrap_or("none");
    let first_failure = session.first_failure.as_deref().unwrap_or("none");
    let mut lines = vec![
        "[INSIDE_MOVE_TRACE]".to_string(),
        format!("unit=U-{:04}", session.unit_id.raw()),
        format!("position=({:.2},{:.2},{:.2})", pos.x, pos.y, pos.z),
        format!("tracked_space={}", session.tracked_space.raw()),
        format!("positional_space={}", session.positional_space.raw()),
        format!("inside_runtime_region={}", session.inside_runtime_region),
        format!(
            "region_label={}",
            session.region_label.as_deref().unwrap_or("none")
        ),
        format!(
            "region_floor_y={}",
            session
                .region_floor_y
                .map(|y| format!("{y:.3}"))
                .unwrap_or_else(|| "none".to_string())
        ),
        format!("collision_radius_m={:.3}", session.collision_radius_meters),
        format!("click_raw={click_raw}"),
        format!("click_grounded={click_grounded}"),
        format!("interior_click_attempted={interior_attempted}"),
        format!("interior_click_skipped_reason={skip_reason}"),
        format!("interior_nav_move_target_space={interior_space}"),
        format!("interior_floor_y_delta={floor_delta}"),
        format!("interaction={interaction}"),
        format!("resolved_order={resolved_order}"),
        format!("FIRST_FAILURE={first_failure}"),
    ];
    if session.order_issued.is_some() {
        lines.push(format!("order_issued={}", session.order_issued.unwrap()));
        if let Some(target) = session.order_target {
            lines.push(format!("order_target={}", format_global(target, layout)));
        }
        if let Some(err) = &session.order_error {
            lines.push(format!("order_error={err}"));
        }
    }
    if session.path_result.is_some() {
        lines.push(format!(
            "path_tracked_before={}",
            session
                .path_tracked_space_before
                .map(|s| s.raw().to_string())
                .unwrap_or_else(|| "none".to_string())
        ));
        lines.push(format!(
            "start_space={}",
            session
                .path_resolved_start_space
                .map(|s| s.raw().to_string())
                .unwrap_or_else(|| "none".to_string())
        ));
        lines.push(format!(
            "goal_space={}",
            session
                .path_resolved_goal_space
                .map(|s| s.raw().to_string())
                .unwrap_or_else(|| "none".to_string())
        ));
        lines.push(format!(
            "cross_space={}",
            session.path_cross_space.unwrap_or(false)
        ));
        lines.push(format!(
            "path_result={}",
            session.path_result.as_deref().unwrap_or("none")
        ));
        lines.push(format!(
            "waypoints={}",
            session
                .path_waypoint_count
                .map(|c| c.to_string())
                .unwrap_or_else(|| "none".to_string())
        ));
    }
    if session.move_position.is_some() {
        lines.push(format!(
            "move_space={}",
            session
                .move_current_space
                .map(|s| s.raw().to_string())
                .unwrap_or_else(|| "none".to_string())
        ));
        if let Some(p) = session.move_position {
            lines.push(format!("move_pos={}", format_global(p, layout)));
        }
        if let Some(wp) = session.move_waypoint_position {
            lines.push(format!("move_waypoint={}", format_global(wp, layout)));
        }
        lines.push(format!(
            "move_waypoint_space={}",
            session
                .move_waypoint_space
                .map(|s| s.raw().to_string())
                .unwrap_or_else(|| "none".to_string())
        ));
        lines.push(format!(
            "point_legality={}",
            session.move_point_legality.as_deref().unwrap_or("none")
        ));
        lines.push(format!(
            "segment_legality={}",
            session.move_segment_legality.as_deref().unwrap_or("none")
        ));
        lines.push(format!(
            "grounding_ok={}",
            session.move_grounding_ok.unwrap_or(false)
        ));
        lines.push(format!(
            "position_changed={}",
            session.move_position_changed.unwrap_or(false)
        ));
        if let Some(reason) = &session.move_block_reason {
            lines.push(format!("move_block={reason}"));
        }
    }
    lines.join("\n")
}

#[cfg(feature = "dev")]
fn format_global(position: WorldPosition, layout: ChunkLayout) -> String {
    let g = position.to_global(layout);
    format!("({:.2},{:.2},{:.2})", g.x, g.y, g.z)
}

#[cfg(not(feature = "dev"))]
pub fn maybe_begin_session(_: &mut WorldData, _: UnitId, _: WorldPosition, _: &UnitCatalog) {}

#[cfg(not(feature = "dev"))]
pub fn record_interior_click_skipped(_: &mut WorldData, _: UnitId, _: &str) {}

#[cfg(not(feature = "dev"))]
pub fn record_interior_click_attempt(_: &mut WorldData, _: UnitId) {}

#[cfg(not(feature = "dev"))]
pub fn record_click_terrain_grounded(_: &mut WorldData, _: UnitId, _: WorldPosition) {}

#[cfg(not(feature = "dev"))]
pub fn record_interior_nav_probe(_: &mut WorldData, _: UnitId, _: WorldPosition) {}

#[cfg(not(feature = "dev"))]
pub fn record_interaction_query(_: &mut WorldData, _: UnitId, _: InteractionType, _: bool) {}

#[cfg(not(feature = "dev"))]
pub fn record_resolved_order_plan(_: &mut WorldData, _: UnitId, _: &str) {}

#[cfg(not(feature = "dev"))]
pub fn finish_command_resolution_failure(_: &mut WorldData, _: UnitId, _: &str) {}

#[cfg(not(feature = "dev"))]
pub fn record_order_issuance(
    _: &mut WorldData,
    _: UnitId,
    _: bool,
    _: Option<WorldPosition>,
    _: Option<String>,
) {
}

#[cfg(not(feature = "dev"))]
pub fn record_path_resolution(
    _: &mut WorldData,
    _: UnitId,
    _: SpaceId,
    _: SpaceId,
    _: SpaceId,
    _: &str,
    _: Option<u32>,
) {
}

#[cfg(not(feature = "dev"))]
pub fn record_first_movement_step(
    _: &mut WorldData,
    _: UnitId,
    _: SpaceId,
    _: WorldPosition,
    _: WorldPosition,
    _: SpaceId,
    _: &str,
    _: &str,
    _: bool,
    _: bool,
    _: Option<String>,
) {
}
