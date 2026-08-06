//! Interior region clearance measurements and IN-11d regressions.

use bevy::prelude::*;

use super::grid::{NavigationAgent, NavigationConfig, grid_coord_at_position};
use super::interior_clearance::{measure_interior_region_clearance, min_edge_clearance_meters};
use crate::world::{
    Affiliation, BuildingCatalog, BuildingCategoryCatalog, BuildingDefinition,
    BuildingDefinitionId, BuildingId, BuildingLifecycleState, BuildingNavigationBlueprint,
    BuildingNavigationBlueprintCatalog, BuildingNavigationBlueprintId, BuildingNavigationRuntime,
    BuildingOwnership, ChunkCoord, ChunkLayout, DoodadCatalog, FootprintCatalog,
    InteriorProfileCatalog, LocalPosition, NavigationEntranceDefinition, NavigationError,
    NavigationFloorDefinition, NavigationPolygon2d, NavigationRegionDefinition, OccupancyCatalogs,
    PassabilityCatalogs, RuntimeNavigationFloor, RuntimeNavigationRegion, SpaceId, SpaceRecord,
    UnitCatalog, UnitDefinition, UnitDefinitionId, UnitRenderKey, WeaponDefinitionId, WorldData,
    WorldPosition, find_path_with_spaces, ground_position_in_space, place_player_building,
    set_building_lifecycle_stage,
};

const ROBOT_RADIUS: f32 = 0.6;
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
        radius_meters: ROBOT_RADIUS,
        max_slope_degrees: MAX_SLOPE,
    }
}

fn imported_survival_hut_definition() -> BuildingDefinition {
    BuildingDefinition::new(
        BuildingDefinitionId::new("hut"),
        "Survival Hut",
        crate::world::BuildingCategoryId::new("residential"),
        crate::world::BuildingRenderKey::reserved("hut"),
        crate::world::BuildingRenderKey::reserved("hut_collision"),
        250,
        45.0,
        crate::world::FootprintSpec::Rectangle {
            width_meters: 4.0,
            depth_meters: 4.0,
        },
        35.0,
        true,
    )
}

fn imported_building_catalog() -> BuildingCatalog {
    BuildingCatalog::from_definitions(
        vec![imported_survival_hut_definition()],
        &BuildingCategoryCatalog::default(),
    )
    .expect("imported hut catalog")
}

fn persisted_hut_nav_blueprint() -> BuildingNavigationBlueprint {
    BuildingNavigationBlueprint::new("hut_nav", "Survival Hut Navigation")
        .with_floors(vec![NavigationFloorDefinition {
            floor_id: 0,
            key: "floor_0".to_string(),
            display_label: "Floor 1.3m".to_string(),
            elevation_meters: 1.269_120_6,
            visibility_group_id: 1,
            room_tag: None,
            walkable_outline_legacy: None,
            regions: vec![NavigationRegionDefinition {
                key: "region_3".to_string(),
                display_label: "Region 3".to_string(),
                room_tag: None,
                walkable_outline: NavigationPolygon2d {
                    vertices_xz: vec![
                        [3.979_980_5, -3.513_916],
                        [4.400_512_7, 4.112_793],
                        [-3.815_185_5, 4.112_793],
                        [-3.553_222_7, -3.517_578_1],
                    ],
                },
            }],
        }])
        .with_entrances(vec![NavigationEntranceDefinition {
            key: "entrance".to_string(),
            floor_key: "floor_0".to_string(),
            region_key: Some("region_3".to_string()),
            local_position_xz: [0.058_471_68, -2.773_437_5],
            radius_meters: 1.5,
            interior_spawn_local: [0.058_471_68, 1.269_120_6, -2.059_437_5],
            bidirectional: true,
            door_key: None,
        }])
}

fn persisted_nav_catalog() -> BuildingNavigationBlueprintCatalog {
    BuildingNavigationBlueprintCatalog::from_definitions(vec![persisted_hut_nav_blueprint()])
        .expect("persisted hut_nav catalog")
}

fn imported_robot_catalog() -> UnitCatalog {
    UnitCatalog::from_definitions(vec![UnitDefinition::new(
        UnitDefinitionId::new("robot"),
        "Robot",
        "Player",
        1,
        100,
        100,
        5,
        5,
        5,
        5,
        5,
        5,
        10.0,
        "Common",
        9.0,
        ROBOT_RADIUS,
        MAX_SLOPE,
        WeaponDefinitionId::new("weapon_fists"),
        true,
        UnitRenderKey::reserved("robot"),
    )])
    .expect("robot catalog")
}

