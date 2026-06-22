//! Navigation path requests (ADR-032).

use bevy::prelude::*;

use super::astar::astar_path;
use super::grid::{
    grid_cell_world_position, grid_coord_at_position, cell_terrain_available, NavigationConfig,
};
use super::path::NavigationPath;
use crate::world::{
    ground_world_position, is_position_blocked_by_doodads, DoodadCatalog, WorldData, WorldPosition,
};

/// Why [`find_path`] failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationError {
    StartBlocked,
    GoalBlocked,
    NoPath,
    TerrainUnavailable,
}

/// Request a grounded navigation path between two authoritative positions.
pub fn find_path(
    world: &WorldData,
    doodad_catalog: &DoodadCatalog,
    config: &NavigationConfig,
    agent_radius_meters: f32,
    start: WorldPosition,
    goal: WorldPosition,
) -> Result<NavigationPath, NavigationError> {
    let layout = world.layout();
    let start_cell = grid_coord_at_position(start, layout, *config);
    let goal_cell = grid_coord_at_position(goal, layout, *config);

    if !cell_terrain_available(world, start_cell, *config)
        || ground_world_position(world, start).is_none()
    {
        return Err(NavigationError::TerrainUnavailable);
    }
    if !cell_terrain_available(world, goal_cell, *config)
        || ground_world_position(world, goal).is_none()
    {
        return Err(NavigationError::TerrainUnavailable);
    }

    if is_position_blocked_by_doodads(world, doodad_catalog, start, agent_radius_meters) {
        return Err(NavigationError::StartBlocked);
    }
    if is_position_blocked_by_doodads(world, doodad_catalog, goal, agent_radius_meters) {
        return Err(NavigationError::GoalBlocked);
    }

    let mut waypoints = astar_path(
        world,
        doodad_catalog,
        *config,
        agent_radius_meters,
        start_cell,
        goal_cell,
    )
    .ok_or(NavigationError::NoPath)?;

    if waypoints.is_empty() {
        if let Some(goal_pos) = grid_cell_world_position(world, goal_cell, *config) {
            waypoints.push(goal_pos);
        } else {
            return Err(NavigationError::NoPath);
        }
    }

    trim_waypoints_at_start(&mut waypoints, start, layout);

    Ok(NavigationPath::new(waypoints))
}

