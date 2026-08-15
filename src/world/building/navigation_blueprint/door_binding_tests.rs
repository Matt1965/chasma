//! NAV-DOOR-BINDING: blueprint door_key authority over profile door collision.

use bevy::prelude::*;

use super::fixtures::one_region_doorless_navigation_blueprint;
use super::surface_exit_tests::{
    ROBOT_RADIUS, activate_fixture, default_catalogs, entrance_portal_for_building,
    local_xz_to_world, pass_catalogs, pos, region_space, surface_local_xz_to_world,
};
use crate::world::{
    BuildingNavigationBlueprint, DoorState, NavigationConfig, SpaceId, WorldData,
    find_path_with_spaces, open_door,
};

fn door_controlled_entrance_blueprint() -> BuildingNavigationBlueprint {
    let mut blueprint = one_region_doorless_navigation_blueprint();
    blueprint.entrances[0].door_key = Some("exterior_entrance".to_string());
    blueprint
}

fn exterior_entrance_door_id(
    world: &WorldData,
    building_id: crate::world::BuildingId,
) -> crate::world::DoorId {
    let portal_id = entrance_portal_for_building(world, building_id).id;
    world
        .door_store()
        .door_for_portal_id(portal_id)
        .expect("door-controlled entrance must register a door")
}

#[test]
fn doorless_entrance_ignores_closed_profile_door_with_same_key() {
    let mut world = super::surface_exit_tests::layout_world();
    let building_id = activate_fixture(
        &mut world,
        one_region_doorless_navigation_blueprint(),
        pos(80.0, 80.0),
    );
    let portal = entrance_portal_for_building(&world, building_id);

    assert!(
        portal.enabled,
        "doorless blueprint entrance must stay enabled despite profile door key collision"
    );
    assert!(
        world.door_store().door_for_portal_id(portal.id).is_none(),
        "profile door must not bind to a doorless blueprint entrance"
    );

    let goal_space = region_space(&world, building_id, "ground", "main");
    let start = local_xz_to_world(&world, building_id, Vec2::new(-3.0, 3.0));
    let goal = local_xz_to_world(&world, building_id, Vec2::new(6.0, 4.0));
    let interior_start = local_xz_to_world(&world, building_id, Vec2::new(4.0, 2.0));
    let surface_goal = surface_local_xz_to_world(&world, building_id, Vec2::new(4.0, -2.0));
    let (doodad, building, footprint) = default_catalogs();
    let catalogs = pass_catalogs(&doodad, &building, &footprint);
    let nav_config = NavigationConfig::default();

    find_path_with_spaces(
        &world,
        catalogs,
        &nav_config,
        ROBOT_RADIUS,
        45.0,
        start,
        goal,
        SpaceId::SURFACE,
        goal_space,
        None,
    )
    .expect("surface→interior must resolve for doorless entrance");

    find_path_with_spaces(
        &world,
        catalogs,
        &nav_config,
        ROBOT_RADIUS,
        45.0,
        interior_start,
        surface_goal,
        goal_space,
        SpaceId::SURFACE,
        None,
    )
    .expect("interior→surface must resolve for doorless entrance");
}

#[test]
fn explicit_door_controlled_entrance_blocks_when_closed() {
    let mut world = super::surface_exit_tests::layout_world();
    let building_id = activate_fixture(
        &mut world,
        door_controlled_entrance_blueprint(),
        pos(80.0, 80.0),
    );
    let portal = entrance_portal_for_building(&world, building_id);
    let door_id = exterior_entrance_door_id(&world, building_id);

    assert_eq!(
        world.door_store().get(door_id).unwrap().state,
        DoorState::Closed
    );
    assert!(
        !portal.enabled,
        "closed door must disable door-controlled portal"
    );

    let goal_space = region_space(&world, building_id, "ground", "main");
    let start = local_xz_to_world(&world, building_id, Vec2::new(3.0, -2.0));
    let goal = local_xz_to_world(&world, building_id, Vec2::new(6.0, 4.0));
    let (doodad, building, footprint) = default_catalogs();
    let catalogs = pass_catalogs(&doodad, &building, &footprint);

    assert!(
        find_path_with_spaces(
            &world,
            catalogs,
            &NavigationConfig::default(),
            ROBOT_RADIUS,
            45.0,
            start,
            goal,
            SpaceId::SURFACE,
            goal_space,
            None,
        )
        .is_err(),
        "closed door-controlled entrance must reject traversal planning"
    );
}

#[test]
fn explicit_door_controlled_entrance_allows_when_open() {
    let mut world = super::surface_exit_tests::layout_world();
    let building_id = activate_fixture(
        &mut world,
        door_controlled_entrance_blueprint(),
        pos(80.0, 80.0),
    );
    let portal_id = entrance_portal_for_building(&world, building_id).id;
    let door_id = exterior_entrance_door_id(&world, building_id);
    open_door(&mut world, door_id).expect("open door");

    assert!(
        world
            .space_registry()
            .get_portal(portal_id)
            .unwrap()
            .enabled
    );

    let goal_space = region_space(&world, building_id, "ground", "main");
    let start = local_xz_to_world(&world, building_id, Vec2::new(3.0, -2.0));
    let goal = local_xz_to_world(&world, building_id, Vec2::new(6.0, 4.0));
    let (doodad, building, footprint) = default_catalogs();
    let catalogs = pass_catalogs(&doodad, &building, &footprint);

    find_path_with_spaces(
        &world,
        catalogs,
        &NavigationConfig::default(),
        ROBOT_RADIUS,
        45.0,
        start,
        goal,
        SpaceId::SURFACE,
        goal_space,
        None,
    )
    .expect("open door-controlled entrance must allow traversal planning");
}
