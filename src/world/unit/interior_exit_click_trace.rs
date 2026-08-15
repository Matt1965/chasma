//! Bounded dev trace for Interior-tracked unit exterior right-clicks (IN-11gI-E-T).
//!
//! One log block per relevant right-click; does not mutate navigation behavior.

use bevy::prelude::*;

use super::id::UnitId;
use crate::world::{
    BuildingNavigationRuntimeStore, ChunkLayout, SpaceId, SpaceRegistry, WorldData, WorldPosition,
    ground_position_in_space, interior_position_walkable, resolve_navigation_space_at_position,
};

/// Dev-only bounded Interior → Surface exit click trace store.
#[derive(Debug, Clone, Default)]
pub struct InteriorExitClickTrace {
    next_session_id: u32,
    active: Option<InteriorExitClickSession>,
}

#[derive(Debug, Clone)]
pub struct InteriorExitClickSession {
    pub session_id: u32,
    pub unit_id: UnitId,
    pub unit_position: WorldPosition,
    pub tracked_space: SpaceId,
    pub positional_space: SpaceId,
    pub ray_origin: Option<Vec3>,
    pub ray_direction: Option<Vec3>,
    pub terrain_pick_attempted: bool,
    pub terrain_pick_result: Option<String>,
    pub picked_world_position: Option<WorldPosition>,
    pub contextual_intent_created: Option<bool>,
    pub contextual_target_position: Option<WorldPosition>,
    pub intent_queue_blocked_reason: Option<String>,
    pub interior_resolver_input_target: Option<WorldPosition>,
    pub interior_grounded_candidate: Option<WorldPosition>,
    pub interior_position_walkable: Option<bool>,
    pub interior_resolver_classification: Option<String>,
    pub resolve_navigation_space_result: Option<SpaceId>,
    pub interior_resolver_move_target: Option<WorldPosition>,
    pub resolve_move_target_result: Option<String>,
    pub order_plan_type: Option<String>,
    pub dispatch_status: Option<String>,
    pub issue_move_orders_ran: Option<bool>,
    pub move_order_enqueued: Option<bool>,
    pub enqueued_target: Option<WorldPosition>,
    pub order_target: Option<WorldPosition>,
    pub tracked_space_before_resolution: Option<SpaceId>,
    pub resolved_start_space: Option<SpaceId>,
    pub resolved_goal_space: Option<SpaceId>,
    pub grounded_goal: Option<WorldPosition>,
    pub cross_space: Option<bool>,
    pub path_result: Option<String>,
    pub path_waypoint_count: Option<u32>,
    pub portal_waypoint_present: Option<bool>,
    pub portal_id: Option<u32>,
    pub first_waypoint: Option<WorldPosition>,
    pub final_waypoint: Option<WorldPosition>,
    pub unit_state_after_resolution: Option<String>,
    pub movement_path_stored: Option<bool>,
    pub stored_waypoint_count: Option<u32>,
    pub current_space_after_resolution: Option<SpaceId>,
    pub entrance_trace_handoff: Option<bool>,
    pub first_failure: Option<String>,
    pub passability_probe_lines: Vec<String>,
    pub emitted: bool,
}

impl InteriorExitClickTrace {
    pub fn is_active_for(&self, unit_id: UnitId) -> bool {
        self.active.as_ref().is_some_and(|s| s.unit_id == unit_id)
    }

    pub(crate) fn has_active_session(&self) -> bool {
        self.active.is_some()
    }

    pub fn is_active_for_target(&self, unit_id: UnitId, target: WorldPosition) -> bool {
        self.active.as_ref().is_some_and(|s| {
            s.unit_id == unit_id && targets_match(s.enqueued_target.or(s.order_target), target)
        })
    }

    pub fn clear_active(&mut self) {
        self.active = None;
    }
}

fn targets_match(expected: Option<WorldPosition>, actual: WorldPosition) -> bool {
    match expected {
        Some(expected) => positions_close(expected, actual),
        None => true,
    }
}

fn positions_close(a: WorldPosition, b: WorldPosition) -> bool {
    let layout = ChunkLayout {
        chunk_size_meters: 256.0,
        units_per_meter: 1.0,
    };
    let ag = a.to_global(layout);
    let bg = b.to_global(layout);
    ag.distance(bg) < 0.25
}

