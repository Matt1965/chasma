//! NAV-GROUND-1 regressions: blueprint support Surface exclusion and Entrance corridors.

use std::path::Path;

use bevy::prelude::*;

use super::adapt::region_space_key;
use super::catalog::{
    BUILDING_NAVIGATION_BLUEPRINT_CATALOG_RON_PATH, BuildingNavigationBlueprintCatalog,
};
use super::definition::{
    BuildingNavigationBlueprint, NavigationEntranceDefinition, NavigationFloorDefinition,
    NavigationPolygon2d, NavigationRegionDefinition,
};
use super::fixtures::{
    dual_doorway_navigation_blueprint, one_region_doorless_navigation_blueprint,
    two_room_hut_navigation_blueprint,
};
use crate::world::{
    Affiliation, BuildingCatalog, BuildingCategoryCatalog, BuildingDefinition,
    BuildingDefinitionId, BuildingLifecycleState, BuildingNavigationBlueprintInstanceOverride,
    BuildingOwnership, BuildingRenderKey, ChunkCoord, ChunkData, ChunkId, ChunkLayout,
    DoodadCatalog, FootprintCatalog, InteriorProfileCatalog, LocalPosition, NavigationAgent,
    NavigationConfig, PassabilityAgent, PassabilityBlockReason, PassabilityCatalogs,
    PassabilityResult, SpaceId, WorldData, WorldPosition, create_building, find_path_with_spaces,
    place_player_building, query_navigation_point_legality, query_navigation_segment_legality,
    set_building_lifecycle_stage,
};

const ROBOT_RADIUS: f32 = 0.68;
const MAX_SLOPE: f32 = 45.0;

fn layout_world() -> WorldData {
    let layout = ChunkLayout {
        chunk_size_meters: 256.0,
        units_per_meter: 1.0,
    };
    let mut world = WorldData::new(layout);
    let heightfield = crate::world::Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
    world.insert(
        ChunkId::new(ChunkCoord::new(0, 0)),
        ChunkData::new(heightfield, Vec::new()),
    );
    world
}

fn pos(x: f32, z: f32) -> WorldPosition {
    WorldPosition::new(
        ChunkCoord::new(0, 0),
        LocalPosition::new(Vec3::new(x, 0.0, z)),
    )
}

fn agent(radius: f32) -> PassabilityAgent {
    PassabilityAgent {
        radius_meters: radius,
        max_slope_degrees: MAX_SLOPE,
    }
}

fn nav_agent(radius: f32) -> NavigationAgent {
    NavigationAgent {
        radius_meters: radius,
        max_slope_degrees: MAX_SLOPE,
    }
}

fn pass_catalogs<'a>(
    doodad: &'a DoodadCatalog,
    building: &'a BuildingCatalog,
    footprint: &'a FootprintCatalog,
) -> PassabilityCatalogs<'a> {
    PassabilityCatalogs {
        doodad,
        building,
        footprint,
    }
}

fn imported_hut_catalog() -> BuildingCatalog {
    BuildingCatalog::from_definitions(
        vec![BuildingDefinition::new(
            BuildingDefinitionId::new("hut"),
            "Survival Hut",
            crate::world::BuildingCategoryId::new("residential"),
            BuildingRenderKey::reserved("hut"),
            BuildingRenderKey::reserved("hut_collision"),
            250,
            45.0,
            crate::world::FootprintSpec::Rectangle {
                width_meters: 4.0,
                depth_meters: 4.0,
            },
            35.0,
            true,
        )],
        &BuildingCategoryCatalog::default(),
    )
    .expect("hut catalog")
}

