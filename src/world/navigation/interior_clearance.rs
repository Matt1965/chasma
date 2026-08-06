//! Interior region clearance and grid discretization diagnostics (IN-11d prep).

use bevy::prelude::*;

use super::grid::{
    GridCoord, NavigationAgent, NavigationConfig, cell_walkability_sample_globals,
    grid_cell_center_global, grid_coord_at_global_xz, grid_coord_at_position,
    is_cell_walkable_in_space, is_position_walkable_in_space,
};
use super::simplify::is_segment_walkable_in_space;
use crate::world::{
    BuildingNavigationRuntimeStore, ChunkLayout, PassabilityCatalogs, SpaceId, SpaceRegistry,
    WorldData, WorldPosition, ground_position_in_space, interior_position_walkable,
    point_in_polygon_xz,
};

/// Why an interior navigation cell failed the walkability probe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteriorCellFailureReason {
    /// No sample grounded in the destination space.
    GroundingFailure,
    /// Sample center is outside the region polygon.
    SampleOutsidePolygon,
    /// Center is inside but at least one inset sample is outside (agent footprint concern).
    AgentRadiusCrossesBoundary,
    /// Point is inside the polygon but passability rejected it.
    PassabilityFailure,
}

/// One interior navigation cell probe.
#[derive(Debug, Clone, PartialEq)]
pub struct InteriorCellProbe {
    pub coord: GridCoord,
    pub center_global_xz: Vec2,
    pub permissive_pass: bool,
    pub strict_pass: bool,
    pub failure: Option<InteriorCellFailureReason>,
}

/// Measurements for one interior region relative to an agent and optional path endpoints.
#[derive(Debug, Clone, PartialEq)]
pub struct InteriorRegionClearanceReport {
    pub space_id: SpaceId,
    pub blueprint_local_width_meters: f32,
    pub blueprint_local_depth_meters: f32,
    pub runtime_width_meters: f32,
    pub runtime_depth_meters: f32,
    pub agent_radius_meters: f32,
    pub agent_diameter_meters: f32,
    pub portal_landing_min_edge_clearance_meters: Option<f32>,
    pub goal_min_edge_clearance_meters: Option<f32>,
    pub portal_landing_inside: bool,
    pub goal_inside: bool,
    pub interior_cell_spacing_meters: f32,
    pub cells_inside_region: usize,
    pub permissive_walkable_cells: usize,
    pub strict_walkable_cells: usize,
    pub connected_walkable_component: usize,
    pub direct_segment_clear: bool,
    pub cell_probes: Vec<InteriorCellProbe>,
}

/// Signed distance from `point` to the nearest polygon edge (negative when inside).
pub fn signed_distance_to_polygon_edges(point: Vec2, polygon: &[Vec2]) -> f32 {
    if polygon.len() < 3 {
        return f32::INFINITY;
    }
    let inside = point_in_polygon_xz(polygon, point);
    let mut min_dist = f32::INFINITY;
    let mut count = polygon.len();
    for index in 0..count {
        let a = polygon[index];
        let b = polygon[(index + 1) % count];
        let edge = b - a;
        let len_sq = edge.length_squared();
        if len_sq <= f32::EPSILON {
            continue;
        }
        let t = ((point - a).dot(edge) / len_sq).clamp(0.0, 1.0);
        let closest = a + edge * t;
        min_dist = min_dist.min(point.distance(closest));
    }
    if inside { -min_dist } else { min_dist }
}

/// Minimum unsigned distance from `point` to any polygon edge.
pub fn min_edge_clearance_meters(point: Vec2, polygon: &[Vec2]) -> f32 {
    crate::world::min_edge_clearance_meters(point, polygon)
}

/// Axis-aligned span of polygon vertices.
pub fn polygon_axis_span(polygon: &[Vec2]) -> (f32, f32) {
    if polygon.is_empty() {
        return (0.0, 0.0);
    }
    let mut min = polygon[0];
    let mut max = polygon[0];
    for point in polygon.iter().skip(1) {
        min = min.min(*point);
        max = max.max(*point);
    }
    (max.x - min.x, max.y - min.y)
}

