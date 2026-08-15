//! IN-11gG universal legality consumer migration regression tests.

use bevy::prelude::*;

use super::astar::{astar_path, astar_path_in_space};
use super::grid::{
    GridCoord, NavigationAgent, NavigationConfig, grid_coord_at_position,
    grid_neighbor_transition_legal_in_space,
};
use super::legality::{
    NavigationSegmentBlockReason, NavigationSegmentLegality, query_navigation_point_legality,
    query_navigation_segment_legality,
};
use super::simplify::{
    all_consecutive_segments_legal_in_space, has_walkable_line_of_sight_surface,
    simplify_navigation_path_in_space,
};
use crate::units::input::{SelectedUnits, issue_move_orders_to_selection};
use crate::world::unit::{
    UnitDefinitionId, UnitSource, UnitState, create_unit_with_ownership, step_unit_movement,
};
use crate::world::{
    Affiliation, AttackTargetingPolicy, BuildingCatalog, BuildingDefinitionId,
    BuildingLifecycleState, BuildingNavigationBlueprint, BuildingNavigationBlueprintCatalog,
    BuildingNavigationBlueprintInstanceOverride, BuildingOwnership, BuildingSource, ChunkCoord,
    ChunkData, ChunkId, ChunkLayout, DoodadCatalog, DoodadDefinitionId, DoodadPlacementOverrides,
    DoodadSource, FootprintCatalog, Heightfield, InteriorProfileCatalog, LocalPosition,
    NavigationEntranceDefinition, NavigationFloorDefinition, NavigationPath, NavigationPolygon2d,
    NavigationRegionDefinition, NavigationWaypoint, OccupancyCatalogs, PassabilityAgent,
    PassabilityBlockReason, PassabilityCatalogs, PassabilityResult, SpaceId, UnitOwnership,
    WeaponCatalog, WorldData, WorldPosition, create_building, create_doodad, find_path,
    ground_position_in_space, place_player_building, resolve_pending_unit_orders,
    set_building_lifecycle_stage,
};

const AGENT_RADIUS: f32 = 0.6;
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

fn flat_world() -> WorldData {
    let mut world = WorldData::new(layout());
    let heightfield = Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
    world.insert(
        ChunkId::new(ChunkCoord::new(0, 0)),
        ChunkData::new(heightfield, Vec::new()),
    );
    world
}

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

fn assert_consecutive_waypoints_legal(
    world: &WorldData,
    space_id: SpaceId,
    catalogs: &TestCatalogs,
    config: NavigationConfig,
    positions: &[WorldPosition],
) {
    let layout = world.layout();
    assert!(
        all_consecutive_segments_legal_in_space(
            world,
            world.space_registry(),
            catalogs.pass(),
            config,
            space_id,
            agent(),
            positions,
            layout,
        ),
        "planner/simplifier produced an illegally simplified segment in space {}",
        space_id.raw()
    );
}

fn oversized_concave_hut_blueprint() -> BuildingNavigationBlueprint {
    BuildingNavigationBlueprint::new("oversized_concave_hut", "Oversized Concave Hut")
        .with_floors(vec![NavigationFloorDefinition {
            floor_id: 0,
            key: "ground".to_string(),
            display_label: "Ground".to_string(),
            elevation_meters: 1.27,
            visibility_group_id: 1,
            room_tag: None,
            walkable_outline_legacy: None,
            regions: vec![NavigationRegionDefinition {
                key: "main".to_string(),
                display_label: "Main".to_string(),
                room_tag: None,
                walkable_outline: NavigationPolygon2d {
                    vertices_xz: vec![
                        [0.0, 0.0],
                        [14.0, 0.0],
                        [14.0, 14.0],
                        [6.0, 14.0],
                        [6.0, 6.0],
                        [0.0, 6.0],
                    ],
                },
            }],
        }])
        .with_entrances(vec![NavigationEntranceDefinition {
            key: "exterior_entrance".to_string(),
            floor_key: "ground".to_string(),
            region_key: Some("main".to_string()),
            local_position_xz: [7.0, 0.0],
            radius_meters: 1.5,
            interior_spawn_local: [7.0, 1.27, 1.5],
            bidirectional: true,
            door_key: None,
        }])
}