#[cfg(feature = "dev")]
pub fn maybe_begin_session(world: &mut WorldData, unit_id: UnitId, ray: &Ray3d) -> bool {
    let record = match world.get_unit(unit_id) {
        Some(record) => record,
        None => return false,
    };
    if record.current_space_id.is_surface() {
        return false;
    }
    let layout = world.layout();
    let runtime = world.building_navigation_runtime();
    let registry = world.space_registry();
    let position = record.placement.position;
    let tracked_space = record.current_space_id;
    let positional_space =
        resolve_navigation_space_at_position(runtime, registry, layout, position);

    let trace = world.interior_exit_click_trace_mut();
    let session_id = trace.next_session_id;
    trace.next_session_id = session_id.saturating_add(1);
    trace.active = Some(InteriorExitClickSession {
        session_id,
        unit_id,
        unit_position: position,
        tracked_space,
        positional_space,
        ray_origin: Some(ray.origin),
        ray_direction: Some(*ray.direction),
        terrain_pick_attempted: false,
        terrain_pick_result: None,
        picked_world_position: None,
        contextual_intent_created: None,
        contextual_target_position: None,
        intent_queue_blocked_reason: None,
        interior_resolver_input_target: None,
        interior_grounded_candidate: None,
        interior_position_walkable: None,
        interior_resolver_classification: None,
        resolve_navigation_space_result: None,
        interior_resolver_move_target: None,
        resolve_move_target_result: None,
        order_plan_type: None,
        dispatch_status: None,
        issue_move_orders_ran: None,
        move_order_enqueued: None,
        enqueued_target: None,
        order_target: None,
        tracked_space_before_resolution: None,
        resolved_start_space: None,
        resolved_goal_space: None,
        grounded_goal: None,
        cross_space: None,
        path_result: None,
        path_waypoint_count: None,
        portal_waypoint_present: None,
        portal_id: None,
        first_waypoint: None,
        final_waypoint: None,
        unit_state_after_resolution: None,
        movement_path_stored: None,
        stored_waypoint_count: None,
        current_space_after_resolution: None,
        entrance_trace_handoff: None,
        first_failure: None,
        passability_probe_lines: Vec::new(),
        emitted: false,
    });
    true
}

#[cfg(feature = "dev")]
pub fn record_terrain_pick_failure(world: &mut WorldData, unit_id: UnitId) {
    let session = active_session_mut(world, unit_id);
    if session.is_none() {
        return;
    }
    let session = session.unwrap();
    session.terrain_pick_attempted = true;
    session.terrain_pick_result = Some("none".to_string());
    session.first_failure = Some("TERRAIN_PICK_NONE".to_string());
    emit_session(world, unit_id);
}

#[cfg(feature = "dev")]
pub fn record_terrain_pick_success(
    world: &mut WorldData,
    unit_id: UnitId,
    picked: WorldPosition,
    intent_created: bool,
) {
    let session = active_session_mut(world, unit_id);
    if session.is_none() {
        return;
    }
    let session = session.unwrap();
    session.terrain_pick_attempted = true;
    session.terrain_pick_result = Some("success".to_string());
    session.picked_world_position = Some(picked);
    session.contextual_intent_created = Some(intent_created);
    session.contextual_target_position = Some(picked);
}

#[cfg(feature = "dev")]
pub fn record_intent_blocked(world: &mut WorldData, unit_id: UnitId, reason: &str) {
    let session = active_session_mut(world, unit_id);
    if session.is_none() {
        return;
    }
    let session = session.unwrap();
    session.intent_queue_blocked_reason = Some(reason.to_string());
    if session.first_failure.is_none() {
        session.first_failure = Some(format!("INTENT_BLOCKED:{reason}"));
    }
    emit_session(world, unit_id);
}

#[cfg(feature = "dev")]
pub fn record_unit_target_click(world: &mut WorldData, unit_id: UnitId) {
    let session = active_session_mut(world, unit_id);
    if session.is_none() {
        return;
    }
    let session = session.unwrap();
    session.terrain_pick_attempted = false;
    session.first_failure = Some("UNIT_TARGET_CLICK".to_string());
    emit_session(world, unit_id);
}