fn persisted_nav_catalog() -> BuildingNavigationBlueprintCatalog {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join(BUILDING_NAVIGATION_BLUEPRINT_CATALOG_RON_PATH);
    BuildingNavigationBlueprintCatalog::load_from_ron_path(&path).expect("nav catalog")
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
    let occupancy = crate::world::OccupancyCatalogs {
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

fn activate_hut(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    nav_catalog: &BuildingNavigationBlueprintCatalog,
    placement: WorldPosition,
) -> crate::world::BuildingId {
    let doodad_catalog = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let occupancy = crate::world::OccupancyCatalogs {
        building: building_catalog,
        doodad: &doodad_catalog,
        footprint: &footprint,
    };
    let interior = InteriorProfileCatalog::default();
    let building_id = place_player_building(
        building_catalog,
        world,
        &BuildingDefinitionId::new("hut"),
        placement,
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

fn local_xz_to_world(
    world: &WorldData,
    building_id: crate::world::BuildingId,
    local_xz: Vec2,
) -> WorldPosition {
    let runtime = world
        .building_navigation_runtime()
        .get(building_id)
        .expect("runtime");
    let layout = world.layout();
    let floor_y = world
        .space_registry()
        .get_space(runtime.regions.first().expect("region").space_id)
        .map(|space| space.floor_y_global)
        .unwrap_or(0.0);
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
        .unwrap();
    let key = region_space_key(floor_key, region_key);
    runtime.space_keys.get(&key).copied().expect("space key")
}

fn raised_one_region_blueprint() -> BuildingNavigationBlueprint {
    BuildingNavigationBlueprint::new("raised_one_region", "Raised One Region")
        .with_floors(vec![NavigationFloorDefinition {
            floor_id: 0,
            key: "ground".to_string(),
            display_label: "Ground".to_string(),
            elevation_meters: 2.0,
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
            interior_spawn_local: [4.0, 2.0, 1.5],
            bidirectional: true,
            door_key: None,
        }])
}

fn wide_entrance_blueprint() -> BuildingNavigationBlueprint {
    BuildingNavigationBlueprint::new("wide_entrance", "Wide Entrance")
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
                    vertices_xz: vec![[0.0, 0.0], [12.0, 0.0], [12.0, 6.0], [0.0, 6.0]],
                },
            }],
        }])
        .with_entrances(vec![NavigationEntranceDefinition {
            key: "wide_test_entrance".to_string(),
            floor_key: "ground".to_string(),
            region_key: Some("main".to_string()),
            local_position_xz: [6.0, 0.0],
            radius_meters: 2.0,
            interior_spawn_local: [6.0, 0.0, 2.0],
            bidirectional: true,
            door_key: None,
        }])
}

fn concave_region_blueprint() -> BuildingNavigationBlueprint {
    BuildingNavigationBlueprint::new("concave_support", "Concave Support")
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
            interior_spawn_local: [7.0, 0.0, 1.5],
            bidirectional: true,
            door_key: None,
        }])
}

fn narrow_entrance_blueprint() -> BuildingNavigationBlueprint {
    BuildingNavigationBlueprint::new("narrow_entrance", "Narrow Entrance")
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
            radius_meters: 0.3,
            interior_spawn_local: [4.0, 0.0, 1.0],
            bidirectional: true,
            door_key: None,
        }])
}

#[test]
fn surface_point_beneath_support_is_blocked() {
    let mut world = layout_world();
    let building_id = activate_fixture(
        &mut world,
        one_region_doorless_navigation_blueprint(),
        pos(80.0, 80.0),
    );
    let under = local_xz_to_world(&world, building_id, Vec2::new(4.0, 3.0));
    let building_catalog = BuildingCatalog::default();
    let doodad = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let result = query_navigation_point_legality(
        &world,
        pass_catalogs(&doodad, &building_catalog, &footprint),
        under,
        agent(ROBOT_RADIUS),
        SpaceId::SURFACE,
    );
    assert!(
        matches!(
            result,
            PassabilityResult::Blocked {
                reason: PassabilityBlockReason::BlueprintSupport,
                ..
            }
        ),
        "expected BlueprintSupport, got {result:?}"
    );
}

#[test]
fn raised_floor_surface_under_projection_blocked() {
    let mut world = layout_world();
    let building_id =
        activate_fixture(&mut world, raised_one_region_blueprint(), pos(100.0, 100.0));
    let under = local_xz_to_world(&world, building_id, Vec2::new(4.0, 3.0));
    let building_catalog = BuildingCatalog::default();
    let doodad = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let result = query_navigation_point_legality(
        &world,
        pass_catalogs(&doodad, &building_catalog, &footprint),
        under,
        agent(ROBOT_RADIUS),
        SpaceId::SURFACE,
    );
    assert!(matches!(
        result,
        PassabilityResult::Blocked {
            reason: PassabilityBlockReason::BlueprintSupport,
            ..
        }
    ));
    let floor_y = world
        .space_registry()
        .get_space(region_space(&world, building_id, "ground", "main"))
        .unwrap()
        .floor_y_global;
    let terrain_y = crate::world::ground_world_position(&world, under)
        .expect("terrain")
        .to_global(world.layout())
        .y;
    assert!(
        (floor_y - terrain_y).abs() > 1.0,
        "surface Y should remain terrain while floor is raised"
    );
}