fn activate_imported_hut(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    nav_catalog: &BuildingNavigationBlueprintCatalog,
) -> BuildingId {
    let doodad_catalog = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let occupancy = OccupancyCatalogs {
        building: building_catalog,
        doodad: &doodad_catalog,
        footprint: &footprint,
    };
    let interior = InteriorProfileCatalog::default();

    let building_id = place_player_building(
        building_catalog,
        world,
        &BuildingDefinitionId::new("hut"),
        pos(80.0, 80.0),
        Quat::IDENTITY,
        BuildingOwnership::with_affiliation(Affiliation::Player),
        occupancy,
    )
    .expect("place hut")
    .id;

    set_building_lifecycle_stage(
        world,
        building_catalog,
        &interior,
        &doodad_catalog,
        occupancy,
        Some(nav_catalog),
        building_id,
        BuildingLifecycleState::Complete,
        1.0,
    )
    .expect("complete hut");
    building_id
}

fn layout_world() -> WorldData {
    let layout = layout();
    let mut world = WorldData::new(layout);
    let heightfield = crate::world::Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
    world.insert(
        crate::world::ChunkId::new(ChunkCoord::new(0, 0)),
        crate::world::ChunkData::new(heightfield, Vec::new()),
    );
    world
}

struct TestHarness {
    building_catalog: BuildingCatalog,
    doodad: DoodadCatalog,
    footprint: FootprintCatalog,
}

impl TestHarness {
    fn imported_hut() -> Self {
        Self {
            building_catalog: imported_building_catalog(),
            doodad: DoodadCatalog::default(),
            footprint: FootprintCatalog::default(),
        }
    }

    fn pass(&self) -> PassabilityCatalogs<'_> {
        PassabilityCatalogs {
            doodad: &self.doodad,
            building: &self.building_catalog,
            footprint: &self.footprint,
        }
    }
}

fn register_interior_floor(world: &mut WorldData, outline: Vec<Vec2>, floor_key: &str) -> SpaceId {
    let space_id = world.space_registry_mut().allocate_space_id();
    world.space_registry_mut().insert_space(SpaceRecord {
        id: space_id,
        owning_building_id: Some(BuildingId::new(1)),
        display_floor_label: floor_key.to_string(),
        visibility_group_id: 1,
        reference_elevation: 2.0,
        floor_y_global: 2.0,
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
            blueprint_id: BuildingNavigationBlueprintId::new("clearance_test"),
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
        elevation_meters: 2.0,
        world_outline_xz: outline,
        world_aabb_min_xz: min,
        world_aabb_max_xz: max,
    });
    floors.push(RuntimeNavigationFloor {
        floor_id,
        floor_key: floor_key.to_string(),
        elevation_meters: 2.0,
        visibility_group_id: 1,
        region_space_ids: vec![space_id],
    });
    world
        .building_navigation_runtime_mut()
        .insert(BuildingNavigationRuntime {
            building_id: BuildingId::new(1),
            blueprint_id: BuildingNavigationBlueprintId::new("clearance_test"),
            model_transform: Transform::IDENTITY,
            space_keys: runtime.space_keys,
            portal_keys: runtime.portal_keys,
            floors,
            regions,
        });
    space_id
}

fn rectangular_outline(min_x: f32, min_z: f32, max_x: f32, max_z: f32) -> Vec<Vec2> {
    vec![
        Vec2::new(min_x, min_z),
        Vec2::new(max_x, min_z),
        Vec2::new(max_x, max_z),
        Vec2::new(min_x, max_z),
    ]
}