#[cfg(feature = "dev")]
pub fn record_interior_resolver_after_plan(
    world: &mut WorldData,
    unit_id: UnitId,
    input_target: WorldPosition,
    current_space: SpaceId,
    move_target: WorldPosition,
) {
    if !world.interior_exit_click_trace().is_active_for(unit_id) {
        return;
    }
    let runtime = world.building_navigation_runtime();
    let registry = world.space_registry();
    let layout = world.layout();
    let interior_grounded = ground_position_in_space(world, registry, current_space, input_target)
        .unwrap_or(input_target);
    let walkable =
        interior_position_walkable(runtime, registry, layout, interior_grounded, current_space);
    let (classification, resolved_space) = if walkable {
        ("SAME_SPACE_INTERIOR", None)
    } else {
        let space =
            resolve_navigation_space_at_position(runtime, registry, layout, interior_grounded);
        ("SURFACE_BOUND", Some(space))
    };
    record_interior_resolver(
        world,
        unit_id,
        input_target,
        current_space,
        classification,
        move_target,
        resolved_space,
    );
}

#[cfg(feature = "dev")]
pub fn record_interior_resolver(
    world: &mut WorldData,
    unit_id: UnitId,
    input_target: WorldPosition,
    current_space: SpaceId,
    classification: &str,
    move_target: WorldPosition,
    resolved_space: Option<SpaceId>,
) {
    if active_session_mut(world, unit_id).is_none() {
        return;
    }
    let runtime = world.building_navigation_runtime();
    let registry = world.space_registry();
    let layout = world.layout();
    let interior_grounded = ground_position_in_space(world, registry, current_space, input_target)
        .unwrap_or(input_target);
    let walkable =
        interior_position_walkable(runtime, registry, layout, interior_grounded, current_space);
    let session = active_session_mut(world, unit_id).unwrap();
    session.interior_resolver_input_target = Some(input_target);
    session.interior_grounded_candidate = Some(interior_grounded);
    session.interior_position_walkable = Some(walkable);
    session.interior_resolver_classification = Some(classification.to_string());
    session.resolve_navigation_space_result = resolved_space;
    session.interior_resolver_move_target = Some(move_target);
}

#[cfg(feature = "dev")]
pub fn record_resolve_move_target(
    world: &mut WorldData,
    unit_id: UnitId,
    result: Option<WorldPosition>,
    order_plan: &str,
) {
    let session = active_session_mut(world, unit_id);
    if session.is_none() {
        return;
    }
    let session = session.unwrap();
    session.resolve_move_target_result = result.map(|_| "Some".to_string());
    session.order_plan_type = Some(order_plan.to_string());
    if result.is_none() {
        session.first_failure = Some("RESOLVE_MOVE_TARGET_NONE".to_string());
    }
}

#[cfg(feature = "dev")]
pub fn record_dispatch(
    world: &mut WorldData,
    unit_id: UnitId,
    dispatch_status: &str,
    issue_move_orders_ran: bool,
    enqueued: bool,
    target: Option<WorldPosition>,
) {
    let session = active_session_mut(world, unit_id);
    if session.is_none() {
        return;
    }
    let session = session.unwrap();
    session.dispatch_status = Some(dispatch_status.to_string());
    session.issue_move_orders_ran = Some(issue_move_orders_ran);
    session.move_order_enqueued = Some(enqueued);
    session.enqueued_target = target;
    if !enqueued && session.first_failure.is_none() {
        session.first_failure = Some(format!("DISPATCH_NOT_ENQUEUED:{dispatch_status}"));
    }
    if dispatch_status == "Ignored" && session.first_failure.is_none() {
        session.first_failure = Some("DISPATCH_IGNORED".to_string());
    }
}

#[cfg(feature = "dev")]
pub fn record_order_enqueue(world: &mut WorldData, unit_id: UnitId, target: WorldPosition) {
    let session = active_session_mut(world, unit_id);
    if session.is_none() {
        return;
    }
    let session = session.unwrap();
    session.move_order_enqueued = Some(true);
    session.enqueued_target = Some(target);
}