#[test]
fn surface_segment_through_support_interior_blocked() {
    let mut world = layout_world();
    let building_id = activate_fixture(
        &mut world,
        one_region_doorless_navigation_blueprint(),
        pos(80.0, 80.0),
    );
    let west = local_xz_to_world(&world, building_id, Vec2::new(1.0, 3.0));
    let east = local_xz_to_world(&world, building_id, Vec2::new(7.0, 3.0));
    let building_catalog = BuildingCatalog::default();
    let doodad = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let result = query_navigation_segment_legality(
        &world,
        world.space_registry(),
        pass_catalogs(&doodad, &building_catalog, &footprint),
        NavigationConfig::default(),
        SpaceId::SURFACE,
        nav_agent(ROBOT_RADIUS),
        west,
        east,
        world.layout(),
    );
    assert!(!result.is_legal());
}

#[test]
fn surface_route_around_support_remains_possible() {
    let mut world = layout_world();
    let building_id = activate_fixture(
        &mut world,
        one_region_doorless_navigation_blueprint(),
        pos(80.0, 80.0),
    );
    let north = local_xz_to_world(&world, building_id, Vec2::new(-2.0, 8.0));
    let south = local_xz_to_world(&world, building_id, Vec2::new(-2.0, -2.0));
    let building_catalog = BuildingCatalog::default();
    let doodad = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let result = query_navigation_segment_legality(
        &world,
        world.space_registry(),
        pass_catalogs(&doodad, &building_catalog, &footprint),
        NavigationConfig::default(),
        SpaceId::SURFACE,
        nav_agent(ROBOT_RADIUS),
        north,
        south,
        world.layout(),
    );
    assert!(result.is_legal());
}

#[test]
fn entrance_corridor_allows_surface_approach() {
    let mut world = layout_world();
    let building_id = activate_fixture(
        &mut world,
        one_region_doorless_navigation_blueprint(),
        pos(80.0, 80.0),
    );
    let portal = world
        .space_registry()
        .portals()
        .map(|(_, portal)| portal)
        .find(|portal| portal.portal_type == crate::world::PortalType::ExteriorEntrance)
        .expect("entrance");
    let threshold = portal.entrance_threshold_global_xz.unwrap();
    let landing = portal.to_position.to_global(world.layout()).xz();
    let outward = (threshold - landing).normalize();
    let approach = threshold + outward * 1.5;
    let approach_pos =
        WorldPosition::from_global(Vec3::new(approach.x, 0.0, approach.y), world.layout());
    let result = query_navigation_point_legality(
        &world,
        pass_catalogs(
            &DoodadCatalog::default(),
            &BuildingCatalog::default(),
            &FootprintCatalog::default(),
        ),
        approach_pos,
        agent(ROBOT_RADIUS),
        SpaceId::SURFACE,
    );
    assert!(matches!(result, PassabilityResult::Passable { .. }));
}

#[test]
fn surface_pathfinding_routes_around_support() {
    let mut world = layout_world();
    let building_id = activate_fixture(
        &mut world,
        one_region_doorless_navigation_blueprint(),
        pos(80.0, 80.0),
    );
    let west = local_xz_to_world(&world, building_id, Vec2::new(-3.0, 3.0));
    let east = local_xz_to_world(&world, building_id, Vec2::new(11.0, 3.0));
    let building_catalog = BuildingCatalog::default();
    let doodad = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let catalogs = pass_catalogs(&doodad, &building_catalog, &footprint);
    let path = find_path_with_spaces(
        &world,
        catalogs,
        &NavigationConfig::default(),
        ROBOT_RADIUS,
        MAX_SLOPE,
        west,
        east,
        SpaceId::SURFACE,
        SpaceId::SURFACE,
        None,
    )
    .expect("surface path around support");
    let under = local_xz_to_world(&world, building_id, Vec2::new(4.0, 3.0));
    for waypoint in &path.waypoints {
        let result = query_navigation_point_legality(
            &world,
            catalogs,
            waypoint.position,
            agent(ROBOT_RADIUS),
            SpaceId::SURFACE,
        );
        if waypoint
            .position
            .to_global(world.layout())
            .xz()
            .distance(under.to_global(world.layout()).xz())
            < 0.5
        {
            continue;
        }
        assert!(
            !matches!(
                result,
                PassabilityResult::Blocked {
                    reason: PassabilityBlockReason::BlueprintSupport,
                    ..
                }
            ),
            "waypoint under support: {:?}",
            waypoint.position
        );
    }
}

