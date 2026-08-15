//! Bounded dev diagnostic trace for post-ExteriorEntrance Surface movement (NAV-EXIT-T).
//!
//! Arms after Interior→Surface portal completion; records a short bounded Surface tick
//! sequence to `logs/navigation_trace.log`. Does not mutate navigation behavior.

use bevy::prelude::*;

use super::id::UnitId;
use super::movement::BlockedMovementReason;
use super::state::UnitState;
use crate::world::{
    ChunkLayout, NavigationAgent, NavigationConfig, NavigationPath, NavigationWaypoint,
    PassabilityAgent, PassabilityCatalogs, PassabilityResult, PortalId, PortalType, SpaceId,
    WorldData, WorldPosition, ground_position_in_space, is_segment_walkable_in_space,
    query_navigation_point_legality, resolve_surface_entrance_escape_position,
    surface_blueprint_support_blocks_position, surface_position_in_entrance_access_corridor,
    xz_distance,
};

pub const TRACE_MARKER: &str = "[POST_EXIT_JITTER_TRACE]";
pub const NEW_ORDER_MARKER: &str = "[POST_EXIT_JITTER_TRACE_NEW_ORDER]";

const MAX_SESSION_TICKS: u32 = 12;
const JITTER_LOOP_TICKS: u32 = 4;
const JITTER_DISPLACEMENT_METERS: f32 = 0.02;

/// Dev-only bounded post-exit jitter trace store.
#[derive(Debug, Clone, Default)]
pub struct PostExitJitterTrace {
    next_session_id: u32,
    active: Option<PostExitJitterSession>,
}

#[derive(Debug, Clone)]
struct PostExitJitterSession {
    session_id: u32,
    unit_id: UnitId,
    portal_id: PortalId,
    interior_space: SpaceId,
    surface_space: SpaceId,
    portal_from_center: WorldPosition,
    escape_position: WorldPosition,
    ordered_goal: WorldPosition,
    agent_radius_m: f32,
    path_waypoint_count: usize,
    start_waypoint_index: usize,
    surface_grid_spacing_m: f32,
    start_position: WorldPosition,
    post_exit_waypoints: Vec<WaypointSummary>,
    tick_lines: Vec<String>,
    tick_count: u32,
    loop_position_anchor: Option<WorldPosition>,
    loop_detector: JitterLoopDetector,
    emitted: bool,
}

#[derive(Debug, Clone)]
struct WaypointSummary {
    index: usize,
    position: WorldPosition,
    space_id: SpaceId,
    portal_id: Option<PortalId>,
    distance_from_previous_m: f32,
}

#[derive(Debug, Clone, Default)]
struct JitterLoopDetector {
    repeated_waypoint_index: Option<usize>,
    repeated_effective_index: Option<usize>,
    repeated_block_reason: Option<String>,
    consecutive_blocked: u32,
    loop_start_position: Option<WorldPosition>,
    loop_candidate: Option<WorldPosition>,
    steering_delta_min_deg: f32,
    steering_delta_max_deg: f32,
}

impl PostExitJitterTrace {
    pub fn is_active_for(&self, unit_id: UnitId) -> bool {
        self.active.as_ref().is_some_and(|s| s.unit_id == unit_id)
    }

    pub fn clear_active(&mut self) {
        self.active = None;
    }

    #[cfg(any(test, feature = "dev"))]
    pub fn active_tick_count(&self) -> Option<u32> {
        self.active.as_ref().map(|s| s.tick_count)
    }
}

impl JitterLoopDetector {
    fn observe_blocked(
        &mut self,
        waypoint_index: usize,
        effective_index: usize,
        block_reason: &str,
        unit_position: WorldPosition,
        candidate: WorldPosition,
        steering_delta_deg: f32,
        layout: ChunkLayout,
    ) -> bool {
        let same_indices = self.repeated_waypoint_index == Some(waypoint_index)
            && self.repeated_effective_index == Some(effective_index)
            && self.repeated_block_reason.as_deref() == Some(block_reason);
        if same_indices {
            self.consecutive_blocked += 1;
        } else {
            self.repeated_waypoint_index = Some(waypoint_index);
            self.repeated_effective_index = Some(effective_index);
            self.repeated_block_reason = Some(block_reason.to_string());
            self.consecutive_blocked = 1;
            self.loop_start_position = Some(unit_position);
            self.loop_candidate = Some(candidate);
            self.steering_delta_min_deg = steering_delta_deg;
            self.steering_delta_max_deg = steering_delta_deg;
        }
        if steering_delta_deg < self.steering_delta_min_deg {
            self.steering_delta_min_deg = steering_delta_deg;
        }
        if steering_delta_deg > self.steering_delta_max_deg {
            self.steering_delta_max_deg = steering_delta_deg;
        }
        if self.consecutive_blocked < JITTER_LOOP_TICKS {
            return false;
        }
        let displacement = self
            .loop_start_position
            .map(|start| xz_distance(start, unit_position, layout))
            .unwrap_or(0.0);
        displacement <= JITTER_DISPLACEMENT_METERS
    }
}