fn blueprint_local_outline_xz(blueprint: &crate::world::BuildingNavigationBlueprint) -> Vec<Vec2> {
    blueprint
        .floors
        .first()
        .and_then(|floor| floor.regions.first())
        .map(|region| {
            region
                .walkable_outline
                .vertices_xz
                .iter()
                .map(|v| Vec2::new(v[0], v[1]))
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn tight_hut_clearance_measurements_show_discretization_not_scale_mismatch() {
    let mut world = layout_world();
    let harness = TestHarness::imported_hut();
    let building_catalog = &harness.building_catalog;
    let nav_catalog = persisted_nav_catalog();
    let blueprint = persisted_hut_nav_blueprint();
    let building_id = activate_imported_hut(&mut world, &building_catalog, &nav_catalog);

    let runtime = world
        .building_navigation_runtime()
        .get(building_id)
        .expect("runtime");
    let region = &runtime.regions[0];
    let space_id = region.space_id;
    let floor_y = world
        .space_registry()
        .get_space(space_id)
        .expect("space")
        .floor_y_global;

    let portal_landing = WorldPosition::from_global(
        runtime
            .model_transform
            .transform_point(Vec3::new(0.058_471_68, 1.269_120_6, -2.059_437_5)),
        world.layout(),
    );
    let goal = WorldPosition::from_global(
        {
            let interior = runtime
                .model_transform
                .transform_point(Vec3::new(0.0, 0.0, 1.0));
            Vec3::new(interior.x, floor_y, interior.z)
        },
        world.layout(),
    );

    let config = NavigationConfig::default();
    let catalogs = harness.pass();
    let report = measure_interior_region_clearance(
        &world,
        world.space_registry(),
        world.building_navigation_runtime(),
        catalogs,
        &config,
        agent(),
        space_id,
        &blueprint_local_outline_xz(&blueprint),
        Some(portal_landing),
        Some(goal),
    )
    .expect("clearance report");

    // 1–3: authored vs runtime spans and robot footprint.
    assert!(
        report.blueprint_local_width_meters > 7.0 && report.blueprint_local_depth_meters > 7.0,
        "authored mesh-unit span {:?} x {:?}",
        report.blueprint_local_width_meters,
        report.blueprint_local_depth_meters
    );
    assert!(
        report.runtime_width_meters > 3.0 && report.runtime_depth_meters > 3.0,
        "runtime world span {:.2} x {:.2} m (scale mismatch would be >>10x)",
        report.runtime_width_meters,
        report.runtime_depth_meters
    );
    assert_eq!(report.agent_radius_meters, ROBOT_RADIUS);
    assert_eq!(report.agent_diameter_meters, ROBOT_RADIUS * 2.0);

    // 4–5: portal and goal edge clearance.
    assert!(report.portal_landing_inside);
    assert!(report.goal_inside);
    let portal_clear = report.portal_landing_min_edge_clearance_meters.unwrap();
    let goal_clear = report.goal_min_edge_clearance_meters.unwrap();
    assert!(
        portal_clear > ROBOT_RADIUS,
        "portal landing clearance {:.2} m should exceed robot radius",
        portal_clear
    );
    assert!(
        goal_clear > ROBOT_RADIUS,
        "goal clearance {:.2} m should exceed robot radius",
        goal_clear
    );

    // 6–9: interior grid probes inside the region.
    assert_eq!(report.interior_cell_spacing_meters, 0.5);
    assert!(report.cells_inside_region > 0);
    assert!(
        report.permissive_walkable_cells > 0,
        "tight hut must contain permissive walkable cells"
    );
    assert!(
        report.connected_walkable_component > 0,
        "portal landing must reach a connected walkable component"
    );

    // 11: direct segment when grid route is sparse.
    assert!(
        report.direct_segment_clear,
        "portal landing and interior goal share a clear direct segment"
    );

    // Oversized polygon only adds peripheral cells; tight room already has walkable cells.
    assert!(
        report.permissive_walkable_cells > 0,
        "success does not depend on enlarging beyond the building footprint"
    );
}

#[test]
fn tight_room_that_fits_robot_paths_with_direct_segment_or_grid() {
    let mut world = layout_world();
    let harness = TestHarness::imported_hut();
    let building_catalog = &harness.building_catalog;
    // 4 m x 4 m room — fits 1.2 m robot diameter with margin.
    let space_id = register_interior_floor(
        &mut world,
        rectangular_outline(50.0, 50.0, 54.0, 54.0),
        "fits_robot",
    );
    let start = pos(51.0, 51.0);
    let goal = pos(53.0, 53.0);
    let config = NavigationConfig::default();
    let catalogs = harness.pass();

    let report = measure_interior_region_clearance(
        &world,
        world.space_registry(),
        world.building_navigation_runtime(),
        catalogs,
        &config,
        agent(),
        space_id,
        &rectangular_outline(0.0, 0.0, 4.0, 4.0),
        Some(start),
        Some(goal),
    )
    .expect("report");

    assert!(
        report.permissive_walkable_cells > 0 && report.connected_walkable_component > 0,
        "4x4 m room must expose connected walkable cells"
    );

    let path = find_path_with_spaces(
        &world,
        catalogs,
        &config,
        ROBOT_RADIUS,
        MAX_SLOPE,
        start,
        goal,
        space_id,
        space_id,
        None,
    )
    .expect("tight but valid room must path");
    assert!(path.len() >= 2);
}

#[test]
fn narrow_room_fails_with_agent_clearance_evidence() {
    let mut world = layout_world();
    let harness = TestHarness::imported_hut();
    let building_catalog = &harness.building_catalog;
    // 1.0 m interior width — narrower than the 1.2 m robot diameter.
    let space_id = register_interior_floor(
        &mut world,
        rectangular_outline(60.0, 60.0, 61.0, 63.0),
        "too_narrow",
    );
    let start = pos(60.5, 61.0);
    let goal = pos(60.5, 62.0);
    let config = NavigationConfig::default();
    let catalogs = harness.pass();
    let polygon = rectangular_outline(60.0, 60.0, 61.0, 63.0);

    let report = measure_interior_region_clearance(
        &world,
        world.space_registry(),
        world.building_navigation_runtime(),
        catalogs,
        &config,
        agent(),
        space_id,
        &rectangular_outline(0.0, 0.0, 1.0, 3.0),
        Some(start),
        Some(goal),
    )
    .expect("report");

    let portal_clear = report
        .portal_landing_min_edge_clearance_meters
        .expect("portal clearance");
    let goal_clear = report
        .goal_min_edge_clearance_meters
        .expect("goal clearance");
    assert!(
        portal_clear < ROBOT_RADIUS && goal_clear < ROBOT_RADIUS,
        "portal clearance {:.2} m and goal clearance {:.2} m must be below robot radius {:.2} m",
        portal_clear,
        goal_clear,
        ROBOT_RADIUS
    );

    let center_clearance = min_edge_clearance_meters(Vec2::new(60.5, 61.5), &polygon);
    assert!(
        center_clearance < ROBOT_RADIUS,
        "room center clearance {:.2} m < robot radius {:.2} m",
        center_clearance,
        ROBOT_RADIUS
    );

    let path_err = find_path_with_spaces(
        &world,
        catalogs,
        &config,
        ROBOT_RADIUS,
        MAX_SLOPE,
        start,
        goal,
        space_id,
        space_id,
        None,
    )
    .expect_err("narrow room must not produce a path");

    let portal_passability = crate::world::query_passability_in_space(
        &world,
        catalogs,
        start,
        crate::world::PassabilityAgent {
            radius_meters: ROBOT_RADIUS,
            max_slope_degrees: MAX_SLOPE,
        },
        space_id,
    );
    assert!(
        matches!(
            portal_passability,
            crate::world::PassabilityResult::Blocked {
                reason: crate::world::PassabilityBlockReason::AgentClearanceInsufficient,
                ..
            }
        ),
        "expected agent-clearance block at portal, got {:?}",
        portal_passability
    );
    assert!(
        matches!(
            path_err,
            NavigationError::StartBlocked | NavigationError::GoalBlocked | NavigationError::NoPath
        ),
        "expected blocked path, got {:?}",
        path_err
    );
}

#[test]
fn real_hut_interior_segment_path_after_portal_with_robot_radius() {
    let mut world = layout_world();
    let harness = TestHarness::imported_hut();
    let building_catalog = &harness.building_catalog;
    let nav_catalog = persisted_nav_catalog();
    let unit_catalog = imported_robot_catalog();
    let building_id = activate_imported_hut(&mut world, &building_catalog, &nav_catalog);

    let runtime = world
        .building_navigation_runtime()
        .get(building_id)
        .expect("runtime")
        .clone();
    let space_id = runtime.regions[0].space_id;
    let floor_y = world
        .space_registry()
        .get_space(space_id)
        .expect("space")
        .floor_y_global;

    let portal_landing = WorldPosition::from_global(
        runtime
            .model_transform
            .transform_point(Vec3::new(0.058_471_68, 1.269_120_6, -2.059_437_5)),
        world.layout(),
    );
    let goal = WorldPosition::from_global(
        {
            let interior = runtime
                .model_transform
                .transform_point(Vec3::new(0.0, 0.0, 1.0));
            Vec3::new(interior.x, floor_y, interior.z)
        },
        world.layout(),
    );

    let catalogs = harness.pass();
    let config = NavigationConfig::default();
    let robot_radius = unit_catalog
        .get(&crate::world::UnitDefinitionId::new("robot"))
        .expect("robot")
        .collision_radius_meters;

    let path = find_path_with_spaces(
        &world,
        catalogs,
        &config,
        robot_radius,
        MAX_SLOPE,
        portal_landing,
        goal,
        space_id,
        space_id,
        None,
    )
    .expect("real hut interior path with robot radius");

    assert!(
        path.len() <= 2,
        "direct-segment path expected for portal landing to interior goal, got {} waypoints",
        path.len()
    );

    let layout = world.layout();
    let space_config = config.config_for_space(space_id);
    let start_cell = grid_coord_at_position(
        ground_position_in_space(&world, world.space_registry(), space_id, portal_landing)
            .expect("grounded portal"),
        layout,
        space_config,
    );
    let goal_cell = grid_coord_at_position(
        ground_position_in_space(&world, world.space_registry(), space_id, goal)
            .expect("grounded goal"),
        layout,
        space_config,
    );
    assert_ne!(
        start_cell, goal_cell,
        "discretization places portal and goal in different cells — grid alone is insufficient"
    );
}
