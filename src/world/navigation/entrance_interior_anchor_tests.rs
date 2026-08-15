//! IN-11gI-C entrance interior planning anchor regressions.

use bevy::prelude::*;

use super::grid::{NavigationAgent, NavigationConfig};
use super::legality::query_navigation_point_legality;
use crate::world::occupancy::{PassabilityAgent, PassabilityCatalogs, PassabilityResult};
use crate::world::{
    Affiliation, BuildingCatalog, BuildingCategoryCatalog, BuildingDefinition,
    BuildingDefinitionId, BuildingLifecycleState, BuildingNavigationBlueprint,
    BuildingNavigationBlueprintCatalog, BuildingOwnership, BuildingRenderKey, ChunkCoord,
    ChunkLayout, DoodadCatalog, FootprintCatalog, InteriorProfileCatalog,
    NavigationEntranceDefinition, NavigationFloorDefinition, NavigationPolygon2d,
    NavigationRegionDefinition, OccupancyCatalogs, SpaceId, WorldData, WorldPosition,
    find_path_with_spaces, min_edge_clearance_meters, place_player_building,
    set_building_lifecycle_stage,
};

fn layout_world() -> WorldData {
    let layout = ChunkLayout {
        chunk_size_meters: 256.0,
        units_per_meter: 1.0,
    };
    let mut world = WorldData::new(layout);
    let heightfield = crate::world::Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
    world.insert(
        crate::world::ChunkId::new(ChunkCoord::new(0, 0)),
        crate::world::ChunkData::new(heightfield, Vec::new()),
    );
    world
}

fn pos(x: f32, z: f32) -> WorldPosition {
    WorldPosition::new(
        ChunkCoord::new(0, 0),
        crate::world::LocalPosition::new(Vec3::new(x, 0.0, z)),
    )
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

fn activate_hut(world: &mut WorldData, placement: WorldPosition) -> crate::world::BuildingId {
    let building_catalog = imported_hut_catalog();
    let nav_catalog = persisted_nav_catalog();
    let doodad_catalog = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let occupancy = OccupancyCatalogs {
        building: &building_catalog,
        doodad: &doodad_catalog,
        footprint: &footprint,
    };
    let interior = InteriorProfileCatalog::default();
    let building_id = place_player_building(
        &building_catalog,
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
        &building_catalog,
        &interior,
        &doodad_catalog,
        occupancy,
        Some(&nav_catalog),
        building_id,
        BuildingLifecycleState::Complete,
        1.0,
    )
    .expect("complete hut");
    building_id
}

fn hut_entrance_portal(
    world: &WorldData,
    building_id: crate::world::BuildingId,
) -> crate::world::PortalRecord {
    let runtime = world
        .building_navigation_runtime()
        .get(building_id)
        .expect("runtime");
    let portal_id = *runtime.portal_keys.get("entrance").expect("entrance key");
    world
        .space_registry()
        .get_portal(portal_id)
        .expect("portal")
        .clone()
}

fn passability_catalogs<'a>(
    building_catalog: &'a BuildingCatalog,
    doodad_catalog: &'a DoodadCatalog,
    footprint: &'a FootprintCatalog,
) -> PassabilityCatalogs<'a> {
    PassabilityCatalogs {
        doodad: doodad_catalog,
        building: building_catalog,
        footprint,
    }
}

struct CatalogHarness {
    building_catalog: BuildingCatalog,
    doodad_catalog: DoodadCatalog,
    footprint: FootprintCatalog,
}

impl CatalogHarness {
    fn imported_hut() -> Self {
        Self {
            building_catalog: imported_hut_catalog(),
            doodad_catalog: DoodadCatalog::default(),
            footprint: FootprintCatalog::default(),
        }
    }

    fn pass(&self) -> PassabilityCatalogs<'_> {
        passability_catalogs(
            &self.building_catalog,
            &self.doodad_catalog,
            &self.footprint,
        )
    }
}

#[test]
fn small_agent_uses_authored_hut_landing() {
    let mut world = layout_world();
    let building_id = activate_hut(&mut world, pos(80.0, 80.0));
    let portal = hut_entrance_portal(&world, building_id);
    let interior_space = portal.to_space;
    let harness = CatalogHarness::imported_hut();
    let catalogs = harness.pass();
    let agent = NavigationAgent {
        radius_meters: 0.25,
        max_slope_degrees: 45.0,
    };
    let resolved = super::entrance_interior_anchor::resolve_entrance_interior_planning_anchor(
        &world,
        world.space_registry(),
        catalogs,
        &portal,
        interior_space,
        world.layout(),
        NavigationConfig::default(),
        agent,
    )
    .expect("small agent anchor");
    let authored = portal.to_position;
    assert!(
        resolved
            .to_global(world.layout())
            .xz()
            .distance(authored.to_global(world.layout()).xz())
            < 0.05,
        "small agent should keep authored landing"
    );
}