#[cfg(feature = "dev")]
pub fn record_start_unit_move_to(
    world: &mut WorldData,
    unit_id: UnitId,
    order_target: WorldPosition,
    tracked_before: SpaceId,
    start_space: SpaceId,
    goal_space: SpaceId,
    grounded_goal: WorldPosition,
) {
    if !world
        .interior_exit_click_trace()
        .is_active_for_target(unit_id, order_target)
    {
        return;
    }
    let session = active_session_mut(world, unit_id);
    if session.is_none() {
        return;
    }
    let session = session.unwrap();
    session.order_target = Some(order_target);
    session.tracked_space_before_resolution = Some(tracked_before);
    session.resolved_start_space = Some(start_space);
    session.resolved_goal_space = Some(goal_space);
    session.grounded_goal = Some(grounded_goal);
    session.cross_space = Some(start_space != goal_space);
}

#[cfg(feature = "dev")]
pub fn record_path_result(
    world: &mut WorldData,
    unit_id: UnitId,
    order_target: WorldPosition,
    path_result_label: &str,
    waypoint_count: Option<u32>,
    portal_waypoint_present: bool,
    portal_id: Option<u32>,
    first_waypoint: Option<WorldPosition>,
    final_waypoint: Option<WorldPosition>,
) {
    if !world
        .interior_exit_click_trace()
        .is_active_for_target(unit_id, order_target)
    {
        return;
    }
    let session = active_session_mut(world, unit_id);
    if session.is_none() {
        return;
    }
    let session = session.unwrap();
    session.path_result = Some(path_result_label.to_string());
    session.path_waypoint_count = waypoint_count;
    session.portal_waypoint_present = Some(portal_waypoint_present);
    session.portal_id = portal_id;
    session.first_waypoint = first_waypoint;
    session.final_waypoint = final_waypoint;
    if path_result_label != "success" {
        session.first_failure = Some(format!("PATH_RESOLUTION:{path_result_label}"));
    }
    if portal_waypoint_present && path_result_label == "success" {
        session.entrance_trace_handoff = Some(true);
    }
}

#[cfg(feature = "dev")]
pub fn record_surface_goal_passability_probe(
    world: &mut WorldData,
    unit_id: UnitId,
    order_target: WorldPosition,
    catalogs: crate::world::PassabilityCatalogs<'_>,
    agent_radius_meters: f32,
    max_slope_degrees: f32,
    grounded_goal: WorldPosition,
    start_space: SpaceId,
    goal_space: SpaceId,
    unit_ownership: Option<crate::world::UnitOwnership>,
) {
    if !world
        .interior_exit_click_trace()
        .is_active_for_target(unit_id, order_target)
    {
        return;
    }
    let probe = super::surface_goal_passability_probe::probe_interior_to_surface_exit_passability(
        world,
        catalogs,
        agent_radius_meters,
        max_slope_degrees,
        grounded_goal,
        start_space,
        goal_space,
        unit_ownership,
    );
    let mut lines = super::surface_goal_passability_probe::format_surface_point_probe_lines(
        "surface_goal",
        &probe.goal,
    );
    if let Some(staging) = &probe.staging {
        lines.extend(
            super::surface_goal_passability_probe::format_surface_point_probe_lines(
                "surface_staging",
                staging,
            ),
        );
        lines.push(format!("staging_point_legality={}", staging.point_legality));
        lines.push(format!(
            "staging_block_reason={}",
            staging
                .passability_block_reason
                .as_deref()
                .unwrap_or("none")
        ));
        lines.push(format!(
            "staging_unavailable_reason={}",
            staging
                .passability_unavailable_reason
                .as_deref()
                .unwrap_or("none")
        ));
    } else {
        lines.push("surface_staging=none".to_string());
        lines.push("staging_point_legality=none".to_string());
        lines.push("staging_block_reason=none".to_string());
        lines.push("staging_unavailable_reason=none".to_string());
    }
    for sample in &probe.local_samples {
        lines.push(super::surface_goal_passability_probe::format_local_sample_line(sample));
    }
    let session = active_session_mut(world, unit_id);
    if session.is_none() {
        return;
    }
    session.unwrap().passability_probe_lines = lines;
}