/// Inset polygon toward its centroid by `distance` meters (visual aid, not authoritative).
pub fn inset_polygon_toward_centroid(polygon: &[Vec2], distance: f32) -> Vec<Vec2> {
    if polygon.len() < 3 || distance <= 0.0 {
        return polygon.to_vec();
    }
    let centroid = polygon.iter().fold(Vec2::ZERO, |acc, p| acc + *p) / polygon.len() as f32;
    polygon
        .iter()
        .map(|vertex| {
            let to_center = centroid - *vertex;
            let len = to_center.length();
            if len <= f32::EPSILON {
                *vertex
            } else {
                *vertex + to_center / len * distance
            }
        })
        .collect()
}

fn probe_interior_cell(
    world: &WorldData,
    space_registry: &SpaceRegistry,
    nav_store: &BuildingNavigationRuntimeStore,
    catalogs: PassabilityCatalogs<'_>,
    config: NavigationConfig,
    agent: NavigationAgent,
    space_id: SpaceId,
    coord: GridCoord,
    polygon: &[Vec2],
    layout: ChunkLayout,
) -> InteriorCellProbe {
    let center = grid_cell_center_global(coord, config);
    let center_xz = Vec2::new(center.x, center.z);
    let permissive = is_cell_walkable_in_space(
        world,
        space_registry,
        catalogs,
        config,
        agent,
        coord,
        space_id,
    );
    let strict = super::grid::is_cell_walkable(world, catalogs, config, agent, coord);
    let failure = if permissive {
        None
    } else {
        classify_cell_failure(
            world,
            space_registry,
            nav_store,
            catalogs,
            config,
            agent,
            space_id,
            coord,
            polygon,
            layout,
        )
    };
    InteriorCellProbe {
        coord,
        center_global_xz: center_xz,
        permissive_pass: permissive,
        strict_pass: strict,
        failure,
    }
}

fn classify_cell_failure(
    world: &WorldData,
    space_registry: &SpaceRegistry,
    nav_store: &BuildingNavigationRuntimeStore,
    catalogs: PassabilityCatalogs<'_>,
    config: NavigationConfig,
    agent: NavigationAgent,
    space_id: SpaceId,
    coord: GridCoord,
    polygon: &[Vec2],
    layout: ChunkLayout,
) -> Option<InteriorCellFailureReason> {
    let samples = cell_walkability_sample_globals(coord, config, agent.radius_meters);
    let mut center_inside = false;
    let mut any_inside = false;
    let mut any_grounded = false;
    let mut any_passable = false;

    for global in samples {
        let sample_xz = Vec2::new(global.x, global.z);
        if point_in_polygon_xz(polygon, sample_xz) {
            any_inside = true;
        }
        if matches!(samples.first(), Some(first) if sample_xz == Vec2::new(first.x, first.z)) {
            center_inside = point_in_polygon_xz(polygon, sample_xz);
        }
        let position = WorldPosition::from_global(global, layout);
        let Some(grounded) = ground_position_in_space(world, space_registry, space_id, position)
        else {
            continue;
        };
        any_grounded = true;
        if interior_position_walkable(nav_store, space_registry, layout, grounded, space_id)
            && is_position_walkable_in_space(
                world,
                space_registry,
                catalogs,
                grounded,
                agent,
                space_id,
            )
        {
            any_passable = true;
        }
    }

    if !any_grounded {
        return Some(InteriorCellFailureReason::GroundingFailure);
    }
    if !any_inside {
        return Some(InteriorCellFailureReason::SampleOutsidePolygon);
    }
    if center_inside && !any_passable {
        return Some(InteriorCellFailureReason::PassabilityFailure);
    }
    if center_inside {
        return Some(InteriorCellFailureReason::AgentRadiusCrossesBoundary);
    }
    Some(InteriorCellFailureReason::SampleOutsidePolygon)
}

fn flood_fill_walkable_component(
    start: GridCoord,
    walkable: &std::collections::BTreeSet<GridCoord>,
) -> usize {
    let mut queue = std::collections::VecDeque::from([start]);
    let mut seen = std::collections::BTreeSet::from([start]);
    let mut count = 0usize;
    while let Some(cell) = queue.pop_front() {
        if !walkable.contains(&cell) {
            continue;
        }
        count += 1;
        for (dx, dz) in super::grid::NEIGHBOR_OFFSETS {
            let next = GridCoord::new(cell.x + dx, cell.z + dz);
            if seen.insert(next) {
                queue.push_back(next);
            }
        }
    }
    count
}

