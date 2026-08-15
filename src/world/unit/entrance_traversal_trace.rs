//! Bounded dev diagnostic trace for Surface → Interior entrance traversal (IN-11gI-T).

use bevy::prelude::*;

use super::id::UnitId;
use crate::world::{
    BuildingId, ChunkLayout, NavigationConfig, PassabilityCatalogs, PortalId, PortalRecord,
    PortalType, SpaceId, SpaceRegistry, UnitCatalog, UnitOwnership, WorldData, WorldPosition,
    infer_navigation_membership_at_position, interior_navigation_move_target_at_position,
    resolve_navigation_space_at_position,
};

/// One bounded entrance-traversal diagnostic session.
#[derive(Debug, Clone, Default)]
pub struct EntranceTraversalTrace {
    active: Option<EntranceTraversalSession>,
}

#[derive(Debug, Clone)]
pub struct EntranceTraversalSession {
    pub unit_id: UnitId,
    pub unit_position: WorldPosition,
    pub tracked_space: SpaceId,
    pub positional_space: SpaceId,
    pub collision_radius_meters: f32,
    pub click_raw: Option<WorldPosition>,
    pub goal_position: Option<WorldPosition>,
    pub goal_space: Option<SpaceId>,
    pub goal_inside_region: Option<bool>,
    pub goal_region_label: Option<String>,
    pub building_id: Option<BuildingId>,
    pub blueprint_id: Option<String>,
    pub runtime_hydrated: Option<bool>,
    pub target_region_key: Option<String>,
    pub connection_portal_ids: Option<Vec<u32>>,
    pub connection_found: Option<bool>,
    pub entrance_key: Option<String>,
    pub owning_edge_index: Option<u32>,
    pub edge_endpoint_a: Option<Vec2>,
    pub edge_endpoint_b: Option<Vec2>,
    pub portal_from_radius_m: Option<f32>,
    pub effective_opening_width_m: Option<f32>,
    pub staging_xz: Option<Vec2>,
    pub threshold_xz: Option<Vec2>,
    pub landing_xz: Option<Vec2>,
    pub authored_landing_legal: Option<bool>,
    pub resolved_interior_anchor_xz: Option<Vec2>,
    pub resolved_anchor_legal: Option<bool>,
    pub interior_continuation_result: Option<String>,
    pub portal_id: Option<u32>,
    pub portal_enabled: Option<bool>,
    pub door_state: Option<String>,
    pub start_space: Option<SpaceId>,
    pub path_goal_space: Option<SpaceId>,
    pub cross_space: Option<bool>,
    pub path_result: Option<String>,
    pub path_waypoint_count: Option<u32>,
    pub route_leg_labels: Option<String>,
    pub surface_approach_start: Option<WorldPosition>,
    pub surface_approach_goal: Option<WorldPosition>,
    pub surface_direct_path: Option<bool>,
    pub surface_waypoint_count: Option<u32>,
    pub surface_final_waypoint_near_entrance_m: Option<f32>,
    pub segment_start: Option<WorldPosition>,
    pub segment_end: Option<WorldPosition>,
    pub boundary_crosses: Option<bool>,
    pub entrance_opening_match: Option<bool>,
    pub universal_segment_legal: Option<bool>,
    pub transition_trigger_type: Option<String>,
    pub transition_center_xz: Option<Vec2>,
    pub transition_radius_m: Option<f32>,
    pub transition_unit_xz: Option<Vec2>,
    pub transition_distance_m: Option<f32>,
    pub transition_contained: Option<bool>,
    pub transition_from_space: Option<SpaceId>,
    pub transition_to_space: Option<SpaceId>,
    pub transition_permitted: Option<bool>,
    pub membership_before: Option<SpaceId>,
    pub membership_after: Option<SpaceId>,
    pub interior_step_position: Option<WorldPosition>,
    pub interior_step_waypoint: Option<WorldPosition>,
    pub interior_point_legal: Option<bool>,
    pub interior_segment_legal: Option<bool>,
    pub interior_move_applied: Option<bool>,
    pub first_failure: Option<String>,
    pub emitted: bool,
}

