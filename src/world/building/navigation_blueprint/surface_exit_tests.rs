//! NAV-EXIT regressions: Interior→Surface escape waypoint through Entrance access corridor.

use bevy::prelude::*;

use super::adapt::region_space_key;
use super::runtime::point_in_polygon_xz;
use super::surface_support::{
    resolve_surface_entrance_escape_position, surface_entrance_terrain_side_escape_global_xz,
    surface_position_in_entrance_access_corridor,
};
use crate::world::{
    Affiliation, BuildingCatalog, BuildingDefinitionId, BuildingLifecycleState,
    BuildingNavigationBlueprintInstanceOverride, DoodadCatalog, FootprintCatalog,
    InteriorProfileCatalog, NavigationAgent, NavigationConfig, PassabilityAgent,
    PassabilityBlockReason, PassabilityCatalogs, PassabilityResult, PortalType, SpaceId, WorldData,
    WorldPosition, find_path_with_spaces, place_player_building, query_navigation_point_legality,
    query_navigation_segment_legality, set_building_lifecycle_stage,
};

pub(crate) const ROBOT_RADIUS: f32 = 0.68;
const MAX_SLOPE: f32 = 45.0;

pub(crate) fn layout_world() -> WorldData {
    let layout = crate::world::ChunkLayout {
        chunk_size_meters: 256.0,
        units_per_meter: 1.0,
    };
    let mut world = WorldData::new(layout);
    let heightfield = crate::world::Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
    world.insert(
        crate::world::ChunkId::new(crate::world::ChunkCoord::new(0, 0)),
        crate::world::ChunkData::new(heightfield, Vec::new()),
    );
    world
}