/// Incrementally filled during one Surface movement tick (dev only).
#[cfg(feature = "dev")]
pub struct PostExitStepCapture {
    unit_id: UnitId,
    tick_sequence: u32,
    current_position: WorldPosition,
    current_space: SpaceId,
    waypoint_index: usize,
    effective_index: usize,
    path: NavigationPath,
    target: WorldPosition,
    state_waypoint: NavigationWaypoint,
    effective_waypoint: NavigationWaypoint,
    next_waypoint: Option<NavigationWaypoint>,
    heading_skip: bool,
    path_direction_xz: Option<Vec2>,
    steered_direction_xz: Option<Vec2>,
    movement_direction_xz: Option<Vec2>,
    step_distance: f32,
    candidate_before_ground: Option<WorldPosition>,
    grounded_candidate: Option<WorldPosition>,
    point_legal: Option<bool>,
    point_block_reason: Option<String>,
    segment_legal: Option<bool>,
    segment_block_reason: Option<String>,
    blueprint_support_blocks_current: Option<bool>,
    blueprint_support_blocks_proposed: Option<bool>,
    current_in_access_corridor: Option<bool>,
    proposed_in_access_corridor: Option<bool>,
    progressed_distance: f32,
    movement_accepted: bool,
    apply_blocked_ran: bool,
    blocked_reason: Option<BlockedMovementReason>,
    waypoint_advanced: bool,
    position_changed: bool,
    suppress_return_space: Option<String>,
    inside_portal_trigger: bool,
    portal_transition_ran: bool,
    outcome_label: String,
}

#[cfg(feature = "dev")]
impl PostExitStepCapture {
    pub fn maybe_begin(
        world: &mut WorldData,
        unit_id: UnitId,
        current_space: SpaceId,
        current_position: WorldPosition,
        path: &NavigationPath,
        waypoint_index: usize,
        effective_index: usize,
        effective_waypoint: NavigationWaypoint,
        target: WorldPosition,
    ) -> Option<Self> {
        if !current_space.is_surface() || !world.post_exit_jitter_trace().is_active_for(unit_id) {
            return None;
        }
        let session = world.post_exit_jitter_trace_mut().active.as_mut()?;
        if session.unit_id != unit_id {
            return None;
        }
        session.tick_count += 1;
        let tick_sequence = session.tick_count;
        let state_waypoint = path.waypoints.get(waypoint_index).copied()?;
        let next_waypoint = path.waypoints.get(effective_index + 1).copied();
        Some(Self {
            unit_id,
            tick_sequence,
            current_position,
            current_space,
            waypoint_index,
            effective_index,
            path: path.clone(),
            target,
            state_waypoint,
            effective_waypoint,
            next_waypoint,
            heading_skip: effective_index != waypoint_index,
            path_direction_xz: None,
            steered_direction_xz: None,
            movement_direction_xz: None,
            step_distance: 0.0,
            candidate_before_ground: None,
            grounded_candidate: None,
            point_legal: None,
            point_block_reason: None,
            segment_legal: None,
            segment_block_reason: None,
            blueprint_support_blocks_current: None,
            blueprint_support_blocks_proposed: None,
            current_in_access_corridor: None,
            proposed_in_access_corridor: None,
            progressed_distance: 0.0,
            movement_accepted: false,
            apply_blocked_ran: false,
            blocked_reason: None,
            waypoint_advanced: false,
            position_changed: false,
            suppress_return_space: None,
            inside_portal_trigger: false,
            portal_transition_ran: false,
            outcome_label: "pending".to_string(),
        })
    }