#[cfg(feature = "dev")]
pub fn record_post_resolution_state(
    world: &mut WorldData,
    unit_id: UnitId,
    order_target: WorldPosition,
    state_label: &str,
    path_stored: bool,
    stored_waypoint_count: Option<u32>,
    current_space: SpaceId,
) {
    if !world
        .interior_exit_click_trace()
        .is_active_for_target(unit_id, order_target)
    {
        return;
    }
    let session = active_session_mut(world, unit_id);
    if session.is_none() {
        return;
    }
    let session = session.unwrap();
    session.unit_state_after_resolution = Some(state_label.to_string());
    session.movement_path_stored = Some(path_stored);
    session.stored_waypoint_count = stored_waypoint_count;
    session.current_space_after_resolution = Some(current_space);
    emit_session(world, unit_id);
}

#[cfg(feature = "dev")]
fn active_session_mut(
    world: &mut WorldData,
    unit_id: UnitId,
) -> Option<&mut InteriorExitClickSession> {
    let trace = world.interior_exit_click_trace_mut();
    match trace.active.as_mut() {
        Some(session) if session.unit_id == unit_id => Some(session),
        _ => None,
    }
}

#[cfg(feature = "dev")]
fn emit_session(world: &mut WorldData, unit_id: UnitId) {
    let trace = world.interior_exit_click_trace_mut();
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
    let mut log = format_session_log(&session);
    if let Some(leg_lines) =
        crate::world::navigation::cross_space_leg_trace::take_pending_leg_trace_lines()
    {
        log = format!("{log}\n{}", leg_lines.join("\n"));
    }
    crate::logging::append_log_block(
        crate::logging::NAVIGATION_TRACE_LOG_PATH,
        "# chasma navigation trace",
        &log,
    );
    trace.clear_active();
}

