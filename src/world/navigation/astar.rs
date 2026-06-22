//! Deterministic A* over the navigation grid (ADR-032).

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

use super::grid::{
    diagonal_corner_clear, grid_cell_world_position, is_cell_walkable, neighbor_step_cost,
    GridCoord, NavigationAgent, NEIGHBOR_OFFSETS, NavigationConfig,
};
use crate::world::{DoodadCatalog, WorldData, WorldPosition};

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
        // BinaryHeap is a max-heap; invert f so the lowest f-score is popped first.
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

/// Run A* between grid cells and return grounded world waypoints (goal inclusive).
pub fn astar_path(
    world: &WorldData,
    doodad_catalog: &DoodadCatalog,
    config: NavigationConfig,
    agent: NavigationAgent,
    start: GridCoord,
    goal: GridCoord,
) -> Option<Vec<WorldPosition>> {
    if start == goal {
        return grid_cell_world_position(world, goal, config).map(|p| vec![p]);
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
            return Some(reconstruct_path(world, config, &came_from, current.coord));
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
            if !is_cell_walkable(
                world,
                doodad_catalog,
                config,
                agent,
                next,
            ) {
                continue;
            }
            if !diagonal_corner_clear(
                world,
                doodad_catalog,
                config,
                agent,
                current.coord,
                dx,
                dz,
            ) {
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
    world: &WorldData,
    config: NavigationConfig,
    came_from: &HashMap<GridCoord, GridCoord>,
    mut current: GridCoord,
) -> Vec<WorldPosition> {
    let mut cells = vec![current];
    while let Some(&prev) = came_from.get(&current) {
        current = prev;
        cells.push(current);
    }
    cells.reverse();

    cells
        .into_iter()
        .filter_map(|coord| grid_cell_world_position(world, coord, config))
        .collect()
}