fn activate_fixture(
    world: &mut WorldData,
    blueprint: BuildingNavigationBlueprint,
    placement: WorldPosition,
) -> crate::world::BuildingId {
    let building_catalog = BuildingCatalog::default();
    let nav_catalog = BuildingNavigationBlueprintCatalog::default();
    let doodad_catalog = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let occupancy = OccupancyCatalogs {
        building: &building_catalog,
        doodad: &doodad_catalog,
        footprint: &footprint,
    };
    let interior = InteriorProfileCatalog::default();
    let id = place_player_building(
        &building_catalog,
        world,
        &BuildingDefinitionId::new("hut"),
        placement,
        Quat::IDENTITY,
        BuildingOwnership::with_affiliation(Affiliation::Player),
        occupancy,
    )
    .unwrap()
    .id;
    world
        .mutate_building(id, |record| {
            record.interior.navigation_blueprint_override = Some(
                BuildingNavigationBlueprintInstanceOverride::inline(blueprint),
            );
        })
        .expect("building");
    set_building_lifecycle_stage(
        world,
        &building_catalog,
        &interior,
        &doodad_catalog,
        occupancy,
        Some(&nav_catalog),
        id,
        BuildingLifecycleState::Complete,
        1.0,
    )
    .unwrap();
    id
}

fn local_xz_to_world(
    world: &WorldData,
    building_id: crate::world::BuildingId,
    local_xz: Vec2,
    floor_y: f32,
) -> WorldPosition {
    let runtime = world
        .building_navigation_runtime()
        .get(building_id)
        .expect("runtime");
    let layout = world.layout();
    let global = runtime
        .model_transform
        .transform_point(Vec3::new(local_xz.x, floor_y, local_xz.y));
    WorldPosition::from_global(global, layout)
}

fn region_space(
    world: &WorldData,
    building_id: crate::world::BuildingId,
    floor_key: &str,
    region_key: &str,
) -> SpaceId {
    let runtime = world
        .building_navigation_runtime()
        .get(building_id)
        .expect("runtime");
    let key = format!("{floor_key}/{region_key}");
    *runtime.space_keys.get(&key).unwrap_or_else(|| {
        panic!("missing space key `{key}`");
    })
}

fn one_region_test_blueprint() -> BuildingNavigationBlueprint {
    BuildingNavigationBlueprint::new("one_region_hut", "One Region Hut")
        .with_floors(vec![NavigationFloorDefinition {
            floor_id: 0,
            key: "ground".to_string(),
            display_label: "Ground".to_string(),
            elevation_meters: 0.0,
            visibility_group_id: 1,
            room_tag: None,
            walkable_outline_legacy: None,
            regions: vec![NavigationRegionDefinition {
                key: "main".to_string(),
                display_label: "Main".to_string(),
                room_tag: None,
                walkable_outline: NavigationPolygon2d {
                    vertices_xz: vec![[0.0, 0.0], [8.0, 0.0], [8.0, 6.0], [0.0, 6.0]],
                },
            }],
        }])
        .with_entrances(vec![NavigationEntranceDefinition {
            key: "exterior_entrance".to_string(),
            floor_key: "ground".to_string(),
            region_key: Some("main".to_string()),
            local_position_xz: [4.0, 0.0],
            radius_meters: 1.5,
            interior_spawn_local: [4.0, 0.0, 1.5],
            bidirectional: true,
            door_key: None,
        }])
}

fn concave_fixture() -> (WorldData, crate::world::BuildingId, SpaceId) {
    let mut world = flat_world();
    let building_id = activate_fixture(
        &mut world,
        oversized_concave_hut_blueprint(),
        pos(80.0, 80.0),
    );
    let interior_space = region_space(&world, building_id, "ground", "main");
    (world, building_id, interior_space)
}

fn grounded_cell_in_space(
    world: &WorldData,
    space_id: SpaceId,
    position: WorldPosition,
    config: NavigationConfig,
) -> GridCoord {
    let grounded =
        ground_position_in_space(world, world.space_registry(), space_id, position).unwrap();
    grid_coord_at_position(grounded, layout(), config.config_for_space(space_id))
}