#[cfg(feature = "dev")]
fn format_session_log(session: &InteriorExitClickSession) -> String {
    let layout = ChunkLayout {
        chunk_size_meters: 256.0,
        units_per_meter: 1.0,
    };
    let pos = session.unit_position.to_global(layout);
    let ray_origin = session
        .ray_origin
        .map(|v| format!("({:.2},{:.2},{:.2})", v.x, v.y, v.z))
        .unwrap_or_else(|| "none".to_string());
    let ray_dir = session
        .ray_direction
        .map(|v| format!("({:.3},{:.3},{:.3})", v.x, v.y, v.z))
        .unwrap_or_else(|| "none".to_string());
    let picked = session
        .picked_world_position
        .map(|p| format_global(p, layout))
        .unwrap_or_else(|| "none".to_string());
    let contextual_target = session
        .contextual_target_position
        .map(|p| format_global(p, layout))
        .unwrap_or_else(|| "none".to_string());
    let interior_input = session
        .interior_resolver_input_target
        .map(|p| format_global(p, layout))
        .unwrap_or_else(|| "none".to_string());
    let interior_grounded = session
        .interior_grounded_candidate
        .map(|p| format_global(p, layout))
        .unwrap_or_else(|| "none".to_string());
    let resolver_target = session
        .interior_resolver_move_target
        .map(|p| format_global(p, layout))
        .unwrap_or_else(|| "none".to_string());
    let enqueued = session
        .enqueued_target
        .map(|p| format_global(p, layout))
        .unwrap_or_else(|| "none".to_string());
    let order_target = session
        .order_target
        .map(|p| format_global(p, layout))
        .unwrap_or_else(|| "none".to_string());
    let grounded_goal = session
        .grounded_goal
        .map(|p| format_global(p, layout))
        .unwrap_or_else(|| "none".to_string());
    let first_wp = session
        .first_waypoint
        .map(|p| format_global(p, layout))
        .unwrap_or_else(|| "none".to_string());
    let final_wp = session
        .final_waypoint
        .map(|p| format_global(p, layout))
        .unwrap_or_else(|| "none".to_string());
    let first_failure = session.first_failure.as_deref().unwrap_or("none");
    let handoff = if session.entrance_trace_handoff == Some(true) {
        "HANDOFF=ENTRANCE_TRAVERSAL_TRACE"
    } else {
        "none"
    };

    let mut lines = vec![
        "[INTERIOR_EXIT_CLICK_TRACE]".to_string(),
        format!("exit_click_session={}", session.session_id),
        format!("unit=U-{:04}", session.unit_id.raw()),
        format!("position=({:.2},{:.2},{:.2})", pos.x, pos.y, pos.z),
        format!("tracked_space={}", session.tracked_space.raw()),
        format!("positional_space={}", session.positional_space.raw()),
        format!("ray_origin={ray_origin}"),
        format!("ray_direction={ray_dir}"),
        format!("terrain_pick_attempted={}", session.terrain_pick_attempted),
        format!(
            "terrain_pick_result={}",
            session.terrain_pick_result.as_deref().unwrap_or("none")
        ),
        format!("picked_world_position={picked}"),
        format!(
            "contextual_intent_created={}",
            session
                .contextual_intent_created
                .map(|v| v.to_string())
                .unwrap_or_else(|| "none".to_string())
        ),
        format!("contextual_target_position={contextual_target}"),
        format!(
            "intent_queue_blocked_reason={}",
            session
                .intent_queue_blocked_reason
                .as_deref()
                .unwrap_or("none")
        ),
        format!("interior_resolver_input={interior_input}"),
        format!("interior_grounded_candidate={interior_grounded}"),
        format!(
            "interior_position_walkable={}",
            session
                .interior_position_walkable
                .map(|v| v.to_string())
                .unwrap_or_else(|| "none".to_string())
        ),
        format!(
            "interior_resolver_classification={}",
            session
                .interior_resolver_classification
                .as_deref()
                .unwrap_or("none")
        ),
        format!(
            "resolve_navigation_space_result={}",
            session
                .resolve_navigation_space_result
                .map(|s| s.raw().to_string())
                .unwrap_or_else(|| "none".to_string())
        ),
        format!("interior_resolver_move_target={resolver_target}"),
        format!(
            "resolve_move_target_result={}",
            session
                .resolve_move_target_result
                .as_deref()
                .unwrap_or("none")
        ),
        format!(
            "order_plan_type={}",
            session.order_plan_type.as_deref().unwrap_or("none")
        ),
        format!(
            "dispatch_status={}",
            session.dispatch_status.as_deref().unwrap_or("none")
        ),
        format!(
            "issue_move_orders_ran={}",
            session
                .issue_move_orders_ran
                .map(|v| v.to_string())
                .unwrap_or_else(|| "none".to_string())
        ),
        format!(
            "move_order_enqueued={}",
            session
                .move_order_enqueued
                .map(|v| v.to_string())
                .unwrap_or_else(|| "none".to_string())
        ),
        format!("enqueued_target={enqueued}"),
        format!("order_target={order_target}"),
        format!(
            "tracked_space_before_resolution={}",
            session
                .tracked_space_before_resolution
                .map(|s| s.raw().to_string())
                .unwrap_or_else(|| "none".to_string())
        ),
        format!(
            "resolved_start_space={}",
            session
                .resolved_start_space
                .map(|s| s.raw().to_string())
                .unwrap_or_else(|| "none".to_string())
        ),
        format!(
            "resolved_goal_space={}",
            session
                .resolved_goal_space
                .map(|s| s.raw().to_string())
                .unwrap_or_else(|| "none".to_string())
        ),
        format!("grounded_goal={grounded_goal}"),
        format!(
            "cross_space={}",
            session
                .cross_space
                .map(|v| v.to_string())
                .unwrap_or_else(|| "none".to_string())
        ),
        format!(
            "path_result={}",
            session.path_result.as_deref().unwrap_or("none")
        ),
        format!(
            "waypoint_count={}",
            session
                .path_waypoint_count
                .map(|c| c.to_string())
                .unwrap_or_else(|| "none".to_string())
        ),
        format!(
            "portal_waypoint_present={}",
            session
                .portal_waypoint_present
                .map(|v| v.to_string())
                .unwrap_or_else(|| "none".to_string())
        ),
        format!(
            "portal_id={}",
            session
                .portal_id
                .map(|id| id.to_string())
                .unwrap_or_else(|| "none".to_string())
        ),
        format!("first_waypoint={first_wp}"),
        format!("final_waypoint={final_wp}"),
        format!(
            "unit_state_after_resolution={}",
            session
                .unit_state_after_resolution
                .as_deref()
                .unwrap_or("none")
        ),
        format!(
            "movement_path_stored={}",
            session
                .movement_path_stored
                .map(|v| v.to_string())
                .unwrap_or_else(|| "none".to_string())
        ),
        format!(
            "stored_waypoint_count={}",
            session
                .stored_waypoint_count
                .map(|c| c.to_string())
                .unwrap_or_else(|| "none".to_string())
        ),
        format!(
            "current_space_after_resolution={}",
            session
                .current_space_after_resolution
                .map(|s| s.raw().to_string())
                .unwrap_or_else(|| "none".to_string())
        ),
        format!("entrance_trace={handoff}"),
        format!("FIRST_FAILURE={first_failure}"),
    ];
    lines.extend(session.passability_probe_lines.clone());
    lines.join("\n")
}