#[test]
fn robot_agent_gets_farther_inward_hut_anchor() {
    let mut world = layout_world();
    let building_id = activate_hut(&mut world, pos(80.0, 80.0));
    let portal = hut_entrance_portal(&world, building_id);
    let interior_space = portal.to_space;
    let region = world
        .building_navigation_runtime()
        .region_for_space(interior_space)
        .expect("region");
    let harness = CatalogHarness::imported_hut();
    let catalogs = harness.pass();
    let robot_radius = 0.68;
    let agent = NavigationAgent {
        radius_meters: robot_radius,
        max_slope_degrees: 45.0,
    };
    let authored = portal.to_position;
    let passability_agent = PassabilityAgent::from(agent);
    let authored_legal = matches!(
        query_navigation_point_legality(
            &world,
            catalogs,
            authored,
            passability_agent,
            interior_space,
        ),
        PassabilityResult::Passable { .. }
    );

    let resolved = super::entrance_interior_anchor::resolve_entrance_interior_planning_anchor(
        &world,
        world.space_registry(),
        catalogs,
        &portal,
        interior_space,
        world.layout(),
        NavigationConfig::default(),
        agent,
    )
    .expect("robot anchor");
    if authored_legal {
        assert!(
            resolved
                .to_global(world.layout())
                .xz()
                .distance(authored.to_global(world.layout()).xz())
                < 0.05,
            "when authored landing is legal, robot anchor should keep authored landing"
        );
    } else {
        let resolved_clearance = min_edge_clearance_meters(
            resolved.to_global(world.layout()).xz(),
            &region.world_outline_xz,
        );
        assert!(
            resolved_clearance >= robot_radius,
            "resolved anchor clearance {resolved_clearance} must satisfy robot radius"
        );
        assert!(
            resolved
                .to_global(world.layout())
                .xz()
                .distance(authored.to_global(world.layout()).xz())
                > 0.05,
            "when authored landing is illegal, robot anchor must be farther inward"
        );
    }
    assert!(matches!(
        query_navigation_point_legality(
            &world,
            catalogs,
            resolved,
            passability_agent,
            interior_space,
        ),
        PassabilityResult::Passable { .. }
    ));
}

#[test]
fn oversized_agent_gets_no_hut_anchor() {
    let mut world = layout_world();
    let building_id = activate_hut(&mut world, pos(80.0, 80.0));
    let portal = hut_entrance_portal(&world, building_id);
    let harness = CatalogHarness::imported_hut();
    let catalogs = harness.pass();
    let agent = NavigationAgent {
        radius_meters: 5.0,
        max_slope_degrees: 45.0,
    };
    let resolved = super::entrance_interior_anchor::resolve_entrance_interior_planning_anchor(
        &world,
        world.space_registry(),
        catalogs,
        &portal,
        portal.to_space,
        world.layout(),
        NavigationConfig::default(),
        agent,
    );
    assert!(
        resolved.is_none(),
        "oversized agent must not get forced anchor"
    );
}

#[test]
fn real_hut_cross_space_route_no_longer_start_blocked_for_robot() {
    let mut world = layout_world();
    let building_id = activate_hut(&mut world, pos(80.0, 80.0));
    let runtime = world
        .building_navigation_runtime()
        .get(building_id)
        .expect("runtime");
    let goal_space = runtime.regions[0].space_id;
    let floor_y = world
        .space_registry()
        .get_space(goal_space)
        .expect("space")
        .floor_y_global;
    let interior_global = runtime
        .model_transform
        .transform_point(Vec3::new(0.0, 0.0, 1.0));
    let goal = WorldPosition::from_global(
        Vec3::new(interior_global.x, floor_y, interior_global.z),
        world.layout(),
    );
    let entrance_global =
        runtime
            .model_transform
            .transform_point(Vec3::new(0.058_471_68, 0.0, -6.0));
    let start = WorldPosition::from_global(
        Vec3::new(entrance_global.x, 0.0, entrance_global.z),
        world.layout(),
    );

    let building_catalog = imported_hut_catalog();
    let doodad_catalog = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let catalogs = PassabilityCatalogs {
        doodad: &doodad_catalog,
        building: &building_catalog,
        footprint: &footprint,
    };
    let robot_radius = 0.68;
    let path = find_path_with_spaces(
        &world,
        catalogs,
        &NavigationConfig::default(),
        robot_radius,
        45.0,
        start,
        goal,
        SpaceId::SURFACE,
        goal_space,
        Some(crate::world::UnitOwnership::player_default()),
    )
    .expect("cross-space route must succeed for robot after anchor resolution");
    assert!(
        path.waypoints.iter().any(|wp| wp.portal_id.is_some()),
        "route must include portal transition"
    );
    assert!(
        path.waypoints
            .iter()
            .any(|wp| wp.portal_interior_destination.is_some()),
        "portal waypoint must carry clearance-safe interior destination"
    );
    assert_eq!(path.waypoints.last().expect("last").space_id, goal_space);
}