/// Build a clearance report for one runtime interior region.
pub fn measure_interior_region_clearance(
    world: &WorldData,
    space_registry: &SpaceRegistry,
    nav_store: &BuildingNavigationRuntimeStore,
    catalogs: PassabilityCatalogs<'_>,
    config: &NavigationConfig,
    agent: NavigationAgent,
    space_id: SpaceId,
    blueprint_local_outline: &[Vec2],
    portal_landing: Option<WorldPosition>,
    goal: Option<WorldPosition>,
) -> Option<InteriorRegionClearanceReport> {
    let region = nav_store.region_for_space(space_id)?;
    let polygon = &region.world_outline_xz;
    let layout = world.layout();
    let space_config = config.config_for_space(space_id);
    let (runtime_width, runtime_depth) = polygon_axis_span(polygon);
    let (local_width, local_depth) = polygon_axis_span(blueprint_local_outline);

    let mut probes = Vec::new();
    let mut permissive_walkable = std::collections::BTreeSet::new();
    let spacing = space_config.cell_spacing_meters;
    let min_x = polygon.iter().map(|p| p.x).fold(f32::INFINITY, f32::min);
    let max_x = polygon
        .iter()
        .map(|p| p.x)
        .fold(f32::NEG_INFINITY, f32::max);
    let min_z = polygon.iter().map(|p| p.y).fold(f32::INFINITY, f32::min);
    let max_z = polygon
        .iter()
        .map(|p| p.y)
        .fold(f32::NEG_INFINITY, f32::max);
    let x0 = (min_x / spacing).floor() as i32;
    let x1 = (max_x / spacing).ceil() as i32;
    let z0 = (min_z / spacing).floor() as i32;
    let z1 = (max_z / spacing).ceil() as i32;

    for z in z0..=z1 {
        for x in x0..=x1 {
            let coord = GridCoord::new(x, z);
            let center = grid_cell_center_global(coord, space_config);
            if !point_in_polygon_xz(polygon, Vec2::new(center.x, center.z)) {
                continue;
            }
            let probe = probe_interior_cell(
                world,
                space_registry,
                nav_store,
                catalogs,
                space_config,
                agent,
                space_id,
                coord,
                polygon,
                layout,
            );
            if probe.permissive_pass {
                permissive_walkable.insert(coord);
            }
            probes.push(probe);
        }
    }

    let portal_landing_min = portal_landing.map(|position| {
        let xz = position.to_global(layout).xz();
        min_edge_clearance_meters(xz, polygon)
    });
    let goal_min = goal.map(|position| {
        let xz = position.to_global(layout).xz();
        min_edge_clearance_meters(xz, polygon)
    });
    let portal_inside = portal_landing.is_some_and(|position| {
        interior_position_walkable(nav_store, space_registry, layout, position, space_id)
    });
    let goal_inside = goal.is_some_and(|position| {
        interior_position_walkable(nav_store, space_registry, layout, position, space_id)
    });

    let connected = portal_landing
        .and_then(|landing| {
            let grounded = ground_position_in_space(world, space_registry, space_id, landing)?;
            let start_cell = grid_coord_at_position(grounded, layout, space_config);
            Some(flood_fill_walkable_component(
                start_cell,
                &permissive_walkable,
            ))
        })
        .unwrap_or(0);

    let direct_segment_clear = match (portal_landing, goal) {
        (Some(start), Some(end)) => is_segment_walkable_in_space(
            world,
            space_registry,
            catalogs,
            *config,
            space_id,
            agent,
            start,
            end,
            layout,
        ),
        _ => false,
    };

    Some(InteriorRegionClearanceReport {
        space_id,
        blueprint_local_width_meters: local_width,
        blueprint_local_depth_meters: local_depth,
        runtime_width_meters: runtime_width,
        runtime_depth_meters: runtime_depth,
        agent_radius_meters: agent.radius_meters,
        agent_diameter_meters: agent.radius_meters * 2.0,
        portal_landing_min_edge_clearance_meters: portal_landing_min,
        goal_min_edge_clearance_meters: goal_min,
        portal_landing_inside: portal_inside,
        goal_inside: goal_inside,
        interior_cell_spacing_meters: spacing,
        cells_inside_region: probes.len(),
        permissive_walkable_cells: permissive_walkable.len(),
        strict_walkable_cells: probes.iter().filter(|p| p.strict_pass).count(),
        connected_walkable_component: connected,
        direct_segment_clear,
        cell_probes: probes,
    })
}
