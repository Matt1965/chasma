//! Deterministic A* over the navigation grid (ADR-032).

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

use super::grid::{
    GridCoord, NEIGHBOR_OFFSETS, NavigationAgent, NavigationConfig, diagonal_corner_clear,
    diagonal_corner_clear_in_space, grid_cell_center_global, grid_cell_world_position,
    is_cell_walkable, is_cell_walkable_in_space, neighbor_step_cost,
};
use crate::world::{
    ChunkLayout, PassabilityCatalogs, SpaceId, SpaceRegistry, WorldData, WorldPosition,
    ground_position_in_space,
};

const MAX_SEARCH_NODES: usize = 16_384;

#[derive(Debug, Clone, Copy, PartialEq)]
struct SearchNode {
    coord: GridCoord,
    g: f32,
    h: f32,
}

impl Eq for SearchNode {}

impl PartialOrd for SearchNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SearchNode {
    fn cmp(&self, other: &Self) -> Ordering {
        let f_self = self.g + self.h;
        let f_other = other.g + other.h;
        f_other
            .total_cmp(&f_self)
            .then_with(|| other.h.total_cmp(&self.h))
            .then_with(|| self.coord.z.cmp(&other.coord.z))
            .then_with(|| self.coord.x.cmp(&other.coord.x))
    }
}

fn octile_heuristic(a: GridCoord, b: GridCoord, cell_spacing_meters: f32) -> f32 {
    let dx = (a.x - b.x).abs();
    let dz = (a.z - b.z).abs();
    let (min, max) = if dx < dz { (dx, dz) } else { (dz, dx) };
    (max as f32 + (std::f32::consts::SQRT_2 - 1.0) * min as f32) * cell_spacing_meters
}

struct AstarOutcome {
    path: Vec<WorldPosition>,
    expanded: usize,
}

/// Run A* between grid cells and return grounded world waypoints (goal inclusive).
pub fn astar_path(
    world: &WorldData,
    catalogs: PassabilityCatalogs<'_>,
    config: NavigationConfig,
    agent: NavigationAgent,
    start: GridCoord,
    goal: GridCoord,
) -> Option<Vec<WorldPosition>> {
    let space_config = config.config_for_space(SpaceId::SURFACE);
    run_astar(
        space_config,
        agent,
        start,
        goal,
        |coord| is_cell_walkable(world, catalogs, space_config, agent, coord),
        |from, dx, dz| diagonal_corner_clear(world, catalogs, space_config, agent, from, dx, dz),
        |coord| grid_cell_world_position(world, coord, space_config),
    )
    .map(|outcome| outcome.path)
}

/// Space-aware A* (NV1.3).
pub fn astar_path_in_space(
    world: &WorldData,
    space_registry: &SpaceRegistry,
    catalogs: PassabilityCatalogs<'_>,
    config: NavigationConfig,
    agent: NavigationAgent,
    start: GridCoord,
    goal: GridCoord,
    space_id: SpaceId,
) -> Option<Vec<WorldPosition>> {
    astar_path_in_space_with_stats(
        world,
        space_registry,
        catalogs,
        config,
        agent,
        start,
        goal,
        space_id,
    )
    .map(|(path, _)| path)
}

pub(crate) fn astar_path_in_space_with_stats(
    world: &WorldData,
    space_registry: &SpaceRegistry,
    catalogs: PassabilityCatalogs<'_>,
    config: NavigationConfig,
    agent: NavigationAgent,
    start: GridCoord,
    goal: GridCoord,
    space_id: SpaceId,
) -> Option<(Vec<WorldPosition>, usize)> {
    let space_config = config.config_for_space(space_id);
    let layout = world.layout();
    run_astar(
        space_config,
        agent,
        start,
        goal,
        |coord| {
            is_cell_walkable_in_space(
                world,
                space_registry,
                catalogs,
                space_config,
                agent,
                coord,
                space_id,
            )
        },
        |from, dx, dz| {
            if dx == 0 || dz == 0 {
                return true;
            }
            diagonal_corner_clear_in_space(
                world,
                space_registry,
                catalogs,
                space_config,
                agent,
                from,
                dx,
                dz,
                space_id,
                layout,
            )
        },
        |coord| {
            grid_cell_world_position_in_space(
                world,
                space_registry,
                layout,
                coord,
                space_config,
                space_id,
            )
        },
    )
    .map(|outcome| (outcome.path, outcome.expanded))
}

fn grid_cell_world_position_in_space(
    world: &WorldData,
    space_registry: &SpaceRegistry,
    layout: ChunkLayout,
    coord: GridCoord,
    config: NavigationConfig,
    space_id: SpaceId,
) -> Option<WorldPosition> {
    let global = grid_cell_center_global(coord, config);
    let position = WorldPosition::from_global(global, layout);
    ground_position_in_space(world, space_registry, space_id, position)
}

fn run_astar(
    config: NavigationConfig,
    agent: NavigationAgent,
    start: GridCoord,
    goal: GridCoord,
    mut is_walkable: impl FnMut(GridCoord) -> bool,
    mut corner_clear: impl FnMut(GridCoord, i32, i32) -> bool,
    ground_cell: impl Fn(GridCoord) -> Option<WorldPosition>,
) -> Option<AstarOutcome> {
    if start == goal {
        return ground_cell(goal).map(|position| AstarOutcome {
            path: vec![position],
            expanded: 0,
        });
    }

    let mut open = BinaryHeap::new();
    open.push(SearchNode {
        coord: start,
        g: 0.0,
        h: octile_heuristic(start, goal, config.cell_spacing_meters),
    });

    let mut came_from: HashMap<GridCoord, GridCoord> = HashMap::new();
    let mut g_score: HashMap<GridCoord, f32> = HashMap::from([(start, 0.0)]);
    let mut expanded = 0usize;

    while let Some(current) = open.pop() {
        if current.coord == goal {
            return Some(AstarOutcome {
                path: reconstruct_path(&came_from, current.coord, ground_cell),
                expanded,
            });
        }

        expanded += 1;
        if expanded > MAX_SEARCH_NODES {
            return None;
        }

        let Some(&current_g) = g_score.get(&current.coord) else {
            continue;
        };
        if current.g > current_g + 1e-4 {
            continue;
        }

        for &(dx, dz) in &NEIGHBOR_OFFSETS {
            let next = GridCoord::new(current.coord.x + dx, current.coord.z + dz);
            if !is_walkable(next) {
                continue;
            }
            if !corner_clear(current.coord, dx, dz) {
                continue;
            }

            let tentative = current_g + neighbor_step_cost(dx, dz, config.cell_spacing_meters);
            let better = g_score
                .get(&next)
                .is_none_or(|&existing| tentative < existing - 1e-4);
            if !better {
                continue;
            }

            came_from.insert(next, current.coord);
            g_score.insert(next, tentative);
            open.push(SearchNode {
                coord: next,
                g: tentative,
                h: octile_heuristic(next, goal, config.cell_spacing_meters),
            });
        }
    }

    None
}

fn reconstruct_path(
    came_from: &HashMap<GridCoord, GridCoord>,
    mut current: GridCoord,
    ground_cell: impl Fn(GridCoord) -> Option<WorldPosition>,
) -> Vec<WorldPosition> {
    let mut cells = vec![current];
    while let Some(&prev) = came_from.get(&current) {
        current = prev;
        cells.push(current);
    }
    cells.reverse();

    cells.into_iter().filter_map(ground_cell).collect()
}

#[cfg(test)]
pub(crate) const MAX_ASTAR_SEARCH_NODES: usize = MAX_SEARCH_NODES;
