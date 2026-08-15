//! Cross-space navigation path stitching (ADR-083 B6).

use bevy::prelude::*;

use super::astar::astar_path_in_space;
use super::entrance_interior_anchor::resolve_entrance_interior_planning_anchor;
use super::grid::{
    NavigationAgent, NavigationConfig, grid_cell_center_global, is_position_walkable_in_space,
    resolve_path_endpoint_cell,
};
use super::path::{NavigationPath, xz_distance};
use super::query::NavigationError;
use super::simplify::{is_segment_walkable_in_space, simplify_navigation_path_in_space};
use super::waypoint::NavigationWaypoint;
use crate::world::{
    ChunkLayout, PassabilityCatalogs, PortalRecord, PortalType, SpaceId, SpaceRegistry,
    UnitOwnership, WorldData, WorldPosition, ground_position_in_space,
    resolve_surface_entrance_approach_position, resolve_surface_entrance_escape_position,
    space_route_for_unit,
};

/// Request a navigation path that may cross space boundaries via portals.
pub fn find_path_in_spaces(
    world: &WorldData,
    space_registry: &SpaceRegistry,
    catalogs: PassabilityCatalogs<'_>,
    config: &NavigationConfig,
    agent_radius_meters: f32,
    max_slope_degrees: f32,
    start: WorldPosition,
    goal: WorldPosition,
    start_space: SpaceId,
    goal_space: SpaceId,
    unit_ownership: Option<UnitOwnership>,
) -> Result<NavigationPath, NavigationError> {
    if start_space == goal_space {
        return find_path_single_space(
            world,
            space_registry,
            catalogs,
            config,
            agent_radius_meters,
            max_slope_degrees,
            start,
            goal,
            start_space,
        );
    }

    let route = space_route_for_unit(world, start_space, goal_space, unit_ownership)
        .or_else(|| space_registry.space_route(start_space, goal_space))
        .ok_or(NavigationError::NoPath)?;

    let agent = NavigationAgent {
        radius_meters: agent_radius_meters,
        max_slope_degrees,
    };
    let layout = world.layout();

    let mut waypoints: Vec<NavigationWaypoint> = Vec::new();
    let mut current_space = start_space;
    let mut current_pos = ground_position_in_space(world, space_registry, start_space, start)
        .ok_or(NavigationError::TerrainUnavailable)?;
    let route_goal_space = goal_space;
    let mut leg_index = 0u32;

    for portal_id in route {
        leg_index += 1;
        let portal = space_registry
            .get_portal(portal_id)
            .ok_or(NavigationError::NoPath)?;

        let portal_entry_pos = portal
            .trigger_world_position_in_space(current_space, layout, world, space_registry)
            .ok_or(NavigationError::NoPath)?;

        let segment =
            if portal.portal_type == PortalType::ExteriorEntrance && current_space.is_surface() {
                surface_entrance_approach_segment(
                    world,
                    space_registry,
                    catalogs,
                    config,
                    agent,
                    portal,
                    current_pos,
                    portal_entry_pos,
                    current_space,
                )?
            } else {
                let segment_result = path_segment_in_space(
                    world,
                    space_registry,
                    catalogs,
                    config,
                    agent,
                    current_pos,
                    portal_entry_pos,
                    current_space,
                );
                #[cfg(feature = "dev")]
                if let Err(error) = &segment_result {
                    if super::cross_space_leg_trace::should_trace_reverse_interior_leg1(
                        start_space,
                        route_goal_space,
                        current_space,
                        portal,
                    ) {
                        super::cross_space_leg_trace::record_reverse_interior_leg1_failure(
                            world,
                            space_registry,
                            catalogs,
                            config,
                            agent,
                            start_space,
                            route_goal_space,
                            leg_index,
                            portal_id,
                            portal,
                            current_space,
                            current_pos,
                            portal_entry_pos,
                            *error,
                        );
                    }
                }
                segment_result?
            };
        append_segment(&mut waypoints, segment, layout);

        let mut portal_waypoint =
            NavigationWaypoint::portal_transition(portal_entry_pos, current_space, portal_id);

        let (dest_space, authored_dest) = portal
            .destination_for_planning(current_space, layout, world, space_registry)
            .ok_or(NavigationError::NoPath)?;
        let dest_pos =
            if portal.portal_type == PortalType::ExteriorEntrance && !dest_space.is_surface() {
                resolve_entrance_interior_planning_anchor(
                    world,
                    space_registry,
                    catalogs,
                    portal,
                    dest_space,
                    layout,
                    *config,
                    agent,
                )
                .ok_or(NavigationError::StartBlocked)?
            } else {
                authored_dest
            };
        if portal.portal_type == PortalType::ExteriorEntrance {
            portal_waypoint.portal_interior_destination = Some(dest_pos);
        }
        waypoints.push(portal_waypoint);

        let exited_interior_to_surface =
            dest_space.is_surface() && portal.portal_type == PortalType::ExteriorEntrance;
        current_space = dest_space;
        current_pos = dest_pos;

        if exited_interior_to_surface {
            if let Some(escape_pos) = resolve_surface_entrance_escape_position(
                world,
                space_registry,
                portal,
                agent.radius_meters,
            ) {
                let escape_segment = surface_entrance_escape_segment(
                    world,
                    space_registry,
                    catalogs,
                    config,
                    agent,
                    current_pos,
                    escape_pos,
                )?;
                append_segment(&mut waypoints, escape_segment, layout);
                current_pos = escape_pos;
            }
        }
    }

    let final_segment = path_segment_in_space(
        world,
        space_registry,
        catalogs,
        config,
        agent,
        current_pos,
        goal,
        goal_space,
    )?;
    append_segment(&mut waypoints, final_segment, layout);

    if waypoints.is_empty() {
        return Err(NavigationError::NoPath);
    }
    if let Some(last) = waypoints.last_mut() {
        if let Some(grounded) = ground_position_in_space(world, space_registry, goal_space, goal) {
            last.position = grounded;
            last.space_id = goal_space;
        }
    }

    Ok(NavigationPath::new(waypoints))
}

