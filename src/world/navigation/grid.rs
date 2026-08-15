//! Navigation grid coordinates and cell walkability adapters (ADR-032).

use bevy::prelude::*;

use super::legality::query_navigation_point_legality;
use crate::world::occupancy::{PassabilityAgent, PassabilityCatalogs, PassabilityResult};
use crate::world::{
    ChunkLayout, SpaceId, SpaceRegistry, WorldData, WorldPosition, ground_position_in_space,
    ground_world_position,
};

/// Grid coordinate in navigation cell space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GridCoord {
    pub x: i32,
    pub z: i32,
}

impl GridCoord {
    pub fn new(x: i32, z: i32) -> Self {
        Self { x, z }
    }
}

/// Agent parameters for navigation queries.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NavigationAgent {
    pub radius_meters: f32,
    pub max_slope_degrees: f32,
}

/// Navigation grid configuration.
#[derive(Debug, Clone, Copy, PartialEq, Resource, Reflect)]
pub struct NavigationConfig {
    pub cell_spacing_meters: f32,
    pub interior_cell_spacing_meters: f32,
}

impl Default for NavigationConfig {
    fn default() -> Self {
        Self {
            cell_spacing_meters: 4.0,
            interior_cell_spacing_meters: 0.5,
        }
    }
}

impl NavigationConfig {
    pub fn cell_spacing_for_space(&self, space_id: SpaceId) -> f32 {
        if space_id.is_surface() {
            self.cell_spacing_meters
        } else {
            self.interior_cell_spacing_meters
        }
    }

    pub fn config_for_space(&self, space_id: SpaceId) -> NavigationConfig {
        NavigationConfig {
            cell_spacing_meters: self.cell_spacing_for_space(space_id),
            interior_cell_spacing_meters: self.interior_cell_spacing_meters,
        }
    }
}

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
    let global = position.to_global(layout);
    grid_coord_at_global_xz(global, config)
}

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
    catalogs: PassabilityCatalogs<'_>,
    position: WorldPosition,
    agent: NavigationAgent,
) -> bool {
    let Some(grounded) = ground_world_position(world, position) else {
        return false;
    };
    matches!(
        query_navigation_point_legality(
            world,
            catalogs,
            grounded,
            PassabilityAgent::from(agent),
            SpaceId::SURFACE,
        ),
        PassabilityResult::Passable { .. }
    )
}