impl EntranceTraversalTrace {
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
    resolved_goal: WorldPosition,
    unit_catalog: &UnitCatalog,
) {
    let record = match world.get_unit(unit_id) {
        Some(record) => record,
        None => return,
    };
    if !record.current_space_id.is_surface() {
        return;
    }
    let layout = world.layout();
    let goal_space = infer_navigation_membership_at_position(world, resolved_goal);
    if goal_space.is_surface() {
        return;
    }
    let runtime = world.building_navigation_runtime();
    let registry = world.space_registry();
    let position = record.placement.position;
    let positional_space =
        resolve_navigation_space_at_position(runtime, registry, layout, position);
    let goal_probe = probe_goal_region(runtime, registry, layout, resolved_goal, goal_space);
    let collision_radius_meters = unit_catalog
        .get(&record.definition_id)
        .map(|def| def.collision_radius_meters)
        .unwrap_or(0.0);

    world.entrance_traversal_trace_mut().active = Some(EntranceTraversalSession {
        unit_id,
        unit_position: position,
        tracked_space: record.current_space_id,
        positional_space,
        collision_radius_meters,
        click_raw: Some(raw_click),
        goal_position: Some(resolved_goal),
        goal_space: Some(goal_space),
        goal_inside_region: Some(goal_probe.inside),
        goal_region_label: goal_probe.label,
        building_id: None,
        blueprint_id: None,
        runtime_hydrated: None,
        target_region_key: goal_probe.region_key,
        connection_portal_ids: None,
        connection_found: None,
        entrance_key: None,
        owning_edge_index: None,
        edge_endpoint_a: None,
        edge_endpoint_b: None,
        portal_from_radius_m: None,
        effective_opening_width_m: None,
        staging_xz: None,
        threshold_xz: None,
        landing_xz: None,
        authored_landing_legal: None,
        resolved_interior_anchor_xz: None,
        resolved_anchor_legal: None,
        interior_continuation_result: None,
        portal_id: None,
        portal_enabled: None,
        door_state: None,
        start_space: None,
        path_goal_space: None,
        cross_space: None,
        path_result: None,
        path_waypoint_count: None,
        route_leg_labels: None,
        surface_approach_start: None,
        surface_approach_goal: None,
        surface_direct_path: None,
        surface_waypoint_count: None,
        surface_final_waypoint_near_entrance_m: None,
        segment_start: None,
        segment_end: None,
        boundary_crosses: None,
        entrance_opening_match: None,
        universal_segment_legal: None,
        transition_trigger_type: None,
        transition_center_xz: None,
        transition_radius_m: None,
        transition_unit_xz: None,
        transition_distance_m: None,
        transition_contained: None,
        transition_from_space: None,
        transition_to_space: None,
        transition_permitted: None,
        membership_before: None,
        membership_after: None,
        interior_step_position: None,
        interior_step_waypoint: None,
        interior_point_legal: None,
        interior_segment_legal: None,
        interior_move_applied: None,
        first_failure: None,
        emitted: false,
    });
}

#[cfg(feature = "dev")]
struct GoalProbe {
    inside: bool,
    label: Option<String>,
    region_key: Option<String>,
}

#[cfg(feature = "dev")]
fn probe_goal_region(
    store: &crate::world::BuildingNavigationRuntimeStore,
    registry: &SpaceRegistry,
    layout: ChunkLayout,
    position: WorldPosition,
    goal_space: SpaceId,
) -> GoalProbe {
    if let Some(runtime) = store.get_for_space(goal_space) {
        if let Some(region) = store.region_for_space(goal_space) {
            return GoalProbe {
                inside: true,
                label: Some(format!(
                    "building#{} {}/{}",
                    runtime.building_id.raw(),
                    region.floor_key,
                    region.region_key
                )),
                region_key: Some(region.region_key.clone()),
            };
        }
    }
    let inside =
        interior_navigation_move_target_at_position(store, registry, layout, position).is_some();
    GoalProbe {
        inside,
        label: None,
        region_key: None,
    }
}