#[test]
fn cardinal_neighbor_across_closed_interior_boundary_is_rejected() {
    let (world, building_id, interior_space) = concave_fixture();
    let catalogs = TestCatalogs::new();
    let config = NavigationConfig::default();
    let floor_y = world
        .space_registry()
        .get_space(interior_space)
        .map(|space| space.floor_y_global)
        .unwrap_or(1.27);
    let west = local_xz_to_world(&world, building_id, Vec2::new(5.75, 10.0), floor_y);
    let east = local_xz_to_world(&world, building_id, Vec2::new(6.25, 10.0), floor_y);
    let west_cell = grounded_cell_in_space(&world, interior_space, west, config);
    let east_cell = grounded_cell_in_space(&world, interior_space, east, config);
    assert!(
        !grid_neighbor_transition_legal_in_space(
            &world,
            world.space_registry(),
            catalogs.pass(),
            config,
            agent(),
            west_cell,
            east_cell,
            interior_space,
            layout(),
        ),
        "cardinal transition across closed interior boundary must be illegal"
    );
    assert!(matches!(
        query_navigation_segment_legality(
            &world,
            world.space_registry(),
            catalogs.pass(),
            config,
            interior_space,
            agent(),
            ground_position_in_space(&world, world.space_registry(), interior_space, west).unwrap(),
            ground_position_in_space(&world, world.space_registry(), interior_space, east).unwrap(),
            layout(),
        ),
        NavigationSegmentLegality::Blocked {
            reason: NavigationSegmentBlockReason::RegionBoundary,
            ..
        }
    ));
}

#[test]
fn diagonal_segment_across_concave_corner_is_rejected() {
    let (world, building_id, interior_space) = concave_fixture();
    let catalogs = TestCatalogs::new();
    let config = NavigationConfig::default();
    let floor_y = world
        .space_registry()
        .get_space(interior_space)
        .map(|space| space.floor_y_global)
        .unwrap_or(1.27);
    let southwest = local_xz_to_world(&world, building_id, Vec2::new(5.75, 5.75), floor_y);
    let northeast = local_xz_to_world(&world, building_id, Vec2::new(6.25, 6.25), floor_y);
    assert!(
        !query_navigation_segment_legality(
            &world,
            world.space_registry(),
            catalogs.pass(),
            config,
            interior_space,
            agent(),
            ground_position_in_space(&world, world.space_registry(), interior_space, southwest)
                .unwrap(),
            ground_position_in_space(&world, world.space_registry(), interior_space, northeast)
                .unwrap(),
            layout(),
        )
        .is_legal(),
        "diagonal segment across concave corner must be illegal"
    );
    let from_cell = grounded_cell_in_space(&world, interior_space, southwest, config);
    let to_cell = grounded_cell_in_space(&world, interior_space, northeast, config);
    if from_cell.x + 1 == to_cell.x && from_cell.z + 1 == to_cell.z {
        assert!(
            !grid_neighbor_transition_legal_in_space(
                &world,
                world.space_registry(),
                catalogs.pass(),
                config,
                agent(),
                from_cell,
                to_cell,
                interior_space,
                layout(),
            ),
            "diagonal grid transition across concave corner must be illegal"
        );
    }
}

#[test]
fn concave_interior_astar_avoids_illegal_boundary_shortcut() {
    let (world, building_id, interior_space) = concave_fixture();
    let catalogs = TestCatalogs::new();
    let config = NavigationConfig::default();
    let floor_y = world
        .space_registry()
        .get_space(interior_space)
        .map(|space| space.floor_y_global)
        .unwrap_or(1.27);
    let start = local_xz_to_world(&world, building_id, Vec2::new(2.0, 2.0), floor_y);
    let goal = local_xz_to_world(&world, building_id, Vec2::new(9.0, 9.0), floor_y);
    let start_cell = grounded_cell_in_space(&world, interior_space, start, config);
    let goal_cell = grounded_cell_in_space(&world, interior_space, goal, config);
    let path = astar_path_in_space(
        &world,
        world.space_registry(),
        catalogs.pass(),
        config,
        agent(),
        start_cell,
        goal_cell,
        interior_space,
    )
    .expect("concave interior route exists around notch");
    assert!(path.len() >= 2);
    assert_consecutive_waypoints_legal(&world, interior_space, &catalogs, config, &path);
    assert!(
        !query_navigation_segment_legality(
            &world,
            world.space_registry(),
            catalogs.pass(),
            config,
            interior_space,
            agent(),
            ground_position_in_space(&world, world.space_registry(), interior_space, start)
                .unwrap(),
            ground_position_in_space(&world, world.space_registry(), interior_space, goal).unwrap(),
            layout(),
        )
        .is_legal(),
        "direct concave diagonal must remain illegal"
    );
}