    pub fn set_movement_vectors(
        &mut self,
        path_direction_xz: Vec2,
        steered_direction_xz: Vec2,
        movement_direction_xz: Vec2,
        step_distance: f32,
        candidate_before_ground: WorldPosition,
    ) {
        self.path_direction_xz = Some(path_direction_xz);
        self.steered_direction_xz = Some(steered_direction_xz);
        self.movement_direction_xz = Some(movement_direction_xz);
        self.step_distance = step_distance;
        self.candidate_before_ground = Some(candidate_before_ground);
    }

    pub fn set_legality_probe(
        &mut self,
        catalogs: PassabilityCatalogs<'_>,
        world: &WorldData,
        agent: PassabilityAgent,
        active_space: SpaceId,
        grounded: WorldPosition,
    ) {
        let layout = world.layout();
        let current_xz = self.current_position.to_global(layout).xz();
        let proposed_xz = grounded.to_global(layout).xz();
        self.current_in_access_corridor = Some(point_in_any_entrance_access_corridor(
            world,
            current_xz,
            agent.radius_meters,
        ));
        self.proposed_in_access_corridor = Some(point_in_any_entrance_access_corridor(
            world,
            proposed_xz,
            agent.radius_meters,
        ));
        self.blueprint_support_blocks_current = Some(
            surface_blueprint_support_blocks_position(
                world,
                layout,
                current_xz,
                agent.radius_meters,
            )
            .is_some(),
        );
        self.blueprint_support_blocks_proposed = Some(
            surface_blueprint_support_blocks_position(
                world,
                layout,
                proposed_xz,
                agent.radius_meters,
            )
            .is_some(),
        );
        match query_navigation_point_legality(world, catalogs, grounded, agent, active_space) {
            PassabilityResult::Passable { .. } => {
                self.point_legal = Some(true);
            }
            PassabilityResult::Unavailable { reason, .. } => {
                self.point_legal = Some(false);
                self.point_block_reason = Some(format!("unavailable:{reason:?}"));
            }
            PassabilityResult::Blocked { reason, source } => {
                self.point_legal = Some(false);
                self.point_block_reason = Some(format!("blocked:{reason:?} source={source:?}"));
            }
        }
        let segment_ok = is_segment_walkable_in_space(
            world,
            world.space_registry(),
            catalogs,
            NavigationConfig::default(),
            active_space,
            NavigationAgent {
                radius_meters: agent.radius_meters,
                max_slope_degrees: agent.max_slope_degrees,
            },
            self.current_position,
            grounded,
            layout,
        );
        self.segment_legal = Some(segment_ok);
        if !segment_ok {
            self.segment_block_reason = Some("segment_blocked".to_string());
        }
    }

    pub fn set_portal_state(
        &mut self,
        world: &WorldData,
        layout: ChunkLayout,
        current_space: SpaceId,
        position: WorldPosition,
        portal_transition_ran: bool,
    ) {
        let transition = world.portal_transition_state(self.unit_id);
        self.suppress_return_space = transition
            .suppress_return_space
            .map(|(portal, space)| format!("portal={} space={}", portal.raw(), space.raw()));
        self.inside_portal_trigger = world
            .post_exit_jitter_trace()
            .active
            .as_ref()
            .and_then(|session| {
                world
                    .space_registry()
                    .get_portal(session.portal_id)
                    .map(|portal| {
                        portal.contains_agent_in_space(
                            position.to_global(layout).xz(),
                            current_space,
                            layout,
                        )
                    })
            })
            .unwrap_or(false);
        self.portal_transition_ran = portal_transition_ran;
    }

    pub fn finish(mut self, world: &mut WorldData, catalogs: PassabilityCatalogs<'_>) {
        self.outcome_label = if self.apply_blocked_ran {
            format!(
                "blocked:{:?}",
                self.blocked_reason
                    .unwrap_or(BlockedMovementReason::BlockedByBuilding)
            )
        } else if self.movement_accepted {
            "moved".to_string()
        } else {
            self.outcome_label.clone()
        };
        record_tick(world, self, catalogs);
    }
}

