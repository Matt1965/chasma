//! IN-03 interior grid resolution and space-safe path simplification tests.

use bevy::prelude::*;

use super::astar::astar_path_in_space_with_stats;
use super::grid::{
    GridCoord, NavigationAgent, NavigationConfig, grid_coord_at_position, is_cell_walkable_in_space,
};
use super::simplify::is_segment_walkable_in_space;
use crate::world::{
    BuildingCatalog, BuildingId, BuildingNavigationBlueprintId, BuildingNavigationRuntime,
    ChunkCoord, ChunkId, ChunkLayout, DoodadCatalog, FootprintCatalog, LocalPosition,
    OCCUPANCY_CELL_SIZE_METERS, OccupancyCellCoord, OccupancyCellEntry, OccupancySource,
    OccupancyState, PassabilityCatalogs, RuntimeNavigationFloor, RuntimeNavigationRegion, SpaceId,
    SpaceRecord, WorldData, WorldPosition, chunk_for_occupancy_cell, find_path_with_spaces,
    ground_position_in_space,
};

struct TestCatalogs {
    doodad: DoodadCatalog,
    building: BuildingCatalog,
    footprint: FootprintCatalog,
}

impl TestCatalogs {
    fn new() -> Self {
        Self {
            doodad: DoodadCatalog::default(),
            building: BuildingCatalog::default(),
            footprint: FootprintCatalog::default(),
        }
    }

    fn pass(&self) -> PassabilityCatalogs<'_> {
        PassabilityCatalogs {
            doodad: &self.doodad,
            building: &self.building,
            footprint: &self.footprint,
        }
    }
}

const FLOOR_Y: f32 = 2.0;
const AGENT_RADIUS: f32 = 0.5;
const MAX_SLOPE: f32 = 45.0;

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

fn agent() -> NavigationAgent {
    NavigationAgent {
        radius_meters: AGENT_RADIUS,
        max_slope_degrees: MAX_SLOPE,
    }
}

fn pass_catalogs() -> TestCatalogs {
    TestCatalogs::new()
}

fn register_interior_floor(world: &mut WorldData, outline: Vec<Vec2>, floor_key: &str) -> SpaceId {
    let space_id = world.space_registry_mut().allocate_space_id();
    world.space_registry_mut().insert_space(SpaceRecord {
        id: space_id,
        owning_building_id: Some(BuildingId::new(1)),
        display_floor_label: floor_key.to_string(),
        visibility_group_id: 1,
        reference_elevation: FLOOR_Y,
        floor_y_global: FLOOR_Y,
        room_tag: None,
        enabled: true,
        walkable: true,
    });

    let runtime = world
        .building_navigation_runtime()
        .get(BuildingId::new(1))
        .cloned()
        .unwrap_or_else(|| BuildingNavigationRuntime {
            building_id: BuildingId::new(1),
            blueprint_id: BuildingNavigationBlueprintId::new("in03_test"),
            model_transform: Transform::IDENTITY,
            space_keys: Default::default(),
            portal_keys: Default::default(),
            floors: Vec::new(),
            regions: Vec::new(),
        });

    let (min, max) = {
        let mut min = Vec2::splat(f32::INFINITY);
        let mut max = Vec2::splat(f32::NEG_INFINITY);
        for point in &outline {
            min = min.min(*point);
            max = max.max(*point);
        }
        (min, max)
    };
    let floor_id = runtime.floors.len() as i32;
    let mut floors = runtime.floors;
    let mut regions = runtime.regions;
    regions.push(RuntimeNavigationRegion {
        space_id,
        floor_id,
        floor_key: floor_key.to_string(),
        region_key: "main".to_string(),
        display_label: floor_key.to_string(),
        elevation_meters: FLOOR_Y,
        world_outline_xz: outline,
        world_aabb_min_xz: min,
        world_aabb_max_xz: max,
    });
    floors.push(RuntimeNavigationFloor {
        floor_id,
        floor_key: floor_key.to_string(),
        elevation_meters: FLOOR_Y,
        visibility_group_id: 1,
        region_space_ids: vec![space_id],
    });
    world
        .building_navigation_runtime_mut()
        .insert(BuildingNavigationRuntime {
            building_id: BuildingId::new(1),
            blueprint_id: BuildingNavigationBlueprintId::new("in03_test"),
            model_transform: Transform::IDENTITY,
            space_keys: runtime.space_keys,
            portal_keys: runtime.portal_keys,
            floors,
            regions,
        });
    space_id
}