#[test]
fn open_surface_astar_finds_route() {
    let world = flat_world();
    let catalogs = TestCatalogs::new();
    let config = NavigationConfig::default();
    let start = pos(8.0, 8.0);
    let goal = pos(40.0, 40.0);
    let start_cell = grounded_cell_in_space(&world, SpaceId::SURFACE, start, config);
    let goal_cell = grounded_cell_in_space(&world, SpaceId::SURFACE, goal, config);
    let path = astar_path(
        &world,
        catalogs.pass(),
        config,
        agent(),
        start_cell,
        goal_cell,
    )
    .expect("open surface route");
    assert!(path.len() >= 2);
    assert_consecutive_waypoints_legal(&world, SpaceId::SURFACE, &catalogs, config, &path);
}

#[test]
fn ghost_building_footprint_does_not_block_surface_planner() {
    let mut world = flat_world();
    let catalogs = TestCatalogs::new();
    create_building(
        &catalogs.building,
        &mut world,
        &BuildingDefinitionId::new("hut"),
        pos(50.0, 50.0),
        Quat::IDENTITY,
        BuildingSource::Authored,
        BuildingOwnership::neutral(),
        None,
    )
    .unwrap();
    let config = NavigationConfig::default();
    let path = find_path(
        &world,
        catalogs.pass(),
        &config,
        AGENT_RADIUS,
        MAX_SLOPE,
        pos(48.0, 48.0),
        pos(52.0, 52.0),
    )
    .expect("ghost building must not block surface path");
    let positions: Vec<_> = path.waypoints.iter().map(|wp| wp.position).collect();
    assert_consecutive_waypoints_legal(&world, SpaceId::SURFACE, &catalogs, config, &positions);
}

#[test]
fn doodad_blocks_surface_planner_through_universal_legality() {
    let mut world = flat_world();
    let catalogs = TestCatalogs::new();
    create_doodad(
        &catalogs.doodad,
        &mut world,
        &DoodadDefinitionId::new("tree_oak"),
        pos(50.0, 50.0),
        DoodadSource::Authored,
        DoodadPlacementOverrides::default(),
        None,
    )
    .unwrap();
    let config = NavigationConfig::default();
    let blocked = find_path(
        &world,
        catalogs.pass(),
        &config,
        AGENT_RADIUS,
        MAX_SLOPE,
        pos(49.0, 50.0),
        pos(51.0, 50.0),
    );
    assert!(
        blocked.is_err(),
        "segment through doodad center must not be planned"
    );
    assert!(matches!(
        query_navigation_point_legality(
            &world,
            catalogs.pass(),
            pos(50.0, 50.0),
            PassabilityAgent {
                radius_meters: AGENT_RADIUS,
                max_slope_degrees: MAX_SLOPE,
            },
            SpaceId::SURFACE,
        ),
        PassabilityResult::Blocked {
            reason: PassabilityBlockReason::DoodadOccupied,
            ..
        }
    ));
}

#[test]
fn simplifier_removes_collinear_interior_route() {
    let mut world = flat_world();
    let building_id = activate_fixture(&mut world, one_region_test_blueprint(), pos(80.0, 80.0));
    let interior_space = region_space(&world, building_id, "ground", "main");
    let catalogs = TestCatalogs::new();
    let config = NavigationConfig::default();
    let floor_y = world
        .space_registry()
        .get_space(interior_space)
        .map(|space| space.floor_y_global)
        .unwrap_or(0.0);
    let mut waypoints = vec![
        local_xz_to_world(&world, building_id, Vec2::new(2.0, 2.0), floor_y),
        local_xz_to_world(&world, building_id, Vec2::new(4.0, 2.0), floor_y),
        local_xz_to_world(&world, building_id, Vec2::new(6.0, 2.0), floor_y),
    ];
    simplify_navigation_path_in_space(
        &mut waypoints,
        &world,
        world.space_registry(),
        catalogs.pass(),
        config,
        interior_space,
        agent(),
        layout(),
    );
    assert!(waypoints.len() <= 2);
    assert_consecutive_waypoints_legal(&world, interior_space, &catalogs, config, &waypoints);
}