#[cfg(feature = "dev")]
pub fn finish_tick_from_step(
    world: &mut WorldData,
    catalogs: PassabilityCatalogs<'_>,
    capture: &mut Option<PostExitStepCapture>,
    agent: PassabilityAgent,
    active_space: SpaceId,
    current_space: SpaceId,
    current_position: WorldPosition,
    layout: ChunkLayout,
    grounded: WorldPosition,
    apply_blocked: bool,
    blocked_reason: Option<BlockedMovementReason>,
    movement_accepted: bool,
    progressed_distance: f32,
    waypoint_advanced: bool,
    position_changed: bool,
    portal_transition_ran: bool,
    outcome_label: &str,
) {
    let Some(cap) = capture.as_mut() else {
        return;
    };
    cap.set_legality_probe(catalogs, world, agent, active_space, grounded);
    cap.set_portal_state(
        world,
        layout,
        current_space,
        current_position,
        portal_transition_ran,
    );
    cap.apply_blocked_ran = apply_blocked;
    cap.blocked_reason = blocked_reason;
    cap.grounded_candidate = Some(grounded);
    cap.movement_accepted = movement_accepted;
    cap.progressed_distance = progressed_distance;
    cap.waypoint_advanced = waypoint_advanced;
    cap.position_changed = position_changed;
    cap.outcome_label = outcome_label.to_string();
    let finished = capture.take().expect("capture");
    finished.finish(world, catalogs);
}

#[cfg(not(feature = "dev"))]
#[allow(clippy::too_many_arguments)]
pub fn finish_tick_from_step(
    _: &mut WorldData,
    _: PassabilityCatalogs<'_>,
    _: &mut Option<PostExitStepCapture>,
    _: PassabilityAgent,
    _: SpaceId,
    _: SpaceId,
    _: WorldPosition,
    _: ChunkLayout,
    _: WorldPosition,
    _: bool,
    _: Option<BlockedMovementReason>,
    _: bool,
    _: f32,
    _: bool,
    _: bool,
    _: bool,
    _: &str,
) {
}

#[cfg(feature = "dev")]
pub fn arm_session_after_interior_surface_exit(
    world: &mut WorldData,
    unit_id: UnitId,
    from_space: SpaceId,
    to_space: SpaceId,
    portal_id: PortalId,
    position_after_transition: WorldPosition,
    path: NavigationPath,
    next_waypoint_index: usize,
    ordered_goal: WorldPosition,
    agent_radius_m: f32,
) {
    if from_space.is_surface() || !to_space.is_surface() {
        return;
    }
    let registry = world.space_registry();
    let portal = match registry.get_portal(portal_id) {
        Some(portal) if portal.portal_type == PortalType::ExteriorEntrance => portal,
        _ => return,
    };
    let layout = world.layout();
    let escape = resolve_surface_entrance_escape_position(world, registry, portal, agent_radius_m)
        .unwrap_or(position_after_transition);
    let portal_from_center = WorldPosition::from_global(
        Vec3::new(
            portal.from_center_global_xz.x,
            position_after_transition.to_global(layout).y,
            portal.from_center_global_xz.y,
        ),
        layout,
    );
    let post_exit_waypoints = summarize_post_exit_waypoints(&path, next_waypoint_index, layout);
    let trace = world.post_exit_jitter_trace_mut();
    let session_id = trace.next_session_id;
    trace.next_session_id = session_id.saturating_add(1);
    trace.active = Some(PostExitJitterSession {
        session_id,
        unit_id,
        portal_id,
        interior_space: from_space,
        surface_space: to_space,
        portal_from_center,
        escape_position: escape,
        ordered_goal,
        agent_radius_m: agent_radius_m,
        path_waypoint_count: path.len(),
        start_waypoint_index: next_waypoint_index,
        surface_grid_spacing_m: NavigationConfig::default().cell_spacing_meters,
        start_position: position_after_transition,
        post_exit_waypoints,
        tick_lines: Vec::new(),
        tick_count: 0,
        loop_position_anchor: Some(position_after_transition),
        loop_detector: JitterLoopDetector::default(),
        emitted: false,
    });
}