#[cfg(feature = "dev")]
pub fn record_pathfinding_probe(
    world: &mut WorldData,
    unit_id: UnitId,
    catalogs: PassabilityCatalogs<'_>,
    nav_config: &NavigationConfig,
    agent_radius_meters: f32,
    max_slope_degrees: f32,
    start: WorldPosition,
    grounded_goal: WorldPosition,
    start_space: SpaceId,
    goal_space: SpaceId,
    unit_ownership: Option<UnitOwnership>,
    path_result: Result<&crate::world::NavigationPath, crate::world::NavigationError>,
) {
    if !world.entrance_traversal_trace().is_active_for(unit_id) {
        return;
    }

    let layout = world.layout();
    let registry = world.space_registry().clone();
    let runtime_store = world.building_navigation_runtime();

    let building_id = runtime_store
        .get_for_space(goal_space)
        .map(|rt| rt.building_id);
    let blueprint_id = runtime_store
        .get_for_space(goal_space)
        .map(|rt| rt.blueprint_id.as_str().to_string());
    let runtime_hydrated = runtime_store.get_for_space(goal_space).is_some();

    let route = crate::world::space_route_for_unit(world, start_space, goal_space, unit_ownership)
        .or_else(|| registry.space_route(start_space, goal_space));
    let connection_found = route.is_some();

    let portal_geometry = route
        .as_ref()
        .and_then(|route| route.first().copied())
        .map(|portal_id| collect_portal_geometry(world, portal_id, goal_space, layout));

    let surface_probe = route
        .as_ref()
        .and_then(|route| route.first().copied())
        .map(|portal_id| {
            probe_surface_approach(
                world,
                catalogs,
                nav_config,
                agent_radius_meters,
                max_slope_degrees,
                start,
                start_space,
                portal_id,
            )
        });

    let interior_anchor_probe = route
        .as_ref()
        .and_then(|route| route.first().copied())
        .and_then(|portal_id| {
            probe_interior_anchor(
                world,
                &registry,
                catalogs,
                nav_config,
                agent_radius_meters,
                max_slope_degrees,
                start_space,
                grounded_goal,
                goal_space,
                portal_id,
            )
        });

    let path_fields = match path_result {
        Ok(path) => (
            "success".to_string(),
            Some(path.len() as u32),
            Some(describe_route_legs(path)),
            None,
        ),
        Err(error) => (
            format!("{error:?}"),
            None,
            None,
            Some(format!("ROUTE_CONSTRUCTION:{error:?}")),
        ),
    };

    let session = world
        .entrance_traversal_trace_mut()
        .active
        .as_mut()
        .unwrap();
    session.start_space = Some(start_space);
    session.path_goal_space = Some(goal_space);
    session.cross_space = Some(start_space != goal_space);
    session.building_id = building_id;
    session.blueprint_id = blueprint_id;
    session.runtime_hydrated = Some(runtime_hydrated);
    session.connection_found = Some(connection_found);

    if let Some(route) = route {
        session.connection_portal_ids = Some(route.iter().map(|id| id.raw()).collect());
        if let Some(geometry) = portal_geometry {
            apply_portal_geometry(session, geometry);
        }
        if let Some(probe) = surface_probe {
            let surface_failed = probe
                .first_failure
                .as_ref()
                .is_some_and(|failure| failure.starts_with("SURFACE_APPROACH"));
            apply_surface_probe(session, probe);
            if surface_failed {
                emit_session(world, unit_id);
                return;
            }
        }
        if let Some(probe) = interior_anchor_probe {
            apply_interior_anchor_probe(session, probe);
        }
    } else {
        session.first_failure = Some("CONNECTION_GRAPH".to_string());
        emit_session(world, unit_id);
        return;
    }

    session.path_result = Some(path_fields.0);
    session.path_waypoint_count = path_fields.1;
    session.route_leg_labels = path_fields.2;
    if let Some(failure) = path_fields.3 {
        session.first_failure = Some(failure);
        emit_session(world, unit_id);
    }
}

#[cfg(feature = "dev")]
fn describe_route_legs(path: &crate::world::NavigationPath) -> String {
    let mut legs = Vec::new();
    let mut current = "surface_approach";
    for wp in &path.waypoints {
        if wp.portal_id.is_some() {
            legs.push(current.to_string());
            current = "portal_transition";
        }
    }
    legs.push(current.to_string());
    legs.push("interior_continuation".to_string());
    legs.join(" -> ")
}

#[cfg(feature = "dev")]
struct PortalGeometryData {
    portal_id: u32,
    portal_enabled: bool,
    entrance_key: Option<String>,
    portal_from_radius_m: f32,
    staging_xz: Vec2,
    threshold_xz: Vec2,
    landing_xz: Vec2,
    transition_trigger_type: String,
    transition_from_space: SpaceId,
    transition_to_space: SpaceId,
    door_state: Option<String>,
    owning_edge_index: Option<u32>,
    edge_endpoint_a: Option<Vec2>,
    edge_endpoint_b: Option<Vec2>,
}

#[cfg(feature = "dev")]
#[derive(Clone)]
struct SurfaceApproachProbe {
    surface_approach_start: WorldPosition,
    surface_approach_goal: WorldPosition,
    surface_direct_path: bool,
    surface_waypoint_count: u32,
    surface_final_waypoint_near_entrance_m: Option<f32>,
    segment_start: WorldPosition,
    segment_end: WorldPosition,
    boundary_crosses: bool,
    entrance_opening_match: bool,
    universal_segment_legal: bool,
    first_failure: Option<String>,
}

#[cfg(feature = "dev")]
fn apply_portal_geometry(session: &mut EntranceTraversalSession, geometry: PortalGeometryData) {
    session.portal_id = Some(geometry.portal_id);
    session.portal_enabled = Some(geometry.portal_enabled);
    session.entrance_key = geometry.entrance_key;
    session.portal_from_radius_m = Some(geometry.portal_from_radius_m);
    session.effective_opening_width_m = Some(geometry.portal_from_radius_m * 2.0);
    session.staging_xz = Some(geometry.staging_xz);
    session.threshold_xz = Some(geometry.threshold_xz);
    session.landing_xz = Some(geometry.landing_xz);
    session.transition_trigger_type = Some(geometry.transition_trigger_type);
    session.transition_center_xz = Some(geometry.staging_xz);
    session.transition_radius_m = Some(geometry.portal_from_radius_m);
    session.transition_from_space = Some(geometry.transition_from_space);
    session.transition_to_space = Some(geometry.transition_to_space);
    session.door_state = geometry.door_state;
    session.owning_edge_index = geometry.owning_edge_index;
    session.edge_endpoint_a = geometry.edge_endpoint_a;
    session.edge_endpoint_b = geometry.edge_endpoint_b;
}