fn trim_waypoints_at_start(
    waypoints: &mut Vec<WorldPosition>,
    start: WorldPosition,
    layout: crate::world::ChunkLayout,
) {
    const EPSILON: f32 = 0.25;
    while let Some(first) = waypoints.first().copied() {
        let a = start.to_global(layout);
        let b = first.to_global(layout);
        let dx = a.x - b.x;
        let dz = a.z - b.z;
        if dx * dx + dz * dz <= EPSILON * EPSILON {
            waypoints.remove(0);
        } else {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        create_doodad, ChunkCoord, ChunkData, ChunkId, ChunkLayout, DoodadCatalog,
        DoodadDefinitionId, DoodadPlacementOverrides, DoodadSource, Heightfield, LocalPosition,
    };

    fn layout() -> ChunkLayout {
        ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        }
    }

    fn pos(x: f32, z: f32) -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(x, 0.0, z)),
        )
    }

    fn pos_chunk(cx: i32, cz: i32, x: f32, z: f32) -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(cx, cz),
            LocalPosition::new(Vec3::new(x, 0.0, z)),
        )
    }

    fn insert_flat(world: &mut WorldData, x: i32, z: i32) {
        let heightfield = Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
        world.insert(
            ChunkId::new(ChunkCoord::new(x, z)),
            ChunkData::new(heightfield, Vec::new()),
        );
    }

    fn nav_config() -> NavigationConfig {
        NavigationConfig::default()
    }

    #[test]
    fn straight_path_on_open_terrain() {
        let mut world = WorldData::new(layout());
        insert_flat(&mut world, 0, 0);
        let catalog = DoodadCatalog::default();
        let path = find_path(
            &world,
            &catalog,
            &nav_config(),
            0.5,
            pos(4.0, 4.0),
            pos(40.0, 4.0),
        )
        .unwrap();
        assert!(path.len() >= 2);
        let last = path.waypoints.last().unwrap();
        assert!((last.to_global(layout()).x - 40.0).abs() < 4.5);
    }

    #[test]
    fn obstacle_detour() {
        let mut world = WorldData::new(layout());
        insert_flat(&mut world, 0, 0);
        let catalog = DoodadCatalog::default();
        for z in 0..16 {
            create_doodad(
                &catalog,
                &mut world,
                &DoodadDefinitionId::new("tree_oak"),
                pos(20.0, z as f32 * 4.0),
                DoodadSource::Authored,
                DoodadPlacementOverrides::default(),
            )
            .unwrap();
        }
        let path = find_path(
            &world,
            &catalog,
            &nav_config(),
            0.5,
            pos(4.0, 28.0),
            pos(36.0, 28.0),
        )
        .unwrap();
        assert!(path.len() >= 3);
        let globals: Vec<_> = path
            .waypoints
            .iter()
            .map(|p| p.to_global(layout()).x)
            .collect();
        assert!(globals.iter().any(|&x| x < 18.0 || x > 22.0));
    }

    #[test]
    fn blocked_goal() {
        let mut world = WorldData::new(layout());
        insert_flat(&mut world, 0, 0);
        let catalog = DoodadCatalog::default();
        create_doodad(
            &catalog,
            &mut world,
            &DoodadDefinitionId::new("tree_oak"),
            pos(40.0, 40.0),
            DoodadSource::Authored,
            DoodadPlacementOverrides::default(),
        )
        .unwrap();
        let err = find_path(
            &world,
            &catalog,
            &nav_config(),
            0.5,
            pos(4.0, 4.0),
            pos(40.0, 40.0),
        )
        .unwrap_err();
        assert_eq!(err, NavigationError::GoalBlocked);
    }

    #[test]
    fn blocked_start() {
        let mut world = WorldData::new(layout());
        insert_flat(&mut world, 0, 0);
        let catalog = DoodadCatalog::default();
        create_doodad(
            &catalog,
            &mut world,
            &DoodadDefinitionId::new("tree_oak"),
            pos(4.0, 4.0),
            DoodadSource::Authored,
            DoodadPlacementOverrides::default(),
        )
        .unwrap();
        let err = find_path(
            &world,
            &catalog,
            &nav_config(),
            0.5,
            pos(4.0, 4.0),
            pos(40.0, 40.0),
        )
        .unwrap_err();
        assert_eq!(err, NavigationError::StartBlocked);
    }

    #[test]
    fn path_from_origin_to_far_x() {
        let mut world = WorldData::new(layout());
        insert_flat(&mut world, 0, 0);
        let catalog = DoodadCatalog::default();
        let path = find_path(
            &world,
            &catalog,
            &nav_config(),
            0.6,
            pos(0.0, 0.0),
            pos(100.0, 0.0),
        )
        .unwrap();
        assert!(!path.is_empty());
    }

    #[test]
    fn no_path_when_walled() {
        let mut world = WorldData::new(layout());
        insert_flat(&mut world, 0, 0);
        let catalog = DoodadCatalog::default();
        for x in 0..64 {
            create_doodad(
                &catalog,
                &mut world,
                &DoodadDefinitionId::new("tree_oak"),
                pos(x as f32 * 4.0, 28.0),
                DoodadSource::Authored,
                DoodadPlacementOverrides::default(),
            )
            .unwrap();
        }
        let err = find_path(
            &world,
            &catalog,
            &nav_config(),
            0.5,
            pos(4.0, 4.0),
            pos(60.0, 60.0),
        )
        .unwrap_err();
        assert_eq!(err, NavigationError::NoPath);
    }

    #[test]
    fn cross_chunk_path() {
        let mut world = WorldData::new(layout());
        insert_flat(&mut world, 0, 0);
        insert_flat(&mut world, 1, 0);
        let catalog = DoodadCatalog::default();
        let path = find_path(
            &world,
            &catalog,
            &nav_config(),
            0.5,
            pos_chunk(0, 0, 250.0, 128.0),
            pos_chunk(1, 0, 10.0, 128.0),
        )
        .unwrap();
        assert!(path.len() >= 2);
        assert_eq!(path.waypoints.last().unwrap().chunk.x, 1);
    }

    #[test]
    fn deterministic_repeated_searches() {
        let mut world = WorldData::new(layout());
        insert_flat(&mut world, 0, 0);
        let catalog = DoodadCatalog::default();
        create_doodad(
            &catalog,
            &mut world,
            &DoodadDefinitionId::new("tree_oak"),
            pos(20.0, 20.0),
            DoodadSource::Authored,
            DoodadPlacementOverrides::default(),
        )
        .unwrap();
        let a = find_path(
            &world,
            &catalog,
            &nav_config(),
            0.5,
            pos(4.0, 4.0),
            pos(36.0, 36.0),
        )
        .unwrap();
        let b = find_path(
            &world,
            &catalog,
            &nav_config(),
            0.5,
            pos(4.0, 4.0),
            pos(36.0, 36.0),
        )
        .unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn missing_terrain_returns_error() {
        let world = WorldData::new(layout());
        let catalog = DoodadCatalog::default();
        let err = find_path(
            &world,
            &catalog,
            &nav_config(),
            0.5,
            pos(4.0, 4.0),
            pos(40.0, 40.0),
        )
        .unwrap_err();
        assert_eq!(err, NavigationError::TerrainUnavailable);
    }
}