fn surface_entrance_approach_segment(
    world: &WorldData,
    space_registry: &SpaceRegistry,
    catalogs: PassabilityCatalogs<'_>,
    config: &NavigationConfig,
    agent: NavigationAgent,
    portal: &PortalRecord,
    start: WorldPosition,
    portal_entry: WorldPosition,
    space_id: SpaceId,
) -> Result<Vec<NavigationWaypoint>, NavigationError> {
    let layout = world.layout();
    let approach_pos = resolve_surface_entrance_approach_position(
        world,
        space_registry,
        portal,
        agent.radius_meters,
    )
    .ok_or(NavigationError::NoPath)?;
    let grounded_entry = ground_position_in_space(world, space_registry, space_id, portal_entry)
        .ok_or(NavigationError::TerrainUnavailable)?;

    let to_approach = surface_corridor_transit_segment(
        world,
        space_registry,
        catalogs,
        config,
        agent,
        start,
        approach_pos,
        space_id,
    )?;
    let approach_end = to_approach
        .last()
        .map(|waypoint| waypoint.position)
        .unwrap_or(approach_pos);
    let to_portal = surface_corridor_transit_segment(
        world,
        space_registry,
        catalogs,
        config,
        agent,
        approach_end,
        grounded_entry,
        space_id,
    )?;

    let mut segment = to_approach;
    append_segment(&mut segment, to_portal, layout);
    Ok(segment)
}

fn surface_entrance_escape_segment(
    world: &WorldData,
    space_registry: &SpaceRegistry,
    catalogs: PassabilityCatalogs<'_>,
    config: &NavigationConfig,
    agent: NavigationAgent,
    start: WorldPosition,
    escape: WorldPosition,
) -> Result<Vec<NavigationWaypoint>, NavigationError> {
    surface_corridor_transit_segment(
        world,
        space_registry,
        catalogs,
        config,
        agent,
        start,
        escape,
        SpaceId::SURFACE,
    )
}

fn surface_corridor_transit_segment(
    world: &WorldData,
    space_registry: &SpaceRegistry,
    catalogs: PassabilityCatalogs<'_>,
    config: &NavigationConfig,
    agent: NavigationAgent,
    start: WorldPosition,
    goal: WorldPosition,
    space_id: SpaceId,
) -> Result<Vec<NavigationWaypoint>, NavigationError> {
    let layout = world.layout();
    let grounded_start = ground_position_in_space(world, space_registry, space_id, start)
        .ok_or(NavigationError::TerrainUnavailable)?;
    let grounded_goal = ground_position_in_space(world, space_registry, space_id, goal)
        .ok_or(NavigationError::TerrainUnavailable)?;

    if xz_distance(grounded_start, grounded_goal, layout) <= WAYPOINT_POSITION_DEDUPE_METERS {
        return Ok(vec![NavigationWaypoint::in_space(grounded_goal, space_id)]);
    }

    if is_segment_walkable_in_space(
        world,
        space_registry,
        catalogs,
        *config,
        space_id,
        agent,
        grounded_start,
        grounded_goal,
        layout,
    ) {
        return Ok(vec![
            NavigationWaypoint::in_space(grounded_start, space_id),
            NavigationWaypoint::in_space(grounded_goal, space_id),
        ]);
    }

    path_segment_in_space(
        world,
        space_registry,
        catalogs,
        config,
        agent,
        grounded_start,
        grounded_goal,
        space_id,
    )
}

/// Matches [`dedupe_consecutive_positions`] and executor arrival tolerance.
const WAYPOINT_POSITION_DEDUPE_METERS: f32 = 0.05;

fn append_segment(
    path: &mut Vec<NavigationWaypoint>,
    segment: Vec<NavigationWaypoint>,
    layout: ChunkLayout,
) {
    for waypoint in segment {
        if path.last().is_some_and(|last| {
            last.portal_id.is_none()
                && waypoint.portal_id.is_none()
                && last.space_id == waypoint.space_id
                && xz_distance(last.position, waypoint.position, layout)
                    <= WAYPOINT_POSITION_DEDUPE_METERS
        }) {
            continue;
        }
        path.push(waypoint);
    }
}