#[cfg(feature = "dev")]
fn apply_surface_probe(session: &mut EntranceTraversalSession, probe: SurfaceApproachProbe) {
    session.surface_approach_start = Some(probe.surface_approach_start);
    session.surface_approach_goal = Some(probe.surface_approach_goal);
    session.surface_direct_path = Some(probe.surface_direct_path);
    session.surface_waypoint_count = Some(probe.surface_waypoint_count);
    session.surface_final_waypoint_near_entrance_m = probe.surface_final_waypoint_near_entrance_m;
    session.segment_start = Some(probe.segment_start);
    session.segment_end = Some(probe.segment_end);
    session.boundary_crosses = Some(probe.boundary_crosses);
    session.entrance_opening_match = Some(probe.entrance_opening_match);
    session.universal_segment_legal = Some(probe.universal_segment_legal);
    if let Some(failure) = probe.first_failure {
        session.first_failure = Some(failure);
    }
}

#[cfg(feature = "dev")]
fn collect_portal_geometry(
    world: &WorldData,
    portal_id: PortalId,
    goal_space: SpaceId,
    layout: ChunkLayout,
) -> PortalGeometryData {
    let registry = world.space_registry();
    let portal = registry.get_portal(portal_id).expect("portal in route");
    let landing = portal.to_position.to_global(layout);
    let landing_xz = Vec2::new(landing.x, landing.z);
    let threshold_xz = portal.from_center_global_xz.lerp(landing_xz, 0.5);
    let door_state = portal
        .owning_building_id
        .map(|building_id| format_door_state(world, building_id, portal));
    let edge_data = portal.owning_building_id.and_then(|_| {
        world
            .building_navigation_runtime()
            .region_for_space(goal_space)
            .and_then(|region| {
                find_owning_edge(region.world_outline_xz.as_slice(), threshold_xz)
                    .map(|(edge_index, a, b)| (edge_index as u32, a, b))
            })
    });
    PortalGeometryData {
        portal_id: portal_id.raw(),
        portal_enabled: portal.enabled,
        entrance_key: portal_key_for_portal(world, portal),
        portal_from_radius_m: portal.from_radius_meters,
        staging_xz: portal.from_center_global_xz,
        threshold_xz,
        landing_xz,
        transition_trigger_type: portal.portal_type.label().to_string(),
        transition_from_space: portal.from_space,
        transition_to_space: portal.to_space,
        door_state,
        owning_edge_index: edge_data.map(|(index, _, _)| index),
        edge_endpoint_a: edge_data.map(|(_, a, _)| a),
        edge_endpoint_b: edge_data.map(|(_, _, b)| b),
    }
}

#[cfg(feature = "dev")]
fn portal_key_for_portal(world: &WorldData, portal: &PortalRecord) -> Option<String> {
    let building_id = portal.owning_building_id?;
    let runtime = world.building_navigation_runtime().get(building_id)?;
    runtime
        .portal_keys
        .iter()
        .find(|(_, id)| **id == portal.id)
        .map(|(key, _)| key.clone())
}

#[cfg(feature = "dev")]
fn format_door_state(world: &WorldData, building_id: BuildingId, portal: &PortalRecord) -> String {
    if portal.portal_type != PortalType::ExteriorEntrance {
        return "non_entrance_portal".to_string();
    }
    if portal.enabled {
        "doorless_or_enabled".to_string()
    } else {
        let doors = world.door_store().building_door_ids(building_id);
        if doors.is_empty() {
            "no_doors".to_string()
        } else {
            format!("doors={}", doors.len())
        }
    }
}

#[cfg(feature = "dev")]
fn find_owning_edge(polygon: &[Vec2], point: Vec2) -> Option<(usize, Vec2, Vec2)> {
    let mut best: Option<(usize, f32, Vec2, Vec2)> = None;
    for index in 0..polygon.len() {
        let a = polygon[index];
        let b = polygon[(index + 1) % polygon.len()];
        let edge = b - a;
        let len_sq = edge.length_squared();
        if len_sq <= f32::EPSILON {
            continue;
        }
        let t = ((point - a).dot(edge) / len_sq).clamp(0.0, 1.0);
        let projected = a + edge * t;
        let dist = point.distance(projected);
        if best
            .as_ref()
            .is_none_or(|(_, best_dist, _, _)| dist < *best_dist)
        {
            best = Some((index, dist, a, b));
        }
    }
    best.map(|(index, _, a, b)| (index, a, b))
}