#[cfg(feature = "dev")]
fn format_global(position: WorldPosition, layout: ChunkLayout) -> String {
    let g = position.to_global(layout);
    format!("({:.2},{:.2},{:.2})", g.x, g.y, g.z)
}

#[cfg(not(feature = "dev"))]
pub fn maybe_begin_session(_: &mut WorldData, _: UnitId, _: &Ray3d) -> bool {
    false
}

#[cfg(not(feature = "dev"))]
pub fn record_terrain_pick_failure(_: &mut WorldData, _: UnitId) {}

#[cfg(not(feature = "dev"))]
pub fn record_terrain_pick_success(_: &mut WorldData, _: UnitId, _: WorldPosition, _: bool) {}

#[cfg(not(feature = "dev"))]
pub fn record_intent_blocked(_: &mut WorldData, _: UnitId, _: &str) {}

#[cfg(not(feature = "dev"))]
pub fn record_unit_target_click(_: &mut WorldData, _: UnitId) {}

#[cfg(not(feature = "dev"))]
pub fn record_interior_resolver_after_plan(
    _: &mut WorldData,
    _: UnitId,
    _: WorldPosition,
    _: SpaceId,
    _: WorldPosition,
) {
}

#[cfg(not(feature = "dev"))]
pub fn record_interior_resolver(
    _: &mut WorldData,
    _: UnitId,
    _: WorldPosition,
    _: SpaceId,
    _: &str,
    _: WorldPosition,
    _: Option<SpaceId>,
) {
}

#[cfg(not(feature = "dev"))]
pub fn record_resolve_move_target(_: &mut WorldData, _: UnitId, _: Option<WorldPosition>, _: &str) {
}

#[cfg(not(feature = "dev"))]
pub fn record_dispatch(
    _: &mut WorldData,
    _: UnitId,
    _: &str,
    _: bool,
    _: bool,
    _: Option<WorldPosition>,
) {
}

#[cfg(not(feature = "dev"))]
pub fn record_order_enqueue(_: &mut WorldData, _: UnitId, _: WorldPosition) {}

#[cfg(not(feature = "dev"))]
pub fn record_start_unit_move_to(
    _: &mut WorldData,
    _: UnitId,
    _: WorldPosition,
    _: SpaceId,
    _: SpaceId,
    _: SpaceId,
    _: WorldPosition,
) {
}

#[cfg(not(feature = "dev"))]
pub fn record_path_result(
    _: &mut WorldData,
    _: UnitId,
    _: WorldPosition,
    _: &str,
    _: Option<u32>,
    _: bool,
    _: Option<u32>,
    _: Option<WorldPosition>,
    _: Option<WorldPosition>,
) {
}

#[cfg(not(feature = "dev"))]
pub fn record_surface_goal_passability_probe(
    _: &mut WorldData,
    _: UnitId,
    _: WorldPosition,
    _: crate::world::PassabilityCatalogs<'_>,
    _: f32,
    _: f32,
    _: WorldPosition,
    _: SpaceId,
    _: SpaceId,
    _: Option<crate::world::UnitOwnership>,
) {
}

#[cfg(not(feature = "dev"))]
pub fn record_post_resolution_state(
    _: &mut WorldData,
    _: UnitId,
    _: WorldPosition,
    _: &str,
    _: bool,
    _: Option<u32>,
    _: SpaceId,
) {
}