fn path_segment_in_space(
    world: &WorldData,
    space_registry: &SpaceRegistry,
    catalogs: PassabilityCatalogs<'_>,
    config: &NavigationConfig,
    agent: NavigationAgent,
    start: WorldPosition,
    goal: WorldPosition,
    space_id: SpaceId,
) -> Result<Vec<NavigationWaypoint>, NavigationError> {
    find_path_single_space(
        world,
        space_registry,
        catalogs,
        config,
        agent.radius_meters,
        agent.max_slope_degrees,
        start,
        goal,
        space_id,
    )
    .map(|path| path.waypoints)
}

fn find_path_single_space(
    world: &WorldData,
    space_registry: &SpaceRegistry,
    catalogs: PassabilityCatalogs<'_>,
    config: &NavigationConfig,
    agent_radius_meters: f32,
    max_slope_degrees: f32,
    start: WorldPosition,
    goal: WorldPosition,
    space_id: SpaceId,
) -> Result<NavigationPath, NavigationError> {
    let agent = NavigationAgent {
        radius_meters: agent_radius_meters,
        max_slope_degrees,
    };
    let layout = world.layout();
    let space_config = config.config_for_space(space_id);

    let grounded_start = ground_position_in_space(world, space_registry, space_id, start)
        .ok_or(NavigationError::TerrainUnavailable)?;
    let grounded_goal = ground_position_in_space(world, space_registry, space_id, goal)
        .ok_or(NavigationError::TerrainUnavailable)?;

    let start_cell = resolve_path_endpoint_cell(
        world,
        space_registry,
        catalogs,
        space_config,
        agent,
        space_id,
        grounded_start,
        layout,
    )
    .ok_or(NavigationError::StartBlocked)?;
    let goal_cell = resolve_path_endpoint_cell(
        world,
        space_registry,
        catalogs,
        space_config,
        agent,
        space_id,
        grounded_goal,
        layout,
    )
    .ok_or(NavigationError::GoalBlocked)?;

    if !is_position_walkable_in_space(
        world,
        space_registry,
        catalogs,
        grounded_start,
        agent,
        space_id,
    ) {
        return Err(NavigationError::StartBlocked);
    }
    if !is_position_walkable_in_space(
        world,
        space_registry,
        catalogs,
        grounded_goal,
        agent,
        space_id,
    ) {
        return Err(NavigationError::GoalBlocked);
    }

    if is_segment_walkable_in_space(
        world,
        space_registry,
        catalogs,
        *config,
        space_id,
        agent,
        grounded_start,
        grounded_goal,
        layout,
    ) {
        return Ok(NavigationPath::new(vec![
            NavigationWaypoint::in_space(grounded_start, space_id),
            NavigationWaypoint::in_space(grounded_goal, space_id),
        ]));
    }

    if start_cell == goal_cell {
        return Ok(NavigationPath::new(vec![NavigationWaypoint::in_space(
            grounded_goal,
            space_id,
        )]));
    }

    let mut positions = astar_path_in_space(
        world,
        space_registry,
        catalogs,
        *config,
        agent,
        start_cell,
        goal_cell,
        space_id,
    )
    .ok_or(NavigationError::NoPath)?;

    if positions.is_empty() {
        let global = grid_cell_center_global(goal_cell, space_config);
        let candidate = WorldPosition::from_global(global, layout);
        if let Some(goal_pos) = ground_position_in_space(world, space_registry, space_id, candidate)
        {
            positions.push(goal_pos);
        } else {
            return Err(NavigationError::NoPath);
        }
    }

    trim_waypoints_at_start(&mut positions, grounded_start, layout);
    positions.insert(0, grounded_start);
    if positions
        .last()
        .is_none_or(|last| xz_distance(*last, grounded_goal, layout) > 0.05)
    {
        positions.push(grounded_goal);
    }
    if let Some(last) = positions.last_mut() {
        *last = grounded_goal;
    }

    simplify_navigation_path_in_space(
        &mut positions,
        world,
        space_registry,
        catalogs,
        *config,
        space_id,
        agent,
        layout,
    );
    dedupe_consecutive_positions(&mut positions, layout);

    Ok(NavigationPath::new(
        positions
            .into_iter()
            .map(|position| NavigationWaypoint::in_space(position, space_id))
            .collect(),
    ))
}

fn trim_waypoints_at_start(
    waypoints: &mut Vec<WorldPosition>,
    start: WorldPosition,
    layout: crate::world::ChunkLayout,
) {
    const EPSILON: f32 = 0.25;
    while let Some(first) = waypoints.first().copied() {
        if xz_distance(start, first, layout) <= EPSILON {
            waypoints.remove(0);
        } else {
            break;
        }
    }
}

fn dedupe_consecutive_positions(
    waypoints: &mut Vec<WorldPosition>,
    layout: crate::world::ChunkLayout,
) {
    const EPSILON: f32 = 0.05;
    let mut index = 0;
    while index + 1 < waypoints.len() {
        if xz_distance(waypoints[index], waypoints[index + 1], layout) <= EPSILON {
            waypoints.remove(index + 1);
        } else {
            index += 1;
        }
    }
}