#[cfg(feature = "dev")]
#[derive(Clone)]
struct InteriorAnchorProbe {
    authored_landing_legal: bool,
    resolved_interior_anchor_xz: Option<Vec2>,
    resolved_anchor_legal: bool,
    interior_continuation_result: String,
}

#[cfg(feature = "dev")]
fn apply_interior_anchor_probe(session: &mut EntranceTraversalSession, probe: InteriorAnchorProbe) {
    session.authored_landing_legal = Some(probe.authored_landing_legal);
    session.resolved_interior_anchor_xz = probe.resolved_interior_anchor_xz;
    session.resolved_anchor_legal = Some(probe.resolved_anchor_legal);
    session.interior_continuation_result = Some(probe.interior_continuation_result);
}

#[cfg(feature = "dev")]
fn probe_interior_anchor(
    world: &WorldData,
    registry: &SpaceRegistry,
    catalogs: PassabilityCatalogs<'_>,
    nav_config: &NavigationConfig,
    agent_radius_meters: f32,
    max_slope_degrees: f32,
    start_space: SpaceId,
    goal: WorldPosition,
    goal_space: SpaceId,
    portal_id: PortalId,
) -> Option<InteriorAnchorProbe> {
    let layout = world.layout();
    let portal = registry.get_portal(portal_id)?;
    let agent = crate::world::NavigationAgent {
        radius_meters: agent_radius_meters,
        max_slope_degrees,
    };
    let passability_agent = crate::world::PassabilityAgent {
        radius_meters: agent_radius_meters,
        max_slope_degrees,
    };
    let (dest_space, authored) =
        portal.destination_for_planning(start_space, layout, world, registry)?;
    let authored_legal = matches!(
        crate::world::query_navigation_point_legality(
            world,
            catalogs,
            authored,
            passability_agent,
            dest_space,
        ),
        crate::world::PassabilityResult::Passable { .. }
    );

    let resolved = crate::world::resolve_entrance_interior_planning_anchor(
        world,
        registry,
        catalogs,
        portal,
        dest_space,
        layout,
        *nav_config,
        agent,
    );
    if let Some(anchor) = resolved {
        let anchor_legal = matches!(
            crate::world::query_navigation_point_legality(
                world,
                catalogs,
                anchor,
                passability_agent,
                dest_space,
            ),
            crate::world::PassabilityResult::Passable { .. }
        );
        let continuation = crate::world::find_path_with_spaces(
            world,
            catalogs,
            nav_config,
            agent_radius_meters,
            max_slope_degrees,
            anchor,
            goal,
            dest_space,
            goal_space,
            None,
        );
        Some(InteriorAnchorProbe {
            authored_landing_legal: authored_legal,
            resolved_interior_anchor_xz: Some(anchor.to_global(layout).xz()),
            resolved_anchor_legal: anchor_legal,
            interior_continuation_result: match continuation {
                Ok(_) => "success".to_string(),
                Err(error) => format!("{error:?}"),
            },
        })
    } else {
        Some(InteriorAnchorProbe {
            authored_landing_legal: authored_legal,
            resolved_interior_anchor_xz: None,
            resolved_anchor_legal: false,
            interior_continuation_result: "StartBlocked".to_string(),
        })
    }
}

#[cfg(feature = "dev")]
fn probe_surface_approach(
    world: &WorldData,
    catalogs: PassabilityCatalogs<'_>,
    nav_config: &NavigationConfig,
    agent_radius_meters: f32,
    max_slope_degrees: f32,
    start: WorldPosition,
    start_space: SpaceId,
    portal_id: PortalId,
) -> SurfaceApproachProbe {
    let registry = world.space_registry();
    let layout = world.layout();
    let portal = registry.get_portal(portal_id).expect("portal in route");
    let portal_entry = portal
        .trigger_world_position_in_space(start_space, layout, world, registry)
        .expect("portal entry in start space");

    let agent = crate::world::NavigationAgent {
        radius_meters: agent_radius_meters,
        max_slope_degrees,
    };
    let direct = crate::world::is_segment_walkable_in_space(
        world,
        registry,
        catalogs,
        *nav_config,
        start_space,
        agent,
        start,
        portal_entry,
        layout,
    );

    let (surface_waypoint_count, surface_final_near, first_failure) =
        match crate::world::find_path_with_spaces(
            world,
            catalogs,
            nav_config,
            agent_radius_meters,
            max_slope_degrees,
            start,
            portal_entry,
            start_space,
            start_space,
            None,
        ) {
            Ok(path) => {
                let near = path.waypoints.last().map(|last| {
                    last.position
                        .to_global(layout)
                        .xz()
                        .distance(portal.from_center_global_xz)
                });
                (path.len() as u32, near, None)
            }
            Err(error) => (0, None, Some(format!("SURFACE_APPROACH:{error:?}"))),
        };

    SurfaceApproachProbe {
        surface_approach_start: start,
        surface_approach_goal: portal_entry,
        surface_direct_path: direct,
        surface_waypoint_count,
        surface_final_waypoint_near_entrance_m: surface_final_near,
        segment_start: start,
        segment_end: portal_entry,
        boundary_crosses: !crate::world::surface_segment_respects_blueprint_boundaries(
            world,
            start,
            portal_entry,
            layout,
            agent_radius_meters,
        ),
        entrance_opening_match: crate::world::probe_segment_crosses_entrance_opening(
            world,
            start,
            portal_entry,
            agent_radius_meters,
        ),
        universal_segment_legal: direct,
        first_failure,
    }
}