#[test]
fn simplifier_blocks_shortcut_across_closed_interior_boundary() {
    let (world, building_id, interior_space) = concave_fixture();
    let catalogs = TestCatalogs::new();
    let config = NavigationConfig::default();
    let floor_y = world
        .space_registry()
        .get_space(interior_space)
        .map(|space| space.floor_y_global)
        .unwrap_or(1.27);
    let from = local_xz_to_world(&world, building_id, Vec2::new(2.0, 2.0), floor_y);
    let to = local_xz_to_world(&world, building_id, Vec2::new(9.0, 9.0), floor_y);
    let mut waypoints = vec![from, to];
    simplify_navigation_path_in_space(
        &mut waypoints,
        &world,
        world.space_registry(),
        catalogs.pass(),
        config,
        interior_space,
        agent(),
        layout(),
    );
    assert_eq!(
        waypoints.len(),
        2,
        "illegal shortcut must not simplify away"
    );
    assert!(!has_walkable_line_of_sight_surface(
        &world,
        catalogs.pass(),
        config,
        agent(),
        from,
        to,
        layout(),
    ));
}

#[test]
fn simplifier_blocks_shortcut_across_doodad_on_surface() {
    let mut world = flat_world();
    let catalogs = TestCatalogs::new();
    create_doodad(
        &catalogs.doodad,
        &mut world,
        &DoodadDefinitionId::new("tree_oak"),
        pos(50.0, 50.0),
        DoodadSource::Authored,
        DoodadPlacementOverrides::default(),
        None,
    )
    .unwrap();
    let config = NavigationConfig::default();
    let from = pos(48.0, 50.0);
    let to = pos(52.0, 50.0);
    let mut waypoints = vec![from, to];
    simplify_navigation_path_in_space(
        &mut waypoints,
        &world,
        world.space_registry(),
        catalogs.pass(),
        config,
        SpaceId::SURFACE,
        agent(),
        layout(),
    );
    assert_eq!(waypoints.len(), 2);
    assert!(
        !query_navigation_segment_legality(
            &world,
            world.space_registry(),
            catalogs.pass(),
            config,
            SpaceId::SURFACE,
            agent(),
            from,
            to,
            layout(),
        )
        .is_legal()
    );
}

#[test]
fn executor_accepts_universally_legal_interior_segment() {
    let (mut world, building_id, interior_space) = concave_fixture();
    let catalogs = TestCatalogs::new();
    let unit_catalog = crate::world::UnitCatalog::default();
    let floor_y = world
        .space_registry()
        .get_space(interior_space)
        .map(|space| space.floor_y_global)
        .unwrap_or(1.27);
    let start = local_xz_to_world(&world, building_id, Vec2::new(2.0, 2.0), floor_y);
    let goal = local_xz_to_world(&world, building_id, Vec2::new(3.5, 3.5), floor_y);
    let unit_id = create_unit_with_ownership(
        &unit_catalog,
        &mut world,
        &UnitDefinitionId::new("wolf"),
        start,
        UnitSource::Authored,
        UnitOwnership::player_default(),
    )
    .unwrap()
    .id;
    world
        .set_unit_current_space(unit_id, interior_space)
        .unwrap();
    world
        .set_unit_state(
            unit_id,
            UnitState::Moving {
                target: goal,
                path: NavigationPath::new(vec![NavigationWaypoint::in_space(goal, interior_space)]),
                waypoint_index: 0,
            },
        )
        .unwrap();
    let before = world.get_unit(unit_id).unwrap().placement.position;
    let outcome = step_unit_movement(&mut world, &unit_catalog, catalogs.pass(), unit_id, 0.5);
    assert_ne!(
        outcome,
        crate::world::unit::UnitMovementStepOutcome::Failed(
            crate::world::unit::UnitMovementError::UnitNotFound
        )
    );
    let after = world.get_unit(unit_id).unwrap().placement.position;
    let layout = world.layout();
    let moved = after
        .to_global(layout)
        .xz()
        .distance(before.to_global(layout).xz());
    assert!(
        moved > 0.05,
        "executor should advance along universally legal interior segment"
    );
}