#[test]
fn wide_entrance_off_center_corridor_approach_allowed() {
    let mut world = layout_world();
    let building_id = activate_fixture(&mut world, wide_entrance_blueprint(), pos(120.0, 120.0));
    let portal = world
        .space_registry()
        .portals()
        .map(|(_, portal)| portal)
        .find(|portal| portal.portal_type == crate::world::PortalType::ExteriorEntrance)
        .expect("entrance");
    let threshold = portal.entrance_threshold_global_xz.unwrap();
    let landing = portal.to_position.to_global(world.layout()).xz();
    let outward = (threshold - landing).normalize();
    let edge = portal.entrance_owning_edge_index.unwrap() as usize;
    let runtime = world
        .building_navigation_runtime()
        .get(building_id)
        .unwrap();
    let polygon = &runtime.regions[0].world_outline_xz;
    let a = polygon[edge];
    let b = polygon[(edge + 1) % polygon.len()];
    let edge_dir = (b - a).normalize();
    let lateral = threshold + edge_dir * 1.2 + outward * 1.0;
    let approach_pos =
        WorldPosition::from_global(Vec3::new(lateral.x, 0.0, lateral.y), world.layout());
    let result = query_navigation_point_legality(
        &world,
        pass_catalogs(
            &DoodadCatalog::default(),
            &BuildingCatalog::default(),
            &FootprintCatalog::default(),
        ),
        approach_pos,
        agent(ROBOT_RADIUS),
        SpaceId::SURFACE,
    );
    assert!(matches!(result, PassabilityResult::Passable { .. }));
}

#[test]
fn beside_corridor_inside_support_blocked() {
    let mut world = layout_world();
    let building_id = activate_fixture(
        &mut world,
        one_region_doorless_navigation_blueprint(),
        pos(80.0, 80.0),
    );
    let beside = local_xz_to_world(&world, building_id, Vec2::new(6.5, 3.0));
    let result = query_navigation_point_legality(
        &world,
        pass_catalogs(
            &DoodadCatalog::default(),
            &BuildingCatalog::default(),
            &FootprintCatalog::default(),
        ),
        beside,
        agent(ROBOT_RADIUS),
        SpaceId::SURFACE,
    );
    assert!(matches!(
        result,
        PassabilityResult::Blocked {
            reason: PassabilityBlockReason::BlueprintSupport,
            ..
        }
    ));
}

#[test]
fn closed_entrance_removes_surface_corridor() {
    let mut world = layout_world();
    let building_id = activate_fixture(
        &mut world,
        one_region_doorless_navigation_blueprint(),
        pos(80.0, 80.0),
    );
    let portal_id = world
        .space_registry()
        .portals()
        .find(|(_, portal)| portal.portal_type == crate::world::PortalType::ExteriorEntrance)
        .map(|(id, _)| *id)
        .expect("portal");
    world
        .space_registry_mut()
        .get_portal_mut(portal_id)
        .expect("portal")
        .enabled = false;
    let approach_on_edge = local_xz_to_world(&world, building_id, Vec2::new(4.0, 0.0));
    let result = query_navigation_point_legality(
        &world,
        pass_catalogs(
            &DoodadCatalog::default(),
            &BuildingCatalog::default(),
            &FootprintCatalog::default(),
        ),
        approach_on_edge,
        agent(ROBOT_RADIUS),
        SpaceId::SURFACE,
    );
    assert!(matches!(
        result,
        PassabilityResult::Blocked {
            reason: PassabilityBlockReason::BlueprintSupport,
            ..
        }
    ));
}

#[test]
fn too_large_agent_gets_no_entrance_corridor() {
    let mut world = layout_world();
    let building_id = activate_fixture(&mut world, narrow_entrance_blueprint(), pos(90.0, 90.0));
    let inside = local_xz_to_world(&world, building_id, Vec2::new(4.0, 0.5));
    let result = query_navigation_point_legality(
        &world,
        pass_catalogs(
            &DoodadCatalog::default(),
            &BuildingCatalog::default(),
            &FootprintCatalog::default(),
        ),
        inside,
        agent(ROBOT_RADIUS),
        SpaceId::SURFACE,
    );
    assert!(matches!(
        result,
        PassabilityResult::Blocked {
            reason: PassabilityBlockReason::BlueprintSupport,
            ..
        }
    ));
}