#[cfg(feature = "dev")]
pub fn record_opening_legality_probe(
    world: &mut WorldData,
    unit_id: UnitId,
    from: WorldPosition,
    to: WorldPosition,
    _active_space: SpaceId,
    agent_radius_meters: f32,
    segment_legal: bool,
) {
    if !world.entrance_traversal_trace().is_active_for(unit_id) {
        return;
    }
    let layout = world.layout();
    let segment_fields_missing = world
        .entrance_traversal_trace()
        .active
        .as_ref()
        .is_some_and(|session| session.segment_start.is_none());
    let boundary_crosses = segment_fields_missing.then(|| {
        !crate::world::surface_segment_respects_blueprint_boundaries(
            world,
            from,
            to,
            layout,
            agent_radius_meters,
        )
    });
    let entrance_opening_match = segment_fields_missing.then(|| {
        crate::world::probe_segment_crosses_entrance_opening(world, from, to, agent_radius_meters)
    });
    let session = world
        .entrance_traversal_trace_mut()
        .active
        .as_mut()
        .unwrap();
    if segment_fields_missing {
        session.segment_start = Some(from);
        session.segment_end = Some(to);
        session.boundary_crosses = boundary_crosses;
        session.entrance_opening_match = entrance_opening_match;
        session.universal_segment_legal = Some(segment_legal);
    }
    if !segment_legal || session.boundary_crosses == Some(true) {
        if session.entrance_opening_match != Some(true) {
            session.first_failure = Some("OPENING_LEGALITY".to_string());
            emit_session(world, unit_id);
        }
    }
}

#[cfg(feature = "dev")]
pub fn record_transition_probe(
    world: &mut WorldData,
    unit_id: UnitId,
    current_space: SpaceId,
    position: WorldPosition,
    portal_id: PortalId,
    permitted: bool,
) {
    if !world.entrance_traversal_trace().is_active_for(unit_id) {
        return;
    }
    let layout = world.layout();
    let transition_fields = {
        let registry = world.space_registry();
        let portal = registry.get_portal(portal_id);
        let agent_xz = position.to_global(layout).xz();
        portal.map(|portal| {
            let center = portal.trigger_center_xz_for_space(current_space, layout);
            let distance_m = center.map(|center| agent_xz.distance(center));
            let contained = portal.contains_agent_in_space(agent_xz, current_space, layout);
            (
                center,
                portal.from_radius_meters,
                agent_xz,
                distance_m,
                contained,
                portal.to_space,
            )
        })
    };
    let session = world
        .entrance_traversal_trace_mut()
        .active
        .as_mut()
        .unwrap();
    if let Some((center, radius, agent_xz, distance_m, contained, to_space)) = transition_fields {
        session.transition_center_xz = center;
        session.transition_radius_m = Some(radius);
        session.transition_unit_xz = Some(agent_xz);
        session.transition_distance_m = distance_m;
        session.transition_contained = Some(contained);
        session.transition_from_space = Some(current_space);
        session.transition_to_space = Some(to_space);
    }
    session.transition_permitted = Some(permitted);
    if !permitted {
        session.first_failure = Some("TRANSITION_TRIGGER".to_string());
        emit_session(world, unit_id);
    }
}

#[cfg(feature = "dev")]
pub fn record_membership_update(
    world: &mut WorldData,
    unit_id: UnitId,
    before: SpaceId,
    after: SpaceId,
    expected: SpaceId,
) {
    if !world.entrance_traversal_trace().is_active_for(unit_id) {
        return;
    }
    let session = world
        .entrance_traversal_trace_mut()
        .active
        .as_mut()
        .unwrap();
    session.membership_before = Some(before);
    session.membership_after = Some(after);
    if after != expected {
        session.first_failure = Some("MEMBERSHIP_TRANSITION".to_string());
        emit_session(world, unit_id);
    }
}

