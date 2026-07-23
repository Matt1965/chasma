//! Cross-space navigation path stitching (ADR-083 B6).

use bevy::prelude::*;

use super::astar::astar_path_in_space as run_astar_in_space;
use super::grid::{
    GridCoord, NavigationAgent, NavigationConfig, grid_cell_world_position, grid_coord_at_position,
    is_position_walkable,
};
use super::path::{NavigationPath, xz_distance};
use super::query::NavigationError;
use super::simplify::simplify_navigation_path;
use super::waypoint::NavigationWaypoint;
use crate::world::{
    PassabilityAgent, PassabilityCatalogs, PassabilityResult, PortalRecord, SpaceId, SpaceRegistry,
    UnitOwnership, WorldData, WorldPosition, ground_position_in_space, query_passability_in_space,
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

    for portal_id in route {
        let portal = space_registry
            .get_portal(portal_id)
            .ok_or(NavigationError::NoPath)?;

        let portal_entry_pos = if portal.from_space == current_space {
            portal_to_entry_position(portal, layout)
        } else if portal.bidirectional {
            portal.to_position
        } else {
            return Err(NavigationError::NoPath);
        };

        let segment = path_segment_in_space(
            world,
            space_registry,
            catalogs,
            config,
            agent,
            current_pos,
            portal_entry_pos,
            current_space,
        )?;
        append_segment(&mut waypoints, segment);

        let dest_space = if portal.from_space == current_space {
            portal.to_space
        } else {
            portal.from_space
        };
        let dest_pos =
            ground_position_in_space(world, space_registry, dest_space, portal.to_position)
                .ok_or(NavigationError::TerrainUnavailable)?;

        waypoints.push(NavigationWaypoint::portal_transition(
            dest_pos, dest_space, portal_id,
        ));

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

fn portal_to_entry_position(
    portal: &PortalRecord,
    layout: crate::world::ChunkLayout,
) -> WorldPosition {
    let global = Vec3::new(
        portal.from_center_global_xz.x,
        0.0,
        portal.from_center_global_xz.y,
    );
    WorldPosition::from_global(global, layout)
}

fn append_segment(path: &mut Vec<NavigationWaypoint>, segment: Vec<NavigationWaypoint>) {
    for waypoint in segment {
        if path.last().is_some_and(|last| {
            last.position == waypoint.position && last.space_id == waypoint.space_id
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

    let grounded_start = ground_position_in_space(world, space_registry, space_id, start)
        .ok_or(NavigationError::TerrainUnavailable)?;
    let grounded_goal = ground_position_in_space(world, space_registry, space_id, goal)
        .ok_or(NavigationError::TerrainUnavailable)?;

    let start_cell = grid_coord_at_position(grounded_start, layout, *config);
    let goal_cell = grid_coord_at_position(grounded_goal, layout, *config);

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
        if let Some(goal_pos) =
            grid_cell_world_position_in_space(world, space_registry, goal_cell, *config, space_id)
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

    simplify_navigation_path(&mut positions, world, catalogs, *config, agent, layout);
    dedupe_consecutive_positions(&mut positions, layout);

    Ok(NavigationPath::new(
        positions
            .into_iter()
            .map(|position| NavigationWaypoint::in_space(position, space_id))
            .collect(),
    ))
}

fn astar_path_in_space(
    world: &WorldData,
    space_registry: &SpaceRegistry,
    catalogs: PassabilityCatalogs<'_>,
    config: NavigationConfig,
    agent: NavigationAgent,
    start: GridCoord,
    goal: GridCoord,
    space_id: SpaceId,
) -> Option<Vec<WorldPosition>> {
    let positions = run_astar_in_space(
        world,
        space_registry,
        catalogs,
        config,
        agent,
        start,
        goal,
        space_id,
    )?;
    let mut grounded = Vec::new();
    for position in positions {
        grounded.push(ground_position_in_space(
            world,
            space_registry,
            space_id,
            position,
        )?);
    }
    Some(grounded)
}

fn grid_cell_world_position_in_space(
    world: &WorldData,
    space_registry: &SpaceRegistry,
    coord: GridCoord,
    config: NavigationConfig,
    space_id: SpaceId,
) -> Option<WorldPosition> {
    let position = grid_cell_world_position(world, coord, config)?;
    ground_position_in_space(world, space_registry, space_id, position)
}

pub fn is_position_walkable_in_space(
    world: &WorldData,
    space_registry: &SpaceRegistry,
    catalogs: PassabilityCatalogs<'_>,
    position: WorldPosition,
    agent: NavigationAgent,
    space_id: SpaceId,
) -> bool {
    let Some(grounded) = ground_position_in_space(world, space_registry, space_id, position) else {
        return false;
    };
    let layout = world.layout();
    if space_id.is_surface() {
        return is_position_walkable(world, catalogs, grounded, agent)
            || crate::world::position_in_surface_entrance_portal(
                world.space_registry(),
                layout,
                grounded,
            );
    }
    matches!(
        query_passability_in_space(
            world,
            catalogs,
            grounded,
            PassabilityAgent::from(agent),
            space_id,
        ),
        PassabilityResult::Passable { .. }
    )
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