#[cfg(feature = "dev")]
pub fn record_new_order_during_session(
    world: &mut WorldData,
    unit_id: UnitId,
    old_waypoint_index: usize,
    old_effective_index: usize,
    old_position: WorldPosition,
    old_goal: WorldPosition,
    old_space: SpaceId,
    old_path: &NavigationPath,
    new_goal: WorldPosition,
    new_start_space: SpaceId,
    new_path: Option<&NavigationPath>,
) {
    if !world.post_exit_jitter_trace().is_active_for(unit_id) {
        return;
    }
    let layout = world.layout();
    let mut lines = vec![NEW_ORDER_MARKER.to_string()];
    lines.push(format!("unit=U-{:04}", unit_id.raw()));
    lines.push(format!("old_waypoint_index={old_waypoint_index}"));
    lines.push(format!("old_effective_index={old_effective_index}"));
    lines.push(format!(
        "old_position={}",
        format_global(old_position, layout)
    ));
    lines.push(format!("old_goal={}", format_global(old_goal, layout)));
    lines.push(format!("old_current_space={}", old_space.raw()));
    lines.push(format!(
        "old_path_had_portal={}",
        old_path.waypoints.iter().any(|wp| wp.portal_id.is_some())
    ));
    lines.push(format!("new_goal={}", format_global(new_goal, layout)));
    lines.push(format!("new_start_space={}", new_start_space.raw()));
    if let Some(path) = new_path {
        for (index, waypoint) in path.waypoints.iter().take(5).enumerate() {
            lines.push(format!(
                "new_wp[{index}]={} space={} portal={}",
                format_global(waypoint.position, layout),
                waypoint.space_id.raw(),
                waypoint
                    .portal_id
                    .map(|id| id.raw().to_string())
                    .unwrap_or_else(|| "none".to_string())
            ));
        }
    }
    emit_and_clear(world, unit_id, lines);
}

#[cfg(feature = "dev")]
fn record_tick(
    world: &mut WorldData,
    capture: PostExitStepCapture,
    _catalogs: PassabilityCatalogs<'_>,
) {
    let layout = world.layout();
    let session = match world.post_exit_jitter_trace_mut().active.as_mut() {
        Some(session) if session.unit_id == capture.unit_id => session,
        _ => return,
    };
    let steering_delta_deg = angular_delta_degrees(
        capture.path_direction_xz.unwrap_or(Vec2::ZERO),
        capture
            .movement_direction_xz
            .unwrap_or(capture.steered_direction_xz.unwrap_or(Vec2::ZERO)),
    );
    let block_label = capture
        .blocked_reason
        .map(|reason| format!("{reason:?}"))
        .or_else(|| capture.point_block_reason.clone())
        .unwrap_or_else(|| "none".to_string());
    let mut lines = vec![format!("tick={}", capture.tick_sequence)];
    lines.push(format!(
        "position={}",
        format_global(capture.current_position, layout)
    ));
    lines.push(format!("current_space={}", capture.current_space.raw()));
    lines.push(format!("waypoint_index={}", capture.waypoint_index));
    lines.push(format!("effective_index={}", capture.effective_index));
    lines.push(format!("path_waypoint_count={}", capture.path.len()));
    lines.push(format!(
        "state_waypoint={}",
        format_global(capture.state_waypoint.position, layout)
    ));
    lines.push(format!(
        "effective_waypoint={}",
        format_global(capture.effective_waypoint.position, layout)
    ));
    if let Some(next) = capture.next_waypoint {
        lines.push(format!(
            "next_waypoint={}",
            format_global(next.position, layout)
        ));
    }
    lines.push(format!(
        "dist_state_wp_m={:.4}",
        xz_distance(
            capture.current_position,
            capture.state_waypoint.position,
            layout
        )
    ));
    lines.push(format!(
        "dist_effective_wp_m={:.4}",
        xz_distance(
            capture.current_position,
            capture.effective_waypoint.position,
            layout
        )
    ));
    lines.push(format!(
        "within_arrival_tol={}",
        xz_distance(
            capture.current_position,
            capture.state_waypoint.position,
            layout
        ) <= 0.05
    ));
    lines.push(format!("heading_skip={}", capture.heading_skip));
    if let Some(dir) = capture.path_direction_xz {
        lines.push(format!("planned_dir=({:.4},{:.4})", dir.x, dir.y));
    }
    if let Some(dir) = capture.steered_direction_xz {
        lines.push(format!("steered_dir=({:.4},{:.4})", dir.x, dir.y));
    }
    if let Some(dir) = capture.movement_direction_xz {
        lines.push(format!("final_dir=({:.4},{:.4})", dir.x, dir.y));
    }
    lines.push(format!("steering_delta_deg={steering_delta_deg:.2}"));
    lines.push(format!("step_distance_m={:.4}", capture.step_distance));
    if let Some(candidate) = capture.candidate_before_ground {
        lines.push(format!(
            "proposed_before_ground={}",
            format_global(candidate, layout)
        ));
    }
    if let Some(grounded) = capture.grounded_candidate {
        lines.push(format!(
            "grounded_proposed={}",
            format_global(grounded, layout)
        ));
    }
    lines.push(format!(
        "point_legal={}",
        capture.point_legal.unwrap_or(false)
    ));
    lines.push(format!(
        "point_block={}",
        capture.point_block_reason.as_deref().unwrap_or("none")
    ));
    lines.push(format!(
        "segment_legal={}",
        capture.segment_legal.unwrap_or(false)
    ));
    lines.push(format!(
        "segment_block={}",
        capture.segment_block_reason.as_deref().unwrap_or("none")
    ));
    lines.push(format!(
        "blueprint_support_current={}",
        capture.blueprint_support_blocks_current.unwrap_or(false)
    ));
    lines.push(format!(
        "blueprint_support_proposed={}",
        capture.blueprint_support_blocks_proposed.unwrap_or(false)
    ));
    lines.push(format!(
        "current_in_access_corridor={}",
        capture.current_in_access_corridor.unwrap_or(false)
    ));
    lines.push(format!(
        "proposed_in_access_corridor={}",
        capture.proposed_in_access_corridor.unwrap_or(false)
    ));
    lines.push(format!("progressed_m={:.4}", capture.progressed_distance));
    lines.push(format!("movement_accepted={}", capture.movement_accepted));
    lines.push(format!("apply_blocked_ran={}", capture.apply_blocked_ran));
    lines.push(format!("blocked_class={block_label}"));
    lines.push(format!("waypoint_advanced={}", capture.waypoint_advanced));
    lines.push(format!("position_changed={}", capture.position_changed));
    lines.push(format!(
        "suppress_return_space={}",
        capture.suppress_return_space.as_deref().unwrap_or("none")
    ));
    lines.push(format!(
        "inside_portal_trigger={}",
        capture.inside_portal_trigger
    ));
    lines.push(format!(
        "portal_transition_ran={}",
        capture.portal_transition_ran
    ));
    lines.push(format!("outcome={}", capture.outcome_label));
    session.tick_lines.push(lines.join("\n"));

    let jitter_detected = capture.apply_blocked_ran
        && session.loop_detector.observe_blocked(
            capture.waypoint_index,
            capture.effective_index,
            &block_label,
            capture.current_position,
            capture
                .grounded_candidate
                .unwrap_or(capture.current_position),
            steering_delta_deg,
            layout,
        );
    let max_ticks = session.tick_count >= MAX_SESSION_TICKS;
    let arrived = capture.outcome_label == "arrived";
    if jitter_detected || max_ticks || arrived {
        if jitter_detected {
            append_jitter_summary(session, capture.current_position, layout);
        }
        emit_session(world, capture.unit_id);
    }
}