#[test]
fn ghost_building_has_no_support_exclusion() {
    let building_catalog = BuildingCatalog::default();
    let doodad = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let mut world = layout_world();
    create_building(
        &building_catalog,
        &mut world,
        &BuildingDefinitionId::new("hut"),
        pos(50.0, 50.0),
        Quat::IDENTITY,
        crate::world::BuildingSource::Authored,
        BuildingOwnership::neutral(),
        None,
    )
    .unwrap();
    let center = world
        .get_building(world.sorted_building_ids()[0])
        .unwrap()
        .placement
        .position;
    let result = query_navigation_point_legality(
        &world,
        pass_catalogs(&doodad, &building_catalog, &footprint),
        center,
        agent(ROBOT_RADIUS),
        SpaceId::SURFACE,
    );
    assert!(matches!(result, PassabilityResult::Passable { .. }));
}

#[test]
fn concave_notch_outside_region_remains_surface() {
    let mut world = layout_world();
    let building_id = activate_fixture(&mut world, concave_region_blueprint(), pos(140.0, 140.0));
    let notch = local_xz_to_world(&world, building_id, Vec2::new(3.0, 10.0));
    let result = query_navigation_point_legality(
        &world,
        pass_catalogs(
            &DoodadCatalog::default(),
            &BuildingCatalog::default(),
            &FootprintCatalog::default(),
        ),
        notch,
        agent(ROBOT_RADIUS),
        SpaceId::SURFACE,
    );
    assert!(matches!(result, PassabilityResult::Passable { .. }));
    let inside = local_xz_to_world(&world, building_id, Vec2::new(3.0, 3.0));
    let inside_result = query_navigation_point_legality(
        &world,
        pass_catalogs(
            &DoodadCatalog::default(),
            &BuildingCatalog::default(),
            &FootprintCatalog::default(),
        ),
        inside,
        agent(ROBOT_RADIUS),
        SpaceId::SURFACE,
    );
    assert!(matches!(
        inside_result,
        PassabilityResult::Blocked {
            reason: PassabilityBlockReason::BlueprintSupport,
            ..
        }
    ));
}

#[test]
fn disjoint_regions_do_not_bridge_surface_gap() {
    let mut world = layout_world();
    let building_id = activate_fixture(
        &mut world,
        two_room_hut_navigation_blueprint(),
        pos(160.0, 160.0),
    );
    let gap = local_xz_to_world(&world, building_id, Vec2::new(6.2, 2.0));
    let result = query_navigation_point_legality(
        &world,
        pass_catalogs(
            &DoodadCatalog::default(),
            &BuildingCatalog::default(),
            &FootprintCatalog::default(),
        ),
        gap,
        agent(ROBOT_RADIUS),
        SpaceId::SURFACE,
    );
    assert!(matches!(result, PassabilityResult::Passable { .. }));
}

#[test]
fn hut_nav_surface_under_projection_blocked() {
    let building_catalog = imported_hut_catalog();
    let nav_catalog = persisted_nav_catalog();
    let mut world = layout_world();
    let building_id = activate_hut(
        &mut world,
        &building_catalog,
        &nav_catalog,
        pos(200.0, 200.0),
    );
    let runtime = world
        .building_navigation_runtime()
        .get(building_id)
        .unwrap();
    let centroid = runtime.regions[0]
        .world_outline_xz
        .iter()
        .fold(Vec2::ZERO, |acc, v| acc + *v)
        / runtime.regions[0].world_outline_xz.len() as f32;
    let under = WorldPosition::from_global(Vec3::new(centroid.x, 0.0, centroid.y), world.layout());
    let result = query_navigation_point_legality(
        &world,
        pass_catalogs(
            &DoodadCatalog::default(),
            &building_catalog,
            &FootprintCatalog::default(),
        ),
        under,
        agent(ROBOT_RADIUS),
        SpaceId::SURFACE,
    );
    assert!(matches!(
        result,
        PassabilityResult::Blocked {
            reason: PassabilityBlockReason::BlueprintSupport,
            ..
        }
    ));
}

#[test]
fn dual_disjoint_regions_do_not_bridge() {
    let mut world = layout_world();
    let building_id = activate_fixture(
        &mut world,
        dual_doorway_navigation_blueprint(),
        pos(180.0, 180.0),
    );
    let gap = local_xz_to_world(&world, building_id, Vec2::new(5.2, 4.0));
    let result = query_navigation_point_legality(
        &world,
        pass_catalogs(
            &DoodadCatalog::default(),
            &BuildingCatalog::default(),
            &FootprintCatalog::default(),
        ),
        gap,
        agent(ROBOT_RADIUS),
        SpaceId::SURFACE,
    );
    assert!(matches!(result, PassabilityResult::Passable { .. }));
}
