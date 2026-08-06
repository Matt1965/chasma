//! IN-07b multi-region runtime navigation end-to-end tests.

use bevy::prelude::*;

use super::adapt::region_space_key;
use super::fixtures::{
    corridor_hut_navigation_blueprint, dual_doorway_navigation_blueprint,
    two_floor_two_room_navigation_blueprint, two_room_hut_navigation_blueprint,
};
use super::runtime::{
    interior_position_walkable, resolve_navigation_space_at_position,
    resolve_navigation_start_space,
};
use crate::world::unit::{UnitOrder, UnitSource, UnitState, create_unit, step_unit_movement};
use crate::world::{
    Affiliation, BuildingCatalog, BuildingDefinitionId, BuildingLifecycleState,
    BuildingNavigationBlueprint, BuildingNavigationBlueprintCatalog,
    BuildingNavigationBlueprintInstanceOverride, BuildingOwnership, ChunkCoord, ChunkLayout,
    DoodadCatalog, FootprintCatalog, NavigationConfig, OccupancyCatalogs, PassabilityCatalogs,
    PortalId, SpaceId, UnitDefinitionId, WorldData, WorldPosition, find_path_with_spaces,
    place_player_building, resolve_pending_unit_orders, set_building_lifecycle_stage,
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

fn occ<'a>(
    building: &'a BuildingCatalog,
    doodad: &'a DoodadCatalog,
    footprint: &'a FootprintCatalog,
) -> OccupancyCatalogs<'a> {
    OccupancyCatalogs {
        building,
        doodad,
        footprint,
    }
}