#[test]
fn executor_rejects_universally_illegal_interior_segment() {
    let (mut world, building_id, interior_space) = concave_fixture();
    let catalogs = TestCatalogs::new();
    let unit_catalog = crate::world::UnitCatalog::default();
    let floor_y = world
        .space_registry()
        .get_space(interior_space)
        .map(|space| space.floor_y_global)
        .unwrap_or(1.27);
    let start = local_xz_to_world(&world, building_id, Vec2::new(2.0, 2.0), floor_y);
    let illegal_goal = local_xz_to_world(&world, building_id, Vec2::new(9.0, 9.0), floor_y);
    assert!(
        !query_navigation_segment_legality(
            &world,
            world.space_registry(),
            catalogs.pass(),
            NavigationConfig::default(),
            interior_space,
            agent(),
            ground_position_in_space(&world, world.space_registry(), interior_space, start)
                .unwrap(),
            ground_position_in_space(&world, world.space_registry(), interior_space, illegal_goal,)
                .unwrap(),
            layout(),
        )
        .is_legal()
    );
    let unit_id = create_unit_with_ownership(
        &unit_catalog,
        &mut world,
        &UnitDefinitionId::new("wolf"),
        start,
        UnitSource::Authored,
        UnitOwnership::player_default(),
    )
    .unwrap()
    .id;
    world
        .set_unit_current_space(unit_id, interior_space)
        .unwrap();
    world
        .set_unit_state(
            unit_id,
            UnitState::Moving {
                target: illegal_goal,
                path: NavigationPath::new(vec![NavigationWaypoint::in_space(
                    illegal_goal,
                    interior_space,
                )]),
                waypoint_index: 0,
            },
        )
        .unwrap();
    let before_xz = world
        .get_unit(unit_id)
        .unwrap()
        .placement
        .position
        .to_global(layout())
        .xz();
    step_unit_movement(&mut world, &unit_catalog, catalogs.pass(), unit_id, 0.5);
    let after_xz = world
        .get_unit(unit_id)
        .unwrap()
        .placement
        .position
        .to_global(layout())
        .xz();
    assert!(
        after_xz.distance(illegal_goal.to_global(layout()).xz()) > 3.0,
        "executor must not advance across universally illegal interior segment"
    );
    assert!(
        after_xz.distance(before_xz) < 1.0,
        "executor should not teleport toward illegal goal"
    );
}