fn block_interior_occupancy(
    world: &mut WorldData,
    space_id: SpaceId,
    global_x: f32,
    global_z: f32,
) {
    let layout = world.layout();
    let cell = OccupancyCellCoord::new(
        (global_x / OCCUPANCY_CELL_SIZE_METERS).floor() as i32,
        (global_z / OCCUPANCY_CELL_SIZE_METERS).floor() as i32,
    );
    let chunk = ChunkId::new(chunk_for_occupancy_cell(cell, layout));
    world.insert_occupancy_cell(
        chunk,
        cell,
        OccupancyCellEntry {
            state: OccupancyState::Blocked,
            source: OccupancySource::Building(BuildingId::new(1)),
            space_id: space_id.raw(),
        },
    );
}

fn l_shaped_outline() -> Vec<Vec2> {
    vec![
        Vec2::new(20.0, 20.0),
        Vec2::new(26.0, 20.0),
        Vec2::new(26.0, 22.0),
        Vec2::new(22.0, 22.0),
        Vec2::new(22.0, 26.0),
        Vec2::new(20.0, 26.0),
    ]
}

fn rectangular_outline(min_x: f32, min_z: f32, max_x: f32, max_z: f32) -> Vec<Vec2> {
    vec![
        Vec2::new(min_x, min_z),
        Vec2::new(max_x, min_z),
        Vec2::new(max_x, max_z),
        Vec2::new(min_x, max_z),
    ]
}

fn assert_all_segments_walkable(
    world: &WorldData,
    space_id: SpaceId,
    catalogs: &TestCatalogs,
    config: NavigationConfig,
    positions: &[WorldPosition],
) {
    let layout = world.layout();
    let pass = catalogs.pass();
    for pair in positions.windows(2) {
        assert!(
            is_segment_walkable_in_space(
                world,
                world.space_registry(),
                pass,
                config,
                space_id,
                agent(),
                pair[0],
                pair[1],
                layout,
            ),
            "segment {:?} -> {:?} is not walkable in space {:?}",
            pair[0],
            pair[1],
            space_id,
        );
    }
}

#[test]
fn interior_astar_reconstructs_without_surface_terrain() {
    let mut world = WorldData::new(layout());
    let space_id = register_interior_floor(
        &mut world,
        rectangular_outline(30.0, 30.0, 34.0, 34.0),
        "small_room",
    );
    let catalogs = pass_catalogs();
    let config = NavigationConfig::default();
    let start = pos(31.0, 31.0);
    let goal = pos(33.0, 33.0);
    let start_cell = grid_coord_at_position(
        ground_position_in_space(&world, world.space_registry(), space_id, start).unwrap(),
        layout(),
        config.config_for_space(space_id),
    );
    let goal_cell = grid_coord_at_position(
        ground_position_in_space(&world, world.space_registry(), space_id, goal).unwrap(),
        layout(),
        config.config_for_space(space_id),
    );
    let path = super::astar::astar_path_in_space(
        &world,
        world.space_registry(),
        catalogs.pass(),
        config,
        agent(),
        start_cell,
        goal_cell,
        space_id,
    )
    .expect("interior astar without terrain");
    assert!(path.len() >= 2);
    for waypoint in &path {
        let global = waypoint.to_global(layout());
        assert!(
            (global.y - FLOOR_Y).abs() < 0.01,
            "interior waypoint should use floor elevation, got y={}",
            global.y
        );
    }
}