fn activate_fixture(
    world: &mut WorldData,
    blueprint: BuildingNavigationBlueprint,
    placement: WorldPosition,
    rotation: Quat,
) -> crate::world::BuildingId {
    let building_catalog = BuildingCatalog::default();
    let doodad_catalog = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let occupancy = occ(&building_catalog, &doodad_catalog, &footprint);
    let interior = crate::world::InteriorProfileCatalog::default();
    let nav_catalog = BuildingNavigationBlueprintCatalog::default();

    let id = place_player_building(
        &building_catalog,
        world,
        &BuildingDefinitionId::new("hut"),
        placement,
        rotation,
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
    let key = region_space_key(floor_key, region_key);
    *runtime.space_keys.get(&key).unwrap_or_else(|| {
        panic!("missing space key `{key}`");
    })
}

fn portal_by_key(
    world: &WorldData,
    building_id: crate::world::BuildingId,
    portal_key: &str,
) -> PortalId {
    let runtime = world
        .building_navigation_runtime()
        .get(building_id)
        .expect("runtime");
    *runtime
        .portal_keys
        .get(portal_key)
        .unwrap_or_else(|| panic!("missing portal `{portal_key}`"))
}

fn issue_move(
    world: &mut WorldData,
    unit_catalog: &crate::world::UnitCatalog,
    catalogs: PassabilityCatalogs<'_>,
    unit_id: crate::world::UnitId,
    target: WorldPosition,
) {
    world
        .command_buffer_mut()
        .enqueue(unit_id, UnitOrder::MoveTo { target });
    let report =
        resolve_pending_unit_orders(world, unit_catalog, catalogs, &NavigationConfig::default());
    assert_eq!(report.resolved, 1);
}

fn run_ticks(
    world: &mut WorldData,
    unit_catalog: &crate::world::UnitCatalog,
    catalogs: PassabilityCatalogs<'_>,
    unit_id: crate::world::UnitId,
    ticks: usize,
) {
    for _ in 0..ticks {
        let _ = step_unit_movement(world, unit_catalog, catalogs, unit_id, 0.25);
    }
}

#[test]
fn two_room_hut_registers_two_region_spaces_and_one_connection() {
    let mut world = layout_world();
    let building_id = activate_fixture(
        &mut world,
        two_room_hut_navigation_blueprint(),
        pos(80.0, 80.0),
        Quat::IDENTITY,
    );
    let runtime = world
        .building_navigation_runtime()
        .get(building_id)
        .expect("runtime");
    assert_eq!(runtime.regions.len(), 2);
    assert_eq!(runtime.portal_keys.len(), 2); // entrance + hall_door
    assert!(runtime.portal_keys.contains_key("hall_door"));
    assert!(runtime.space_keys.contains_key("ground/room_a"));
    assert!(runtime.space_keys.contains_key("ground/room_b"));
    assert!(!runtime.space_keys.contains_key("ground"));
}

#[test]
fn two_room_position_resolution_and_wall_band() {
    let mut world = layout_world();
    let building_id = activate_fixture(
        &mut world,
        two_room_hut_navigation_blueprint(),
        pos(80.0, 80.0),
        Quat::IDENTITY,
    );
    let room_a = region_space(&world, building_id, "ground", "room_a");
    let room_b = region_space(&world, building_id, "ground", "room_b");
    let start = local_xz_to_world(&world, building_id, Vec2::new(1.5, 2.0), 0.0);
    let goal = local_xz_to_world(&world, building_id, Vec2::new(11.0, 2.0), 0.0);
    let wall = local_xz_to_world(&world, building_id, Vec2::new(6.2, 2.0), 0.0);

    assert_eq!(
        resolve_navigation_space_at_position(
            world.building_navigation_runtime(),
            world.space_registry(),
            world.layout(),
            start,
        ),
        room_a
    );
    assert_eq!(
        resolve_navigation_space_at_position(
            world.building_navigation_runtime(),
            world.space_registry(),
            world.layout(),
            goal,
        ),
        room_b
    );
    assert_eq!(
        resolve_navigation_space_at_position(
            world.building_navigation_runtime(),
            world.space_registry(),
            world.layout(),
            wall,
        ),
        SpaceId::SURFACE
    );
    assert!(!interior_position_walkable(
        world.building_navigation_runtime(),
        world.space_registry(),
        world.layout(),
        wall,
        room_a,
    ));
}

#[test]
fn two_room_route_uses_hall_door_portal() {
    let building_catalog = BuildingCatalog::default();
    let doodad_catalog = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let catalogs = PassabilityCatalogs {
        doodad: &doodad_catalog,
        building: &building_catalog,
        footprint: &footprint,
    };
    let mut world = layout_world();
    let building_id = activate_fixture(
        &mut world,
        two_room_hut_navigation_blueprint(),
        pos(80.0, 80.0),
        Quat::IDENTITY,
    );
    let room_a = region_space(&world, building_id, "ground", "room_a");
    let room_b = region_space(&world, building_id, "ground", "room_b");
    let start = local_xz_to_world(&world, building_id, Vec2::new(1.5, 2.0), 0.0);
    let goal = local_xz_to_world(&world, building_id, Vec2::new(11.0, 2.0), 0.0);
    let hall_door = portal_by_key(&world, building_id, "hall_door");

    let path = find_path_with_spaces(
        &world,
        catalogs,
        &NavigationConfig::default(),
        0.5,
        45.0,
        start,
        goal,
        room_a,
        room_b,
        None,
    )
    .expect("room A to room B path");
    let portal_waypoints: Vec<_> = path
        .waypoints
        .iter()
        .filter(|wp| wp.portal_id.is_some())
        .collect();
    assert_eq!(portal_waypoints.len(), 1);
    assert_eq!(portal_waypoints[0].portal_id, Some(hall_door));

    let reverse = find_path_with_spaces(
        &world,
        catalogs,
        &NavigationConfig::default(),
        0.6,
        45.0,
        goal,
        start,
        room_b,
        room_a,
        Some(crate::world::UnitOwnership::player_default()),
    )
    .expect("room B to room A path");
    assert_eq!(
        reverse
            .waypoints
            .iter()
            .filter_map(|wp| wp.portal_id)
            .collect::<Vec<_>>(),
        vec![hall_door]
    );
}

fn run_until<F>(
    world: &mut WorldData,
    unit_catalog: &crate::world::UnitCatalog,
    catalogs: PassabilityCatalogs<'_>,
    unit_id: crate::world::UnitId,
    max_ticks: usize,
    mut predicate: F,
) -> bool
where
    F: FnMut(&WorldData, crate::world::UnitId) -> bool,
{
    for _ in 0..max_ticks {
        if predicate(world, unit_id) {
            return true;
        }
        let _ = step_unit_movement(world, unit_catalog, catalogs, unit_id, 0.25);
    }
    predicate(world, unit_id)
}

#[test]
fn two_room_unit_crosses_hall_door_both_ways() {
    let building_catalog = BuildingCatalog::default();
    let doodad_catalog = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let unit_catalog = crate::world::UnitCatalog::default();
    let catalogs = PassabilityCatalogs {
        doodad: &doodad_catalog,
        building: &building_catalog,
        footprint: &footprint,
    };
    let mut world = layout_world();
    let building_id = activate_fixture(
        &mut world,
        two_room_hut_navigation_blueprint(),
        pos(80.0, 80.0),
        Quat::IDENTITY,
    );
    let room_a = region_space(&world, building_id, "ground", "room_a");
    let room_b = region_space(&world, building_id, "ground", "room_b");
    let start = local_xz_to_world(&world, building_id, Vec2::new(1.5, 2.0), 0.0);
    let goal = local_xz_to_world(&world, building_id, Vec2::new(11.0, 2.0), 0.0);

    let unit_id = create_unit(
        &unit_catalog,
        &mut world,
        &UnitDefinitionId::new("wolf"),
        start,
        UnitSource::Authored,
    )
    .unwrap()
    .id;
    world
        .set_unit_current_space(unit_id, room_a)
        .expect("set space");

    issue_move(&mut world, &unit_catalog, catalogs, unit_id, goal);
    let layout = world.layout();
    let goal_xz = goal.to_global(layout).xz();
    let hall_door = portal_by_key(&world, building_id, "hall_door");
    assert!(
        run_until(
            &mut world,
            &unit_catalog,
            catalogs,
            unit_id,
            400,
            |world, unit_id| {
                let record = world.get_unit(unit_id).unwrap();
                record.current_space_id == room_b
                    && record
                        .placement
                        .position
                        .to_global(layout)
                        .xz()
                        .distance(goal_xz)
                        < 2.5
            }
        ),
        "unit should reach room B goal"
    );

    issue_move(&mut world, &unit_catalog, catalogs, unit_id, start);
    let start_xz = start.to_global(layout).xz();
    let mut portal_consumed = false;
    assert!(
        run_until(
            &mut world,
            &unit_catalog,
            catalogs,
            unit_id,
            400,
            |world, unit_id| {
                let record = world.get_unit(unit_id).unwrap();
                if let UnitState::Moving {
                    ref path,
                    waypoint_index,
                    ..
                } = record.state
                {
                    if let Some(pi) = path
                        .waypoints
                        .iter()
                        .position(|wp| wp.portal_id == Some(hall_door))
                    {
                        if waypoint_index > pi {
                            portal_consumed = true;
                        }
                    }
                }
                record.current_space_id == room_a
                    && record
                        .placement
                        .position
                        .to_global(layout)
                        .xz()
                        .distance(start_xz)
                        < 2.5
            }
        ),
        "unit should return to room A"
    );
    assert!(
        portal_consumed,
        "hall_door portal should be consumed on return"
    );
}

#[test]
fn disabled_hall_door_blocks_route() {
    let building_catalog = BuildingCatalog::default();
    let doodad_catalog = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let catalogs = PassabilityCatalogs {
        doodad: &doodad_catalog,
        building: &building_catalog,
        footprint: &footprint,
    };
    let mut world = layout_world();
    let building_id = activate_fixture(
        &mut world,
        two_room_hut_navigation_blueprint(),
        pos(80.0, 80.0),
        Quat::IDENTITY,
    );
    let room_a = region_space(&world, building_id, "ground", "room_a");
    let room_b = region_space(&world, building_id, "ground", "room_b");
    let start = local_xz_to_world(&world, building_id, Vec2::new(1.5, 2.0), 0.0);
    let goal = local_xz_to_world(&world, building_id, Vec2::new(11.0, 2.0), 0.0);
    let hall_door = portal_by_key(&world, building_id, "hall_door");
    world
        .space_registry_mut()
        .set_portal_enabled(hall_door, false);

    assert!(
        find_path_with_spaces(
            &world,
            catalogs,
            &NavigationConfig::default(),
            0.5,
            45.0,
            start,
            goal,
            room_a,
            room_b,
            None,
        )
        .is_err()
    );
}

#[test]
fn corridor_fixture_portal_order_forward_and_reverse() {
    let building_catalog = BuildingCatalog::default();
    let doodad_catalog = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let catalogs = PassabilityCatalogs {
        doodad: &doodad_catalog,
        building: &building_catalog,
        footprint: &footprint,
    };
    let mut world = layout_world();
    let building_id = activate_fixture(
        &mut world,
        corridor_hut_navigation_blueprint(),
        pos(80.0, 80.0),
        Quat::IDENTITY,
    );
    let west = region_space(&world, building_id, "ground", "room_west");
    let east = region_space(&world, building_id, "ground", "room_east");
    let start = local_xz_to_world(&world, building_id, Vec2::new(1.0, 2.0), 0.0);
    let goal = local_xz_to_world(&world, building_id, Vec2::new(15.0, 2.0), 0.0);
    let west_door = portal_by_key(&world, building_id, "west_door");
    let east_door = portal_by_key(&world, building_id, "east_door");

    let forward = find_path_with_spaces(
        &world,
        catalogs,
        &NavigationConfig::default(),
        0.5,
        45.0,
        start,
        goal,
        west,
        east,
        None,
    )
    .expect("forward path");
    let portal_ids: Vec<_> = forward
        .waypoints
        .iter()
        .filter_map(|wp| wp.portal_id)
        .collect();
    assert_eq!(portal_ids, vec![west_door, east_door]);

    let reverse = find_path_with_spaces(
        &world,
        catalogs,
        &NavigationConfig::default(),
        0.5,
        45.0,
        goal,
        start,
        east,
        west,
        None,
    )
    .expect("reverse path");
    let portal_ids: Vec<_> = reverse
        .waypoints
        .iter()
        .filter_map(|wp| wp.portal_id)
        .collect();
    assert_eq!(portal_ids, vec![east_door, west_door]);
}

#[test]
fn dual_doorway_registers_distinct_portals() {
    let mut world = layout_world();
    let building_id = activate_fixture(
        &mut world,
        dual_doorway_navigation_blueprint(),
        pos(80.0, 80.0),
        Quat::IDENTITY,
    );
    let runtime = world
        .building_navigation_runtime()
        .get(building_id)
        .expect("runtime");
    let north = runtime
        .portal_keys
        .get("door_north")
        .copied()
        .expect("north");
    let south = runtime
        .portal_keys
        .get("door_south")
        .copied()
        .expect("south");
    assert_ne!(north, south);
}

#[test]
fn two_floor_fixture_resolves_regions_by_elevation() {
    let mut world = layout_world();
    let building_id = activate_fixture(
        &mut world,
        two_floor_two_room_navigation_blueprint(),
        pos(80.0, 80.0),
        Quat::IDENTITY,
    );
    let ground_entry = region_space(&world, building_id, "ground", "ground_entry");
    let upper_bed = region_space(&world, building_id, "upper", "upper_bed");
    let ground_pos = local_xz_to_world(&world, building_id, Vec2::new(3.0, 3.0), 0.0);
    let upper_pos = local_xz_to_world(&world, building_id, Vec2::new(3.0, 3.0), 4.0);

    assert_eq!(
        resolve_navigation_space_at_position(
            world.building_navigation_runtime(),
            world.space_registry(),
            world.layout(),
            ground_pos,
        ),
        ground_entry
    );
    assert_eq!(
        resolve_navigation_space_at_position(
            world.building_navigation_runtime(),
            world.space_registry(),
            world.layout(),
            upper_pos,
        ),
        upper_bed
    );
}

#[test]
fn tracked_region_authority_preserved_in_two_room_hut() {
    let mut world = layout_world();
    let building_id = activate_fixture(
        &mut world,
        two_room_hut_navigation_blueprint(),
        pos(80.0, 80.0),
        Quat::IDENTITY,
    );
    let room_a = region_space(&world, building_id, "ground", "room_a");
    let pos_a = local_xz_to_world(&world, building_id, Vec2::new(1.5, 2.0), 0.0);
    assert_eq!(
        resolve_navigation_start_space(
            world.building_navigation_runtime(),
            world.space_registry(),
            world.layout(),
            pos_a,
            room_a,
        ),
        room_a
    );
}