pub fn cell_walkability_sample_globals(
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

fn point_legal_in_space(
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
    matches!(
        query_navigation_point_legality(
            world,
            catalogs,
            grounded,
            PassabilityAgent::from(agent),
            space_id,
        ),
        PassabilityResult::Passable { .. }
    )
}

/// Whether a navigation cell is walkable for an agent (center + inset cardinal samples).
///
/// Grid search adapter: ALL inset samples must pass universal point legality.
pub fn is_cell_walkable(
    world: &WorldData,
    catalogs: PassabilityCatalogs<'_>,
    config: NavigationConfig,
    agent: NavigationAgent,
    coord: GridCoord,
) -> bool {
    let layout = world.layout();
    for global in cell_walkability_sample_globals(coord, config, agent.radius_meters) {
        let position = WorldPosition::from_global(global, layout);
        if !point_legal_in_space(
            world,
            world.space_registry(),
            catalogs,
            position,
            agent,
            SpaceId::SURFACE,
        ) {
            return false;
        }
    }
    true
}

/// Whether a navigation cell is walkable in a specific space (NV1.3).
///
/// Grid search adapter: ANY inset sample passing universal point legality marks the cell usable.
pub fn is_cell_walkable_in_space(
    world: &WorldData,
    space_registry: &SpaceRegistry,
    catalogs: PassabilityCatalogs<'_>,
    config: NavigationConfig,
    agent: NavigationAgent,
    coord: GridCoord,
    space_id: SpaceId,
) -> bool {
    let layout = world.layout();
    for global in cell_walkability_sample_globals(coord, config, agent.radius_meters) {
        let position = WorldPosition::from_global(global, layout);
        if point_legal_in_space(world, space_registry, catalogs, position, agent, space_id) {
            return true;
        }
    }
    false
}

/// Resolve a walkable navigation cell for a grounded endpoint.
pub fn resolve_path_endpoint_cell(
    world: &WorldData,
    space_registry: &SpaceRegistry,
    catalogs: PassabilityCatalogs<'_>,
    space_config: NavigationConfig,
    agent: NavigationAgent,
    space_id: SpaceId,
    position: WorldPosition,
    layout: ChunkLayout,
) -> Option<GridCoord> {
    let preferred = grid_coord_at_position(position, layout, space_config);
    if is_cell_walkable_in_space(
        world,
        space_registry,
        catalogs,
        space_config,
        agent,
        preferred,
        space_id,
    ) {
        return Some(preferred);
    }

    let mut queue = std::collections::VecDeque::from([preferred]);
    let mut seen = std::collections::BTreeSet::from([preferred]);
    let mut expanded = 0usize;
    while let Some(cell) = queue.pop_front() {
        expanded += 1;
        if expanded > 64 {
            break;
        }
        let mut neighbors: Vec<_> = NEIGHBOR_OFFSETS
            .iter()
            .map(|&(dx, dz)| GridCoord::new(cell.x + dx, cell.z + dz))
            .collect();
        neighbors.sort_by_key(|coord| (coord.z, coord.x));
        for next in neighbors {
            if !seen.insert(next) {
                continue;
            }
            if is_cell_walkable_in_space(
                world,
                space_registry,
                catalogs,
                space_config,
                agent,
                next,
                space_id,
            ) {
                return Some(next);
            }
            queue.push_back(next);
        }
    }
    Some(preferred)
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
    ground_world_position(world, position).is_some()
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

/// Grid-search diagonal corner clearance: both cardinal neighbor cells must be usable.
pub fn diagonal_corner_clear(
    world: &WorldData,
    catalogs: PassabilityCatalogs<'_>,
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
    is_cell_walkable(world, catalogs, config, agent, cardinal_a)
        && is_cell_walkable(world, catalogs, config, agent, cardinal_b)
}

/// Diagonal corner clearance within a navigation space (IN-11gG).
pub fn diagonal_corner_clear_in_space(
    world: &WorldData,
    space_registry: &SpaceRegistry,
    catalogs: PassabilityCatalogs<'_>,
    config: NavigationConfig,
    agent: NavigationAgent,
    from: GridCoord,
    dx: i32,
    dz: i32,
    space_id: SpaceId,
) -> bool {
    if dx == 0 || dz == 0 {
        return true;
    }
    let cardinal_a = GridCoord::new(from.x + dx, from.z);
    let cardinal_b = GridCoord::new(from.x, from.z + dz);
    is_cell_walkable_in_space(
        world,
        space_registry,
        catalogs,
        config,
        agent,
        cardinal_a,
        space_id,
    ) && is_cell_walkable_in_space(
        world,
        space_registry,
        catalogs,
        config,
        agent,
        cardinal_b,
        space_id,
    )
}

/// Whether universal segment legality permits a grid neighbor transition.
pub fn grid_neighbor_transition_legal_in_space(
    world: &WorldData,
    space_registry: &SpaceRegistry,
    catalogs: PassabilityCatalogs<'_>,
    config: NavigationConfig,
    agent: NavigationAgent,
    from: GridCoord,
    to: GridCoord,
    space_id: SpaceId,
    layout: ChunkLayout,
) -> bool {
    use super::legality::query_navigation_segment_legality;
    let space_config = config.config_for_space(space_id);
    let from_pos = WorldPosition::from_global(grid_cell_center_global(from, space_config), layout);
    let to_pos = WorldPosition::from_global(grid_cell_center_global(to, space_config), layout);
    let Some(from_grounded) = ground_position_in_space(world, space_registry, space_id, from_pos)
    else {
        return false;
    };
    let Some(to_grounded) = ground_position_in_space(world, space_registry, space_id, to_pos)
    else {
        return false;
    };
    query_navigation_segment_legality(
        world,
        space_registry,
        catalogs,
        config,
        space_id,
        agent,
        from_grounded,
        to_grounded,
        layout,
    )
    .is_legal()
}

/// Whether a grounded position is walkable in a specific navigation space.
pub fn is_position_walkable_in_space(
    world: &WorldData,
    space_registry: &SpaceRegistry,
    catalogs: PassabilityCatalogs<'_>,
    position: WorldPosition,
    agent: NavigationAgent,
    space_id: SpaceId,
) -> bool {
    point_legal_in_space(world, space_registry, catalogs, position, agent, space_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        BuildingCatalog, ChunkCoord, ChunkData, ChunkId, DoodadCatalog, FootprintCatalog,
        Heightfield, LocalPosition, WorldData,
    };

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
    fn default_surface_spacing_is_four_meters() {
        let config = NavigationConfig::default();
        assert_eq!(config.cell_spacing_meters, 4.0);
    }

    #[test]
    fn default_interior_spacing_is_half_meter() {
        let config = NavigationConfig::default();
        assert_eq!(config.interior_cell_spacing_meters, 0.5);
    }

    #[test]
    fn effective_spacing_selects_by_space() {
        let config = NavigationConfig::default();
        assert_eq!(
            config.cell_spacing_for_space(crate::world::SpaceId::SURFACE),
            4.0
        );
        assert_eq!(
            config.cell_spacing_for_space(crate::world::SpaceId::new(1)),
            0.5
        );
        let interior_config = config.config_for_space(crate::world::SpaceId::new(1));
        assert_eq!(interior_config.cell_spacing_meters, 0.5);
        assert_eq!(
            config
                .config_for_space(crate::world::SpaceId::SURFACE)
                .cell_spacing_meters,
            4.0
        );
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
        let building = BuildingCatalog::default();
        let footprint = FootprintCatalog::default();
        let pass = PassabilityCatalogs {
            doodad: &catalog,
            building: &building,
            footprint: &footprint,
        };
        let config = NavigationConfig::default();
        for x in 0..=30 {
            let coord = GridCoord::new(x, 0);
            assert!(
                is_cell_walkable(&world, pass, config, agent(), coord),
                "cell {coord:?} not walkable"
            );
        }
    }
}