#[cfg(feature = "dev")]
fn append_jitter_summary(
    session: &mut PostExitJitterSession,
    current_position: WorldPosition,
    layout: ChunkLayout,
) {
    let detector = &session.loop_detector;
    let unit_displacement = detector
        .loop_start_position
        .map(|start| xz_distance(start, current_position, layout))
        .unwrap_or(0.0);
    session.tick_lines.push(format!(
        "jitter_loop_detected=true repeated_waypoint_index={} repeated_effective_index={} repeated_block={} unit_displacement_m={:.4} candidate={} steering_delta_deg_range=[{:.2},{:.2}]",
        detector
            .repeated_waypoint_index
            .map(|v| v.to_string())
            .unwrap_or_else(|| "none".to_string()),
        detector
            .repeated_effective_index
            .map(|v| v.to_string())
            .unwrap_or_else(|| "none".to_string()),
        detector
            .repeated_block_reason
            .as_deref()
            .unwrap_or("none"),
        unit_displacement,
        detector
            .loop_candidate
            .map(|candidate| format_global(candidate, layout))
            .unwrap_or_else(|| "none".to_string()),
        detector.steering_delta_min_deg,
        detector.steering_delta_max_deg,
    ));
}

#[cfg(feature = "dev")]
fn emit_session(world: &mut WorldData, unit_id: UnitId) {
    let trace = world.post_exit_jitter_trace_mut();
    let session = match trace.active.as_ref() {
        Some(session) if session.unit_id == unit_id => session.clone(),
        _ => return,
    };
    if session.emitted {
        return;
    }
    if let Some(active) = trace.active.as_mut() {
        active.emitted = true;
    }
    emit_and_clear(
        world,
        unit_id,
        format_session_header(&session, world.layout()),
    );
}

#[cfg(feature = "dev")]
fn emit_and_clear(world: &mut WorldData, unit_id: UnitId, body_lines: Vec<String>) {
    let mut lines = body_lines;
    if !lines.first().is_some_and(|line| line.starts_with('[')) {
        lines.insert(0, TRACE_MARKER.to_string());
    }
    crate::logging::append_log_block(
        crate::logging::NAVIGATION_TRACE_LOG_PATH,
        "# chasma navigation trace",
        &lines.join("\n"),
    );
    world.post_exit_jitter_trace_mut().clear_active();
}

