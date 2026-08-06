//! Cross-space navigation path stitching (ADR-083 B6).

use bevy::prelude::*;

use super::astar::astar_path_in_space;
use super::grid::{
    NavigationAgent, NavigationConfig, grid_cell_center_global, is_position_walkable_in_space,
    resolve_path_endpoint_cell,
};
use super::path::{NavigationPath, xz_distance};
use super::query::NavigationError;
use super::simplify::{is_segment_walkable_in_space, simplify_navigation_path_in_space};
use super::waypoint::NavigationWaypoint;
use crate::world::{
    PassabilityCatalogs, PortalRecord, PortalType, SpaceId, SpaceRegistry, UnitOwnership,
    WorldData, WorldPosition, ground_position_in_space, space_route_for_unit,
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

    for portal_id in route {
        let portal = space_registry
            .get_portal(portal_id)
            .ok_or(NavigationError::NoPath)?;

        let portal_entry_pos = portal
            .trigger_world_position_in_space(current_space, layout, world, space_registry)
            .ok_or(NavigationError::NoPath)?;

        let mut segment = path_segment_in_space(
            world,
            space_registry,
            catalogs,
            config,
            agent,
            current_pos,
            portal_entry_pos,
            current_space,
        )?;
        if portal.portal_type == PortalType::ExteriorEntrance
            && (current_space.is_surface()
                || (portal.bidirectional && portal.to_space == current_space))
        {
            segment = entrance_approach_segment(
                world,
                space_registry,
                current_space,
                current_pos,
                portal_entry_pos,
            )?;
        }
        append_segment(&mut waypoints, segment);

        waypoints.push(NavigationWaypoint::portal_transition(
            portal_entry_pos,
            current_space,
            portal_id,
        ));

        let (dest_space, dest_pos) = portal
            .destination_for_planning(current_space, layout, world, space_registry)
            .ok_or(NavigationError::NoPath)?;
        current_space = dest_space;
        current_pos = dest_pos;
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
    append_segment(&mut waypoints, final_segment);

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

fn entrance_approach_segment(
    world: &WorldData,
    space_registry: &SpaceRegistry,
    space_id: SpaceId,
    start: WorldPosition,
    portal_entry: WorldPosition,
) -> Result<Vec<NavigationWaypoint>, NavigationError> {
    let grounded_start = ground_position_in_space(world, space_registry, space_id, start)
        .ok_or(NavigationError::TerrainUnavailable)?;
    let grounded_entry = ground_position_in_space(world, space_registry, space_id, portal_entry)
        .ok_or(NavigationError::TerrainUnavailable)?;
    Ok(vec![
        NavigationWaypoint::in_space(grounded_start, space_id),
        NavigationWaypoint::in_space(grounded_entry, space_id),
    ])
}

fn append_segment(path: &mut Vec<NavigationWaypoint>, segment: Vec<NavigationWaypoint>) {
    for waypoint in segment {
        if path.last().is_some_and(|last| {
            last.portal_id.is_none()
                && waypoint.portal_id.is_none()
                && last.position == waypoint.position
                && last.space_id == waypoint.space_id
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
