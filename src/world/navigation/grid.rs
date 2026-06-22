//! Logical navigation grid derived from terrain and obstacles (ADR-032).

use bevy::prelude::*;

use crate::world::{
    ground_world_position, is_position_blocked_by_doodads, is_position_slope_walkable, ChunkId,
    ChunkLayout, DoodadCatalog, WorldData, WorldPosition,
};

/// Grid configuration for pathfinding (ADR-032).
#[derive(Debug, Clone, Copy, PartialEq, Reflect, Resource)]
pub struct NavigationConfig {
    /// Distance between adjacent navigation cell centers (meters).
    pub cell_spacing_meters: f32,
}

impl Default for NavigationConfig {
    fn default() -> Self {
        Self {
            cell_spacing_meters: 4.0,
        }
    }
}

/// Agent footprint and terrain limits used by pathfinding and LOS checks.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NavigationAgent {
    pub radius_meters: f32,
    pub max_slope_degrees: f32,
}

/// Integer grid coordinate in navigation space (global XZ / spacing).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GridCoord {
    pub x: i32,
    pub z: i32,
}

impl GridCoord {
    pub const fn new(x: i32, z: i32) -> Self {
        Self { x, z }
    }
}

/// Convert global XZ to the containing navigation cell.
pub fn grid_coord_at_global_xz(global: Vec3, config: NavigationConfig) -> GridCoord {
    let spacing = config.cell_spacing_meters;
    GridCoord::new(
        (global.x / spacing).floor() as i32,
        (global.z / spacing).floor() as i32,
    )
}

pub fn grid_coord_at_position(
    position: WorldPosition,
    layout: ChunkLayout,
    config: NavigationConfig,
) -> GridCoord {
    grid_coord_at_global_xz(position.to_global(layout), config)
}

/// Cell center in global XZ (Y filled by terrain grounding).
pub fn grid_cell_center_global(coord: GridCoord, config: NavigationConfig) -> Vec3 {
    let spacing = config.cell_spacing_meters;
    Vec3::new(
        coord.x as f32 * spacing + spacing * 0.5,
        0.0,
        coord.z as f32 * spacing + spacing * 0.5,
    )
}

pub fn grid_cell_world_position(
    world: &WorldData,
    coord: GridCoord,
    config: NavigationConfig,
) -> Option<WorldPosition> {
    let layout = world.layout();
    let global = grid_cell_center_global(coord, config);
    let position = WorldPosition::from_global(global, layout);
    ground_world_position(world, position)
}

/// Whether a grounded position is walkable for the given agent.
pub fn is_position_walkable(
    world: &WorldData,
    doodad_catalog: &DoodadCatalog,
    position: WorldPosition,
    agent: NavigationAgent,
) -> bool {
    let Some(grounded) = ground_world_position(world, position) else {
        return false;
    };
    if is_position_blocked_by_doodads(
        world,
        doodad_catalog,
        grounded,
        agent.radius_meters,
    ) {
        return false;
    }
    is_position_slope_walkable(world, grounded, agent.max_slope_degrees)
}

fn cell_walkability_sample_globals(
    coord: GridCoord,
    config: NavigationConfig,
    agent_radius_meters: f32,
) -> [Vec3; 5] {
    let spacing = config.cell_spacing_meters;
    let center = grid_cell_center_global(coord, config);
    let inset = agent_radius_meters.min(spacing * 0.25);
    let offset = (spacing * 0.5 - inset).max(0.0);
    [
        center,
        center + Vec3::new(offset, 0.0, 0.0),
        center + Vec3::new(-offset, 0.0, 0.0),
        center + Vec3::new(0.0, 0.0, offset),
        center + Vec3::new(0.0, 0.0, -offset),
    ]
}

/// Whether a navigation cell is walkable for an agent (center + inset cardinal samples).
pub fn is_cell_walkable(
    world: &WorldData,
    doodad_catalog: &DoodadCatalog,
    config: NavigationConfig,
    agent: NavigationAgent,
    coord: GridCoord,
) -> bool {
    let layout = world.layout();
    for global in cell_walkability_sample_globals(coord, config, agent.radius_meters) {
        let position = WorldPosition::from_global(global, layout);
        if !is_position_walkable(world, doodad_catalog, position, agent) {
            return false;
        }
    }
    true
}

/// Whether terrain heightfield is resident for this cell.
pub fn cell_terrain_available(
    world: &WorldData,
    coord: GridCoord,
    config: NavigationConfig,
) -> bool {
    let layout = world.layout();
    let global = grid_cell_center_global(coord, config);
    let position = WorldPosition::from_global(global, layout);
    let chunk = ChunkId::new(position.chunk);
    world.get(chunk).is_some() && ground_world_position(world, position).is_some()
}

/// Deterministic 8-neighbor offsets: N, NE, E, SE, S, SW, W, NW.
pub const NEIGHBOR_OFFSETS: [(i32, i32); 8] = [
    (0, 1),
    (1, 1),
    (1, 0),
    (1, -1),
    (0, -1),
    (-1, -1),
    (-1, 0),
    (-1, 1),
];

pub fn neighbor_step_cost(dx: i32, dz: i32, cell_spacing_meters: f32) -> f32 {
    let unit = if dx == 0 || dz == 0 {
        1.0
    } else {
        std::f32::consts::SQRT_2
    };
    unit * cell_spacing_meters
}

pub fn diagonal_corner_clear(
    world: &WorldData,
    doodad_catalog: &DoodadCatalog,
    config: NavigationConfig,
    agent: NavigationAgent,
    from: GridCoord,
    dx: i32,
    dz: i32,
) -> bool {
    if dx == 0 || dz == 0 {
        return true;
    }
    let cardinal_a = GridCoord::new(from.x + dx, from.z);
    let cardinal_b = GridCoord::new(from.x, from.z + dz);
    is_cell_walkable(world, doodad_catalog, config, agent, cardinal_a)
        && is_cell_walkable(world, doodad_catalog, config, agent, cardinal_b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{ChunkCoord, ChunkData, ChunkId, DoodadCatalog, Heightfield, LocalPosition, WorldData};

    fn layout() -> ChunkLayout {
        ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        }
    }

    fn agent() -> NavigationAgent {
        NavigationAgent {
            radius_meters: 0.6,
            max_slope_degrees: 40.0,
        }
    }

    #[test]
    fn grid_coord_snaps_to_cell() {
        let config = NavigationConfig::default();
        let pos = WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(5.0, 0.0, 9.0)),
        );
        let cell = grid_coord_at_position(pos, layout(), config);
        assert_eq!(cell, GridCoord::new(1, 2));
    }

    #[test]
    fn x_row_cells_walkable_on_flat_terrain() {
        let mut world = WorldData::new(layout());
        let heightfield = Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
        world.insert(
            ChunkId::new(ChunkCoord::new(0, 0)),
            ChunkData::new(heightfield, Vec::new()),
        );
        let catalog = DoodadCatalog::default();
        let config = NavigationConfig::default();
        for x in 0..=30 {
            let coord = GridCoord::new(x, 0);
            assert!(
                is_cell_walkable(&world, &catalog, config, agent(), coord),
                "cell {coord:?} not walkable"
            );
        }
    }
}