#[cfg(feature = "dev")]
pub fn record_interior_first_step(
    world: &mut WorldData,
    unit_id: UnitId,
    position: WorldPosition,
    waypoint: WorldPosition,
    point_legal: bool,
    segment_legal: bool,
    move_applied: bool,
) {
    if !world.entrance_traversal_trace().is_active_for(unit_id) {
        return;
    }
    let session = world
        .entrance_traversal_trace_mut()
        .active
        .as_mut()
        .unwrap();
    if session.interior_step_position.is_some() {
        return;
    }
    session.interior_step_position = Some(position);
    session.interior_step_waypoint = Some(waypoint);
    session.interior_point_legal = Some(point_legal);
    session.interior_segment_legal = Some(segment_legal);
    session.interior_move_applied = Some(move_applied);
    if !session.first_failure.is_some() {
        session.first_failure = Some("ENTRANCE_TRAVERSAL=SUCCESS".to_string());
    }
    emit_session(world, unit_id);
}

#[cfg(feature = "dev")]
fn emit_session(world: &mut WorldData, unit_id: UnitId) {
    let layout = world.layout();
    let trace = world.entrance_traversal_trace_mut();
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
        &format_session_log(&session, layout),
    );
    trace.clear_active();
}

#[cfg(feature = "dev")]
fn format_session_log(session: &EntranceTraversalSession, layout: ChunkLayout) -> String {
    let mut lines = vec!["[ENTRANCE_TRAVERSAL_TRACE]".to_string()];
    lines.push(format!("unit=U-{:04}", session.unit_id.raw()));
    let pos = session.unit_position.to_global(layout);
    lines.push(format!("position=({:.2},{:.2},{:.2})", pos.x, pos.y, pos.z));
    lines.push(format!("tracked_space={}", session.tracked_space.raw()));
    lines.push(format!(
        "positional_space={}",
        session.positional_space.raw()
    ));
    lines.push(format!(
        "collision_radius_m={:.3}",
        session.collision_radius_meters
    ));
    if let Some(goal) = session.goal_position {
        let g = goal.to_global(layout);
        lines.push(format!("goal=({:.2},{:.2},{:.2})", g.x, g.y, g.z));
    }
    lines.push(format!(
        "goal_space={}",
        session
            .goal_space
            .map(|s| s.raw().to_string())
            .unwrap_or_else(|| "none".to_string())
    ));
    lines.push(format!(
        "goal_inside_region={}",
        session.goal_inside_region.unwrap_or(false)
    ));
    lines.push(format!(
        "goal_region={}",
        session.goal_region_label.as_deref().unwrap_or("none")
    ));
    if let Some(id) = session.building_id {
        lines.push(format!("building_id={}", id.raw()));
    }
    lines.push(format!(
        "blueprint_id={}",
        session.blueprint_id.as_deref().unwrap_or("none")
    ));
    lines.push(format!(
        "runtime_hydrated={}",
        session.runtime_hydrated.unwrap_or(false)
    ));
    lines.push(format!(
        "target_region={}",
        session.target_region_key.as_deref().unwrap_or("none")
    ));
    lines.push(format!(
        "connection_found={}",
        session.connection_found.unwrap_or(false)
    ));
    if let Some(ids) = &session.connection_portal_ids {
        lines.push(format!("connection_portals={:?}", ids));
    }
    lines.push(format!(
        "entrance_key={}",
        session.entrance_key.as_deref().unwrap_or("none")
    ));
    if let Some(edge) = session.owning_edge_index {
        lines.push(format!("owning_edge_index={edge}"));
    }
    if let (Some(a), Some(b)) = (session.edge_endpoint_a, session.edge_endpoint_b) {
        lines.push(format!("edge_a=({:.2},{:.2})", a.x, a.y));
        lines.push(format!("edge_b=({:.2},{:.2})", b.x, b.y));
    }
    lines.push(format!(
        "portal_from_radius_m={}",
        session
            .portal_from_radius_m
            .map(|v| format!("{v:.3}"))
            .unwrap_or_else(|| "none".to_string())
    ));
    lines.push(format!(
        "effective_opening_width_m={}",
        session
            .effective_opening_width_m
            .map(|v| format!("{v:.3}"))
            .unwrap_or_else(|| "none".to_string())
    ));
    if let Some(s) = session.staging_xz {
        lines.push(format!("staging=({:.2},{:.2})", s.x, s.y));
    }
    if let Some(t) = session.threshold_xz {
        lines.push(format!("threshold=({:.2},{:.2})", t.x, t.y));
    }
    if let Some(l) = session.landing_xz {
        lines.push(format!("landing=({:.2},{:.2})", l.x, l.y));
    }
    lines.push(format!(
        "authored_landing_legal={}",
        session
            .authored_landing_legal
            .map(|v| v.to_string())
            .unwrap_or_else(|| "none".to_string())
    ));
    if let Some(a) = session.resolved_interior_anchor_xz {
        lines.push(format!("resolved_interior_anchor=({:.2},{:.2})", a.x, a.y));
    }
    lines.push(format!(
        "resolved_anchor_legal={}",
        session
            .resolved_anchor_legal
            .map(|v| v.to_string())
            .unwrap_or_else(|| "none".to_string())
    ));
    lines.push(format!(
        "interior_continuation_result={}",
        session
            .interior_continuation_result
            .as_deref()
            .unwrap_or("none")
    ));
    lines.push(format!(
        "portal_id={}",
        session
            .portal_id
            .map(|v| v.to_string())
            .unwrap_or_else(|| "none".to_string())
    ));
    lines.push(format!(
        "portal_enabled={}",
        session.portal_enabled.unwrap_or(false)
    ));
    lines.push(format!(
        "door_state={}",
        session.door_state.as_deref().unwrap_or("none")
    ));
    lines.push(format!(
        "start_space={}",
        session
            .start_space
            .map(|s| s.raw().to_string())
            .unwrap_or_else(|| "none".to_string())
    ));
    lines.push(format!(
        "path_goal_space={}",
        session
            .path_goal_space
            .map(|s| s.raw().to_string())
            .unwrap_or_else(|| "none".to_string())
    ));
    lines.push(format!(
        "cross_space={}",
        session.cross_space.unwrap_or(false)
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
    lines.push(format!(
        "route_legs={}",
        session.route_leg_labels.as_deref().unwrap_or("none")
    ));
    lines.push(format!(
        "surface_direct_path={}",
        session.surface_direct_path.unwrap_or(false)
    ));
    lines.push(format!(
        "surface_waypoints={}",
        session
            .surface_waypoint_count
            .map(|c| c.to_string())
            .unwrap_or_else(|| "none".to_string())
    ));
    lines.push(format!(
        "surface_final_near_entrance_m={}",
        session
            .surface_final_waypoint_near_entrance_m
            .map(|d| format!("{d:.3}"))
            .unwrap_or_else(|| "none".to_string())
    ));
    lines.push(format!(
        "boundary_crosses={}",
        session.boundary_crosses.unwrap_or(false)
    ));
    lines.push(format!(
        "entrance_opening_match={}",
        session.entrance_opening_match.unwrap_or(false)
    ));
    lines.push(format!(
        "universal_segment_legal={}",
        session.universal_segment_legal.unwrap_or(false)
    ));
    lines.push(format!(
        "transition_type={}",
        session.transition_trigger_type.as_deref().unwrap_or("none")
    ));
    if let Some(d) = session.transition_distance_m {
        lines.push(format!("transition_distance_m={d:.3}"));
    }
    lines.push(format!(
        "transition_contained={}",
        session.transition_contained.unwrap_or(false)
    ));
    lines.push(format!(
        "transition_permitted={}",
        session.transition_permitted.unwrap_or(false)
    ));
    if let (Some(b), Some(a)) = (session.membership_before, session.membership_after) {
        lines.push(format!("membership_before={}", b.raw()));
        lines.push(format!("membership_after={}", a.raw()));
    }
    lines.push(format!(
        "FIRST_FAILURE={}",
        session.first_failure.as_deref().unwrap_or("none")
    ));
    lines.join("\n")
}

#[cfg(not(feature = "dev"))]
pub fn maybe_begin_session(
    _: &mut WorldData,
    _: UnitId,
    _: WorldPosition,
    _: WorldPosition,
    _: &UnitCatalog,
) {
}

#[cfg(not(feature = "dev"))]
pub fn record_pathfinding_probe(
    _: &mut WorldData,
    _: UnitId,
    _: PassabilityCatalogs<'_>,
    _: &NavigationConfig,
    _: f32,
    _: f32,
    _: WorldPosition,
    _: WorldPosition,
    _: SpaceId,
    _: SpaceId,
    _: Option<UnitOwnership>,
    _: Result<&crate::world::NavigationPath, crate::world::NavigationError>,
) {
}

#[cfg(not(feature = "dev"))]
pub fn record_opening_legality_probe(
    _: &mut WorldData,
    _: UnitId,
    _: WorldPosition,
    _: WorldPosition,
    _: SpaceId,
    _: f32,
    _: bool,
) {
}

#[cfg(not(feature = "dev"))]
pub fn record_transition_probe(
    _: &mut WorldData,
    _: UnitId,
    _: SpaceId,
    _: WorldPosition,
    _: PortalId,
    _: bool,
) {
}

#[cfg(not(feature = "dev"))]
pub fn record_membership_update(_: &mut WorldData, _: UnitId, _: SpaceId, _: SpaceId, _: SpaceId) {}

#[cfg(not(feature = "dev"))]
pub fn record_interior_first_step(
    _: &mut WorldData,
    _: UnitId,
    _: WorldPosition,
    _: WorldPosition,
    _: bool,
    _: bool,
    _: bool,
) {
}