#[test]
fn small_interior_room_produces_valid_path_at_half_meter_spacing() {
    let mut world = WorldData::new(layout());
    let space_id = register_interior_floor(
        &mut world,
        rectangular_outline(30.0, 30.0, 34.0, 34.0),
        "small_room",
    );
    let catalogs = pass_catalogs();
    let config = NavigationConfig::default();
    let space_config = config.config_for_space(space_id);
    let start = pos(31.0, 31.0);
    let goal = pos(33.0, 33.0);

    let walkable_cells = {
        let min_x = (30.0 / space_config.cell_spacing_meters).floor() as i32;
        let max_x = (34.0 / space_config.cell_spacing_meters).floor() as i32;
        let min_z = min_x;
        let max_z = max_x;
        (min_x..=max_x)
            .flat_map(|x| (min_z..=max_z).map(move |z| GridCoord::new(x, z)))
            .filter(|cell| {
                is_cell_walkable_in_space(
                    &world,
                    world.space_registry(),
                    catalogs.pass(),
                    space_config,
                    agent(),
                    *cell,
                    space_id,
                )
            })
            .count()
    };
    assert!(
        walkable_cells > 1,
        "4x4 interior should expose multiple interior cells, got {walkable_cells}"
    );

    let path = find_path_with_spaces(
        &world,
        catalogs.pass(),
        &config,
        AGENT_RADIUS,
        MAX_SLOPE,
        start,
        goal,
        space_id,
        space_id,
        None,
    )
    .expect("small room path");
    assert!(path.len() >= 2);
    let last = path.waypoints.last().unwrap().position;
    let goal_global = goal.to_global(layout());
    let last_global = last.to_global(layout());
    assert!((last_global.x - goal_global.x).abs() < 0.05);
    assert!((last_global.z - goal_global.z).abs() < 0.05);

    let positions: Vec<_> = path.waypoints.iter().map(|wp| wp.position).collect();
    assert_all_segments_walkable(&world, space_id, &catalogs, config, &positions);

    let a = find_path_with_spaces(
        &world,
        catalogs.pass(),
        &config,
        AGENT_RADIUS,
        MAX_SLOPE,
        start,
        goal,
        space_id,
        space_id,
        None,
    )
    .unwrap();
    let b = find_path_with_spaces(
        &world,
        catalogs.pass(),
        &config,
        AGENT_RADIUS,
        MAX_SLOPE,
        start,
        goal,
        space_id,
        space_id,
        None,
    )
    .unwrap();
    assert_eq!(a, b, "interior planning should be deterministic");
}

#[test]
fn concave_l_floor_rejects_direct_shortcut_and_preserves_detour() {
    let mut world = WorldData::new(layout());
    let space_id = register_interior_floor(&mut world, l_shaped_outline(), "l_floor");
    let catalogs = pass_catalogs();
    let config = NavigationConfig::default();
    let layout = layout();

    let start = pos(25.0, 20.5);
    let goal = pos(20.5, 25.0);

    assert!(
        crate::world::interior_position_walkable(
            world.building_navigation_runtime(),
            world.space_registry(),
            layout,
            start,
            space_id,
        ),
        "start should be inside L floor"
    );
    assert!(
        crate::world::interior_position_walkable(
            world.building_navigation_runtime(),
            world.space_registry(),
            layout,
            goal,
            space_id,
        ),
        "goal should be inside L floor"
    );
    assert!(
        !is_segment_walkable_in_space(
            &world,
            world.space_registry(),
            catalogs.pass(),
            config,
            space_id,
            agent(),
            start,
            goal,
            layout,
        ),
        "direct segment should leave the concave polygon"
    );

    let space_config = config.config_for_space(space_id);
    let start_cell = grid_coord_at_position(
        ground_position_in_space(&world, world.space_registry(), space_id, start).unwrap(),
        layout,
        space_config,
    );
    let goal_cell = grid_coord_at_position(
        ground_position_in_space(&world, world.space_registry(), space_id, goal).unwrap(),
        layout,
        space_config,
    );
    let (unsimplified, expanded) = astar_path_in_space_with_stats(
        &world,
        world.space_registry(),
        catalogs.pass(),
        config,
        agent(),
        start_cell,
        goal_cell,
        space_id,
    )
    .expect("astar should find L detour");
    assert!(expanded < super::astar::MAX_ASTAR_SEARCH_NODES);
    assert!(
        unsimplified.len() >= 3,
        "L detour should need multiple cells"
    );

    let path = find_path_with_spaces(
        &world,
        catalogs.pass(),
        &config,
        AGENT_RADIUS,
        MAX_SLOPE,
        start,
        goal,
        space_id,
        space_id,
        None,
    )
    .expect("simplified interior path");
    let positions: Vec<_> = path.waypoints.iter().map(|wp| wp.position).collect();
    assert!(
        !is_segment_walkable_in_space(
            &world,
            world.space_registry(),
            catalogs.pass(),
            config,
            space_id,
            agent(),
            start,
            goal,
            layout,
        ),
        "simplifier must not replace path with invalid direct shortcut"
    );
    assert_all_segments_walkable(&world, space_id, &catalogs, config, &positions);
    assert!(
        path.len() >= 2,
        "simplified path should retain a detour, got {} waypoints",
        path.len()
    );
    assert!(
        positions.len() <= unsimplified.len(),
        "simplification should not increase waypoint count"
    );

    let globals: Vec<Vec2> = positions
        .iter()
        .map(|p| {
            let g = p.to_global(layout);
            Vec2::new(g.x, g.z)
        })
        .collect();
    let corner = Vec2::new(22.0, 22.0);
    assert!(
        globals.iter().any(|p| p.distance(corner) < 1.5),
        "path should route near the inner corner {:?}, got {:?}",
        corner,
        globals
    );
}