#[test]
/// Downstream integration: concave interior routing after membership is already interior.
fn vertical_player_command_inside_concave_region_moves_unit() {
    let (mut world, building_id, interior_space) = concave_fixture();
    let unit_catalog = crate::world::UnitCatalog::default();
    let weapon_catalog = WeaponCatalog::default();
    let catalogs = TestCatalogs::new();
    let floor_y = world
        .space_registry()
        .get_space(interior_space)
        .map(|space| space.floor_y_global)
        .unwrap_or(1.27);
    let start = local_xz_to_world(&world, building_id, Vec2::new(3.0, 3.0), floor_y);
    let goal = local_xz_to_world(&world, building_id, Vec2::new(10.0, 10.0), floor_y);
    let unit_id = create_unit_with_ownership(
        &unit_catalog,
        &mut world,
        &UnitDefinitionId::new("wolf"),
        start,
        UnitSource::Authored,
        UnitOwnership::player_default(),
    )
    .unwrap()
    .id;
    assert_eq!(
        world.get_unit(unit_id).unwrap().current_space_id,
        interior_space,
        "spawn inside concave region must initialize interior membership"
    );
    let mut selected = SelectedUnits::default();
    selected.set_single(unit_id);
    let report = issue_move_orders_to_selection(
        &mut world,
        &selected,
        &unit_catalog,
        &weapon_catalog,
        &catalogs.doodad,
        &NavigationConfig::default(),
        goal,
        AttackTargetingPolicy::default(),
    );
    assert_eq!(report.issued, 1);
    let resolve_report = resolve_pending_unit_orders(
        &mut world,
        &unit_catalog,
        catalogs.pass(),
        &NavigationConfig::default(),
    );
    assert_eq!(resolve_report.resolved, 1);
    let command = world
        .movement_authority_trace()
        .latest_command_for_unit(unit_id)
        .expect("command trace");
    assert_eq!(command.start_space, interior_space);
    assert_eq!(command.goal_space, interior_space);
    if let UnitState::Moving { path, .. } = &world.get_unit(unit_id).unwrap().state {
        let positions: Vec<_> = path.waypoints.iter().map(|wp| wp.position).collect();
        assert_consecutive_waypoints_legal(
            &world,
            interior_space,
            &catalogs,
            NavigationConfig::default(),
            &positions,
        );
    } else {
        panic!("unit should be moving after order resolution");
    }
    let layout = world.layout();
    let goal_xz = goal.to_global(layout).xz();
    let mut arrived = false;
    for _ in 0..800 {
        step_unit_movement(&mut world, &unit_catalog, catalogs.pass(), unit_id, 0.25);
        let record = world.get_unit(unit_id).unwrap();
        if record.current_space_id != interior_space {
            panic!("inside-region move must not change space without portal");
        }
        let pos_xz = record.placement.position.to_global(layout).xz();
        if pos_xz.distance(goal_xz) < 1.5 && matches!(record.state, UnitState::Idle) {
            arrived = true;
            break;
        }
    }
    assert!(
        arrived,
        "unit inside concave region should reach legal interior goal via planner/executor stack"
    );
}

#[test]
fn vertical_player_command_does_not_cross_closed_boundary_to_nowhere() {
    let (mut world, building_id, interior_space) = concave_fixture();
    let unit_catalog = crate::world::UnitCatalog::default();
    let weapon_catalog = WeaponCatalog::default();
    let catalogs = TestCatalogs::new();
    let floor_y = world
        .space_registry()
        .get_space(interior_space)
        .map(|space| space.floor_y_global)
        .unwrap_or(1.27);
    let start = local_xz_to_world(&world, building_id, Vec2::new(2.0, 2.0), floor_y);
    let void_goal = local_xz_to_world(&world, building_id, Vec2::new(3.0, 8.0), floor_y);
    assert!(
        matches!(
            query_navigation_point_legality(
                &world,
                catalogs.pass(),
                void_goal,
                PassabilityAgent {
                    radius_meters: AGENT_RADIUS,
                    max_slope_degrees: MAX_SLOPE,
                },
                interior_space,
            ),
            PassabilityResult::Blocked { .. }
        ),
        "concave notch void must be blocked for point legality"
    );
    let unit_id = create_unit_with_ownership(
        &unit_catalog,
        &mut world,
        &UnitDefinitionId::new("wolf"),
        start,
        UnitSource::Authored,
        UnitOwnership::player_default(),
    )
    .unwrap()
    .id;
    world
        .set_unit_current_space(unit_id, interior_space)
        .unwrap();
    let mut selected = SelectedUnits::default();
    selected.set_single(unit_id);
    issue_move_orders_to_selection(
        &mut world,
        &selected,
        &unit_catalog,
        &weapon_catalog,
        &catalogs.doodad,
        &NavigationConfig::default(),
        void_goal,
        AttackTargetingPolicy::default(),
    );
    resolve_pending_unit_orders(
        &mut world,
        &unit_catalog,
        catalogs.pass(),
        &NavigationConfig::default(),
    );
    let layout = world.layout();
    let void_xz = void_goal.to_global(layout).xz();
    let mut reached_void = false;
    for _ in 0..400 {
        step_unit_movement(&mut world, &unit_catalog, catalogs.pass(), unit_id, 0.25);
        let record = world.get_unit(unit_id).unwrap();
        let pos_xz = record.placement.position.to_global(layout).xz();
        if pos_xz.distance(void_xz) < 1.0 {
            reached_void = true;
            break;
        }
    }
    assert!(
        !reached_void,
        "unit must not reach concave notch void across closed region boundary"
    );
}