#[cfg(feature = "dev")]
fn format_session_header(session: &PostExitJitterSession, layout: ChunkLayout) -> Vec<String> {
    let mut lines = vec![TRACE_MARKER.to_string()];
    lines.push(format!("session={}", session.session_id));
    lines.push(format!("unit=U-{:04}", session.unit_id.raw()));
    lines.push(format!("portal_id={}", session.portal_id.raw()));
    lines.push(format!("interior_space={}", session.interior_space.raw()));
    lines.push(format!("surface_space={}", session.surface_space.raw()));
    lines.push(format!(
        "portal_from_center={}",
        format_global(session.portal_from_center, layout)
    ));
    lines.push(format!(
        "escape_position={}",
        format_global(session.escape_position, layout)
    ));
    lines.push(format!(
        "ordered_goal={}",
        format_global(session.ordered_goal, layout)
    ));
    lines.push(format!("agent_radius_m={:.3}", session.agent_radius_m));
    lines.push(format!(
        "path_waypoint_count={}",
        session.path_waypoint_count
    ));
    lines.push(format!(
        "start_waypoint_index={}",
        session.start_waypoint_index
    ));
    lines.push(format!(
        "surface_grid_spacing_m={:.2}",
        session.surface_grid_spacing_m
    ));
    lines.push(format!(
        "start_position={}",
        format_global(session.start_position, layout)
    ));
    for waypoint in &session.post_exit_waypoints {
        lines.push(format!(
            "wp[{}] xz=({:.2},{:.2}) space={} portal={} dist_prev_m={:.3}",
            waypoint.index,
            waypoint.position.to_global(layout).x,
            waypoint.position.to_global(layout).z,
            waypoint.space_id.raw(),
            waypoint
                .portal_id
                .map(|id| id.raw().to_string())
                .unwrap_or_else(|| "none".to_string()),
            waypoint.distance_from_previous_m,
        ));
    }
    lines.extend(session.tick_lines.clone());
    lines
}

fn point_in_any_entrance_access_corridor(
    world: &WorldData,
    point_xz: Vec2,
    agent_radius_meters: f32,
) -> bool {
    let layout = world.layout();
    let registry = world.space_registry();
    for runtime in world.building_navigation_runtime().iter() {
        for region in &runtime.regions {
            if surface_position_in_entrance_access_corridor(
                registry,
                layout,
                runtime.building_id,
                region,
                point_xz,
                agent_radius_meters,
            ) {
                return true;
            }
        }
    }
    false
}

fn summarize_post_exit_waypoints(
    path: &NavigationPath,
    start_index: usize,
    layout: ChunkLayout,
) -> Vec<WaypointSummary> {
    let mut summaries = Vec::new();
    let mut previous = path
        .waypoints
        .get(start_index.saturating_sub(1))
        .map(|wp| wp.position);
    for (index, waypoint) in path.waypoints.iter().enumerate().skip(start_index) {
        let distance_from_previous_m = previous
            .map(|prev| xz_distance(prev, waypoint.position, layout))
            .unwrap_or(0.0);
        summaries.push(WaypointSummary {
            index,
            position: waypoint.position,
            space_id: waypoint.space_id,
            portal_id: waypoint.portal_id,
            distance_from_previous_m,
        });
        previous = Some(waypoint.position);
    }
    summaries
}

fn angular_delta_degrees(planned: Vec2, final_dir: Vec2) -> f32 {
    if planned.length_squared() <= 1e-8 || final_dir.length_squared() <= 1e-8 {
        return 0.0;
    }
    let dot = planned
        .normalize()
        .dot(final_dir.normalize())
        .clamp(-1.0, 1.0);
    dot.acos().to_degrees()
}

fn format_global(position: WorldPosition, layout: ChunkLayout) -> String {
    let global = position.to_global(layout);
    format!("({:.2},{:.2},{:.2})", global.x, global.y, global.z)
}

#[cfg(not(feature = "dev"))]
pub struct PostExitStepCapture;