#[test]
fn interior_diagonal_step_requires_clear_cardinal_neighbors() {
    let mut world = WorldData::new(layout());
    let space_id = register_interior_floor(
        &mut world,
        rectangular_outline(40.0, 40.0, 52.0, 52.0),
        "corner_room",
    );
    block_interior_occupancy(&mut world, space_id, 43.0, 42.0);
    block_interior_occupancy(&mut world, space_id, 42.0, 43.0);

    let catalogs = pass_catalogs();
    let config = NavigationConfig::default();
    let start = pos(41.0, 41.0);
    let goal = pos(44.0, 44.0);
    let path = find_path_with_spaces(
        &world,
        catalogs.pass(),
        &config,
        AGENT_RADIUS,
        MAX_SLOPE,
        start,
        goal,
        space_id,
        space_id,
        None,
    )
    .expect("path around blocked diagonal corner");
    let positions: Vec<_> = path.waypoints.iter().map(|wp| wp.position).collect();
    assert_all_segments_walkable(&world, space_id, &catalogs, config, &positions);
    assert!(
        path.len() >= 3,
        "path should detour when diagonal corner is blocked"
    );
    assert!(
        !is_segment_walkable_in_space(
            &world,
            world.space_registry(),
            catalogs.pass(),
            config,
            space_id,
            agent(),
            start,
            goal,
            layout(),
        ),
        "direct diagonal shortcut should be invalid"
    );
}

#[test]
fn interior_search_node_budget_for_fixtures() {
    let mut world = WorldData::new(layout());
    let small_space = register_interior_floor(
        &mut world,
        rectangular_outline(30.0, 30.0, 34.0, 34.0),
        "small_room",
    );
    let l_space = register_interior_floor(&mut world, l_shaped_outline(), "l_floor");
    let catalogs = pass_catalogs();
    let config = NavigationConfig::default();
    let layout = layout();

    let small_start = pos(31.0, 31.0);
    let small_goal = pos(33.0, 33.0);
    let small_config = config.config_for_space(small_space);
    let (_, small_expanded) = astar_path_in_space_with_stats(
        &world,
        world.space_registry(),
        catalogs.pass(),
        config,
        agent(),
        grid_coord_at_position(
            ground_position_in_space(&world, world.space_registry(), small_space, small_start)
                .unwrap(),
            layout,
            small_config,
        ),
        grid_coord_at_position(
            ground_position_in_space(&world, world.space_registry(), small_space, small_goal)
                .unwrap(),
            layout,
            small_config,
        ),
        small_space,
    )
    .expect("small room astar");

    let l_start = pos(25.0, 20.5);
    let l_goal = pos(20.5, 25.0);
    let l_config = config.config_for_space(l_space);
    let (_, l_expanded) = astar_path_in_space_with_stats(
        &world,
        world.space_registry(),
        catalogs.pass(),
        config,
        agent(),
        grid_coord_at_position(
            ground_position_in_space(&world, world.space_registry(), l_space, l_start).unwrap(),
            layout,
            l_config,
        ),
        grid_coord_at_position(
            ground_position_in_space(&world, world.space_registry(), l_space, l_goal).unwrap(),
            layout,
            l_config,
        ),
        l_space,
    )
    .expect("L floor astar");

    assert!(small_expanded < super::astar::MAX_ASTAR_SEARCH_NODES);
    assert!(l_expanded < super::astar::MAX_ASTAR_SEARCH_NODES);
    assert!(small_expanded > 0);
    assert!(l_expanded > 0);
}