pub(crate) fn pos(x: f32, z: f32) -> WorldPosition {
    WorldPosition::new(
        crate::world::ChunkCoord::new(0, 0),
        crate::world::LocalPosition::new(Vec3::new(x, 0.0, z)),
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

pub(crate) fn pass_catalogs<'a>(
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

pub(crate) fn default_catalogs() -> (DoodadCatalog, BuildingCatalog, FootprintCatalog) {
    (
        DoodadCatalog::default(),
        BuildingCatalog::default(),
        FootprintCatalog::default(),
    )
}

pub(crate) fn activate_fixture(
    world: &mut WorldData,
    blueprint: super::definition::BuildingNavigationBlueprint,
    placement: WorldPosition,
) -> crate::world::BuildingId {
    let building_catalog = BuildingCatalog::default();
    let nav_catalog = super::catalog::BuildingNavigationBlueprintCatalog::default();
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
        crate::world::BuildingOwnership::with_affiliation(Affiliation::Player),
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

pub(crate) fn local_xz_to_world(
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

pub(crate) fn surface_local_xz_to_world(
    world: &WorldData,
    building_id: crate::world::BuildingId,
    local_xz: Vec2,
) -> WorldPosition {
    let runtime = world
        .building_navigation_runtime()
        .get(building_id)
        .expect("runtime");
    let layout = world.layout();
    let global = runtime
        .model_transform
        .transform_point(Vec3::new(local_xz.x, 0.0, local_xz.y));
    WorldPosition::from_global(global, layout)
}

pub(crate) fn region_space(
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

pub(crate) fn exit_hut_blueprint() -> super::definition::BuildingNavigationBlueprint {
    use super::definition::{
        BuildingNavigationBlueprint, NavigationEntranceDefinition, NavigationFloorDefinition,
        NavigationPolygon2d, NavigationRegionDefinition,
    };
    BuildingNavigationBlueprint::new("exit_hut", "Exit Hut")
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
            key: "exit_test_entrance".to_string(),
            floor_key: "ground".to_string(),
            region_key: Some("main".to_string()),
            local_position_xz: [4.0, 0.0],
            radius_meters: 1.5,
            interior_spawn_local: [4.0, 0.0, 1.5],
            bidirectional: true,
            door_key: None,
        }])
}

pub(crate) fn entrance_portal_for_building_key(
    world: &WorldData,
    building_id: crate::world::BuildingId,
    entrance_key: &str,
) -> crate::world::PortalRecord {
    let portal_id = *world
        .building_navigation_runtime()
        .get(building_id)
        .expect("runtime")
        .portal_keys
        .get(entrance_key)
        .unwrap_or_else(|| panic!("entrance key `{entrance_key}`"));
    world
        .space_registry()
        .get_portal(portal_id)
        .unwrap_or_else(|| panic!("portal `{entrance_key}`"))
        .clone()
}

/// Resolve the building's enabled [`PortalType::ExteriorEntrance`] portal for tests.
pub(crate) fn exterior_entrance_portal_for_building(
    world: &WorldData,
    building_id: crate::world::BuildingId,
) -> crate::world::PortalRecord {
    let runtime = world
        .building_navigation_runtime()
        .get(building_id)
        .expect("runtime");
    for portal_id in runtime.portal_keys.values() {
        let Some(portal) = world.space_registry().get_portal(*portal_id) else {
            continue;
        };
        if portal.portal_type == PortalType::ExteriorEntrance
            && portal.owning_building_id == Some(building_id)
        {
            return portal.clone();
        }
    }
    panic!(
        "no ExteriorEntrance portal registered for building {:?}",
        building_id
    );
}

pub(crate) fn entrance_portal_for_building(
    world: &WorldData,
    building_id: crate::world::BuildingId,
) -> crate::world::PortalRecord {
    exterior_entrance_portal_for_building(world, building_id)
}

fn assert_surface_waypoints_avoid_support(
    world: &WorldData,
    catalogs: PassabilityCatalogs<'_>,
    path: &crate::world::NavigationPath,
) {
    for waypoint in &path.waypoints {
        if waypoint.space_id != SpaceId::SURFACE || waypoint.portal_id.is_some() {
            continue;
        }
        let result = query_navigation_point_legality(
            world,
            catalogs,
            waypoint.position,
            agent(ROBOT_RADIUS),
            SpaceId::SURFACE,
        );
        assert!(
            !matches!(
                result,
                PassabilityResult::Blocked {
                    reason: PassabilityBlockReason::BlueprintSupport,
                    ..
                }
            ),
            "surface waypoint blocked by support: {:?}",
            waypoint.position
        );
    }
}

fn escape_in_path(
    world: &WorldData,
    portal: &crate::world::PortalRecord,
    path: &crate::world::NavigationPath,
) -> bool {
    let escape = resolve_surface_entrance_escape_position(
        world,
        world.space_registry(),
        portal,
        ROBOT_RADIUS,
    )
    .expect("escape position");
    let escape_xz = escape.to_global(world.layout()).xz();
    path.waypoints.iter().any(|waypoint| {
        waypoint.space_id == SpaceId::SURFACE
            && waypoint.portal_id.is_none()
            && waypoint
                .position
                .to_global(world.layout())
                .xz()
                .distance(escape_xz)
                < 1.0
    })
}

fn portal_to_escape_segment_legal(
    world: &WorldData,
    catalogs: PassabilityCatalogs<'_>,
    portal: &crate::world::PortalRecord,
) {
    let layout = world.layout();
    let escape = resolve_surface_entrance_escape_position(
        world,
        world.space_registry(),
        portal,
        ROBOT_RADIUS,
    )
    .expect("escape");
    let portal_surface = portal
        .destination_for_planning(SpaceId::SURFACE, layout, world, world.space_registry())
        .expect("portal surface dest")
        .1;
    let reverse_surface = portal
        .destination_for_planning(portal.to_space, layout, world, world.space_registry())
        .expect("reverse dest")
        .1;
    let start = if portal.from_space == SpaceId::SURFACE {
        reverse_surface
    } else {
        portal_surface
    };
    let result = query_navigation_segment_legality(
        world,
        world.space_registry(),
        catalogs,
        NavigationConfig::default(),
        SpaceId::SURFACE,
        nav_agent(ROBOT_RADIUS),
        start,
        escape,
        layout,
    );
    assert!(result.is_legal(), "portal to escape segment must be legal");
}

#[test]
fn exit_stitched_path_has_no_near_duplicate_escape_at_seam() {
    let mut world = layout_world();
    let building_id = activate_fixture(&mut world, exit_hut_blueprint(), pos(80.0, 80.0));
    let interior = region_space(&world, building_id, "ground", "main");
    let start = local_xz_to_world(&world, building_id, Vec2::new(4.0, 2.0));
    let goal = surface_local_xz_to_world(&world, building_id, Vec2::new(-2.0, 8.0));
    let (doodad, building, footprint) = default_catalogs();
    let path = find_path_with_spaces(
        &world,
        pass_catalogs(&doodad, &building, &footprint),
        &NavigationConfig::default(),
        ROBOT_RADIUS,
        MAX_SLOPE,
        start,
        goal,
        interior,
        SpaceId::SURFACE,
        None,
    )
    .expect("opposite-side path");
    let portal = entrance_portal_for_building(&world, building_id);
    let escape = resolve_surface_entrance_escape_position(
        &world,
        world.space_registry(),
        &portal,
        ROBOT_RADIUS,
    )
    .expect("escape");
    let layout = world.layout();
    let escape_xz = escape.to_global(layout).xz();
    let escape_index = path
        .waypoints
        .iter()
        .position(|waypoint| {
            waypoint.space_id == SpaceId::SURFACE
                && waypoint.portal_id.is_none()
                && waypoint.position.to_global(layout).xz().distance(escape_xz) < 1.0
        })
        .expect("escape waypoint in path");
    assert!(
        escape_index + 1 < path.waypoints.len(),
        "path must continue past escape"
    );
    let next = &path.waypoints[escape_index + 1];
    assert!(
        next.portal_id.is_none(),
        "post-escape waypoint must not be a portal marker"
    );
    assert!(
        crate::world::xz_distance(next.position, escape, layout) > 0.05,
        "seam must not repeat escape within dedupe tolerance"
    );
}

#[test]
fn interior_to_surface_straight_out_succeeds() {
    let mut world = layout_world();
    let building_id = activate_fixture(&mut world, exit_hut_blueprint(), pos(80.0, 80.0));
    let interior = region_space(&world, building_id, "ground", "main");
    let start = local_xz_to_world(&world, building_id, Vec2::new(4.0, 2.0));
    let goal = surface_local_xz_to_world(&world, building_id, Vec2::new(4.0, -2.0));
    let (doodad, building, footprint) = default_catalogs();
    let catalogs = pass_catalogs(&doodad, &building, &footprint);
    let path = find_path_with_spaces(
        &world,
        catalogs,
        &NavigationConfig::default(),
        ROBOT_RADIUS,
        MAX_SLOPE,
        start,
        goal,
        interior,
        SpaceId::SURFACE,
        None,
    )
    .expect("straight-out path");
    let portal = entrance_portal_for_building(&world, building_id);
    assert_eq!(
        path.waypoints
            .iter()
            .filter(|wp| wp.portal_id.is_some())
            .count(),
        1
    );
    assert!(escape_in_path(&world, &portal, &path));
    assert_surface_waypoints_avoid_support(&world, catalogs, &path);
}

#[test]
fn interior_to_surface_opposite_side_succeeds() {
    let mut world = layout_world();
    let building_id = activate_fixture(&mut world, exit_hut_blueprint(), pos(80.0, 80.0));
    let interior = region_space(&world, building_id, "ground", "main");
    let start = local_xz_to_world(&world, building_id, Vec2::new(4.0, 2.0));
    // North-west behind the south entrance; avoids centerline grid deadlock from escape.
    let goal = surface_local_xz_to_world(&world, building_id, Vec2::new(-2.0, 8.0));
    let (doodad, building, footprint) = default_catalogs();
    let catalogs = pass_catalogs(&doodad, &building, &footprint);
    let path = find_path_with_spaces(
        &world,
        catalogs,
        &NavigationConfig::default(),
        ROBOT_RADIUS,
        MAX_SLOPE,
        start,
        goal,
        interior,
        SpaceId::SURFACE,
        None,
    )
    .expect("opposite-side path");
    let portal = entrance_portal_for_building(&world, building_id);
    assert!(escape_in_path(&world, &portal, &path));
    portal_to_escape_segment_legal(&world, catalogs, &portal);
    assert_surface_waypoints_avoid_support(&world, catalogs, &path);
}

#[test]
fn interior_to_surface_side_turn_succeeds() {
    let mut world = layout_world();
    let building_id = activate_fixture(&mut world, exit_hut_blueprint(), pos(80.0, 80.0));
    let interior = region_space(&world, building_id, "ground", "main");
    let start = local_xz_to_world(&world, building_id, Vec2::new(4.0, 2.0));
    let goal = surface_local_xz_to_world(&world, building_id, Vec2::new(-2.0, 3.0));
    let (doodad, building, footprint) = default_catalogs();
    let catalogs = pass_catalogs(&doodad, &building, &footprint);
    let path = find_path_with_spaces(
        &world,
        catalogs,
        &NavigationConfig::default(),
        ROBOT_RADIUS,
        MAX_SLOPE,
        start,
        goal,
        interior,
        SpaceId::SURFACE,
        None,
    )
    .expect("side-turn path");
    let portal = entrance_portal_for_building(&world, building_id);
    assert!(escape_in_path(&world, &portal, &path));
    assert_surface_waypoints_avoid_support(&world, catalogs, &path);
}

#[test]
fn escape_point_outside_support_polygon() {
    let mut world = layout_world();
    let building_id = activate_fixture(&mut world, exit_hut_blueprint(), pos(80.0, 80.0));
    let portal = entrance_portal_for_building(&world, building_id);
    let runtime = world
        .building_navigation_runtime()
        .get(building_id)
        .unwrap();
    let polygon = &runtime.regions[0].world_outline_xz;
    let escape_xz = surface_entrance_terrain_side_escape_global_xz(
        &portal,
        polygon,
        world.layout(),
        ROBOT_RADIUS,
    )
    .expect("escape xz");
    assert!(
        !point_in_polygon_xz(polygon, escape_xz),
        "escape must lie outside support polygon"
    );
    assert!(
        surface_position_in_entrance_access_corridor(
            world.space_registry(),
            world.layout(),
            building_id,
            &runtime.regions[0],
            escape_xz,
            ROBOT_RADIUS,
        ),
        "escape must remain inside access corridor"
    );
}

#[test]
fn closed_entrance_interior_exit_fails() {
    let mut world = layout_world();
    let building_id = activate_fixture(&mut world, exit_hut_blueprint(), pos(80.0, 80.0));
    let portal_id = *world
        .building_navigation_runtime()
        .get(building_id)
        .expect("runtime")
        .portal_keys
        .get("exit_test_entrance")
        .expect("portal key");
    world
        .space_registry_mut()
        .get_portal_mut(portal_id)
        .expect("portal")
        .enabled = false;
    let interior = region_space(&world, building_id, "ground", "main");
    let start = local_xz_to_world(&world, building_id, Vec2::new(4.0, 2.0));
    let goal = surface_local_xz_to_world(&world, building_id, Vec2::new(4.0, 8.0));
    let (doodad, building, footprint) = default_catalogs();
    let result = find_path_with_spaces(
        &world,
        pass_catalogs(&doodad, &building, &footprint),
        &NavigationConfig::default(),
        ROBOT_RADIUS,
        MAX_SLOPE,
        start,
        goal,
        interior,
        SpaceId::SURFACE,
        None,
    );
    assert!(
        result.is_err(),
        "closed entrance must not produce exit path"
    );
}

#[test]
fn too_large_agent_interior_exit_fails() {
    let mut world = layout_world();
    let building_id = activate_fixture(&mut world, narrow_entrance_blueprint(), pos(90.0, 90.0));
    let interior = region_space(&world, building_id, "ground", "main");
    let start = local_xz_to_world(&world, building_id, Vec2::new(4.0, 0.5));
    let goal = surface_local_xz_to_world(&world, building_id, Vec2::new(4.0, -2.0));
    let (doodad, building, footprint) = default_catalogs();
    let result = find_path_with_spaces(
        &world,
        pass_catalogs(&doodad, &building, &footprint),
        &NavigationConfig::default(),
        ROBOT_RADIUS,
        MAX_SLOPE,
        start,
        goal,
        interior,
        SpaceId::SURFACE,
        None,
    );
    assert!(
        result.is_err(),
        "too-large agent must not route through narrow entrance"
    );
}

fn narrow_entrance_blueprint() -> super::definition::BuildingNavigationBlueprint {
    use super::definition::{
        BuildingNavigationBlueprint, NavigationEntranceDefinition, NavigationFloorDefinition,
        NavigationPolygon2d, NavigationRegionDefinition,
    };
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
            key: "narrow_test_entrance".to_string(),
            floor_key: "ground".to_string(),
            region_key: Some("main".to_string()),
            local_position_xz: [4.0, 0.0],
            radius_meters: 0.5,
            interior_spawn_local: [4.0, 0.0, 1.5],
            bidirectional: true,
            door_key: None,
        }])
}