#[cfg(not(feature = "dev"))]
impl PostExitStepCapture {
    pub fn maybe_begin(
        _: &mut WorldData,
        _: UnitId,
        _: SpaceId,
        _: WorldPosition,
        _: &NavigationPath,
        _: usize,
        _: usize,
        _: NavigationWaypoint,
        _: WorldPosition,
    ) -> Option<Self> {
        None
    }
    pub fn set_movement_vectors(&mut self, _: Vec2, _: Vec2, _: Vec2, _: f32, _: WorldPosition) {}
    pub fn set_legality_probe(
        &mut self,
        _: PassabilityCatalogs<'_>,
        _: &WorldData,
        _: PassabilityAgent,
        _: SpaceId,
        _: WorldPosition,
    ) {
    }
    pub fn set_portal_state(
        &mut self,
        _: &WorldData,
        _: ChunkLayout,
        _: SpaceId,
        _: WorldPosition,
        _: bool,
    ) {
    }
    pub fn finish(self, _: &mut WorldData, _: PassabilityCatalogs<'_>) {}
}

#[cfg(not(feature = "dev"))]
pub fn arm_session_after_interior_surface_exit(
    _: &mut WorldData,
    _: UnitId,
    _: SpaceId,
    _: SpaceId,
    _: PortalId,
    _: WorldPosition,
    _: NavigationPath,
    _: usize,
    _: WorldPosition,
    _: f32,
) {
}

#[cfg(not(feature = "dev"))]
pub fn record_new_order_during_session(
    _: &mut WorldData,
    _: UnitId,
    _: usize,
    _: usize,
    _: WorldPosition,
    _: WorldPosition,
    _: SpaceId,
    _: &NavigationPath,
    _: WorldPosition,
    _: SpaceId,
    _: Option<&NavigationPath>,
) {
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jitter_loop_detector_flags_repeated_blocked_signature() {
        let layout = ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        };
        let start = WorldPosition::from_global(Vec3::new(10.0, 0.0, 10.0), layout);
        let mut detector = JitterLoopDetector::default();
        for tick in 0..JITTER_LOOP_TICKS - 1 {
            assert!(
                !detector.observe_blocked(3, 3, "BlockedByBuilding", start, start, 1.0, layout),
                "blocked signature should not trip before {JITTER_LOOP_TICKS} ticks (tick={tick})"
            );
        }
        assert!(detector.observe_blocked(3, 3, "BlockedByBuilding", start, start, 2.0, layout));
    }

    #[cfg(feature = "dev")]
    #[test]
    fn session_is_bounded_by_max_ticks() {
        use crate::world::{
            BuildingCatalog, ChunkCoord, DoodadCatalog, FootprintCatalog, LocalPosition,
            NavigationWaypoint,
        };

        let layout = ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        };
        let mut world = WorldData::new(layout);
        let unit_id = UnitId::new(1);
        let surface = SpaceId::SURFACE;
        let start = WorldPosition::from_global(Vec3::new(10.0, 0.0, 10.0), layout);
        world.post_exit_jitter_trace_mut().active = Some(PostExitJitterSession {
            session_id: 1,
            unit_id,
            portal_id: PortalId::new(7),
            interior_space: SpaceId::new(1),
            surface_space: surface,
            portal_from_center: start,
            escape_position: start,
            ordered_goal: start,
            agent_radius_m: 0.6,
            path_waypoint_count: 2,
            start_waypoint_index: 1,
            surface_grid_spacing_m: 4.0,
            start_position: start,
            post_exit_waypoints: Vec::new(),
            tick_lines: Vec::new(),
            tick_count: 0,
            loop_position_anchor: Some(start),
            loop_detector: JitterLoopDetector::default(),
            emitted: false,
        });
        let path = NavigationPath::new(vec![NavigationWaypoint::in_space(start, surface)]);
        let catalogs = PassabilityCatalogs {
            doodad: &DoodadCatalog::default(),
            building: &BuildingCatalog::default(),
            footprint: &FootprintCatalog::default(),
        };
        for _ in 0..MAX_SESSION_TICKS {
            let mut capture = PostExitStepCapture::maybe_begin(
                &mut world,
                unit_id,
                surface,
                start,
                &path,
                0,
                0,
                NavigationWaypoint::in_space(start, surface),
                start,
            )
            .expect("capture");
            capture.outcome_label = "moved".to_string();
            capture.movement_accepted = true;
            capture.finish(&mut world, catalogs);
            if !world.post_exit_jitter_trace().is_active_for(unit_id) {
                break;
            }
        }
        assert!(!world.post_exit_jitter_trace().is_active_for(unit_id));
    }
}
