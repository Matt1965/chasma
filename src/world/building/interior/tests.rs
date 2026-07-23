//! B7 interior, door, and child-object tests (ADR-084).

use bevy::prelude::*;

use super::catalog::InteriorProfileCatalog;
use super::door::{DoorAccessPolicy, DoorState};
use super::id::{DoorId, InteriorProfileId};
use super::{
    activate_building_interior, close_door, deactivate_building_interior, open_door,
    portal_traversable, try_activate_interior_if_complete, try_open_door_for_unit,
};
use crate::world::{
    Affiliation, BuildingCatalog, BuildingDefinitionId, BuildingLifecycleState, BuildingOwnership,
    BuildingSource, ChunkCoord, ChunkData, ChunkId, ChunkLayout, DoodadCatalog, FootprintCatalog,
    Heightfield, LocalPosition, NavigationConfig, OccupancyCatalogs, OwnerId, PassabilityCatalogs,
    UnitOwnership, WorldData, WorldPosition, create_building, find_path_with_spaces,
    place_player_building, set_building_lifecycle_stage, BuildingNavigationBlueprintCatalog,
};

fn layout_world() -> WorldData {
    WorldData::new(ChunkLayout {
        chunk_size_meters: 256.0,
        units_per_meter: 1.0,
    })
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

fn position(x: f32, z: f32) -> WorldPosition {
    WorldPosition::new(
        ChunkCoord::new(0, 0),
        LocalPosition::new(Vec3::new(x, 0.0, z)),
    )
}

fn interior_catalog() -> InteriorProfileCatalog {
    InteriorProfileCatalog::default()
}

#[test]
fn interior_profile_loads_with_room_tags() {
    let catalog = interior_catalog();
    let profile = catalog
        .get(&InteriorProfileId::new("two_story_hut"))
        .expect("profile");
    assert_eq!(profile.spaces.len(), 2);
    assert_eq!(profile.spaces[0].room_tag, Some("hall"));
    assert_eq!(profile.spaces[1].room_tag, Some("bedroom"));
}

#[test]
fn barn_interior_profile_has_open_entrance() {
    let catalog = interior_catalog();
    let profile = catalog
        .get(&InteriorProfileId::new("barn_interior"))
        .expect("barn profile");
    assert_eq!(profile.spaces.len(), 1);
    assert!(profile.doors.is_empty());
    assert_eq!(profile.portals.len(), 1);
    assert_eq!(profile.portals[0].key, "exterior_entrance");
}

#[test]
fn try_activate_interior_if_complete_on_dev_spawned_hut() {
    let building_catalog = BuildingCatalog::default();
    let doodad_catalog = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let occupancy = occ(&building_catalog, &doodad_catalog, &footprint);
    let interior = interior_catalog();
    let mut world = layout_world();

    let record = create_building(
        &building_catalog,
        &mut world,
        &BuildingDefinitionId::new("hut"),
        position(80.0, 80.0),
        Quat::IDENTITY,
        BuildingSource::Dev,
        BuildingOwnership::with_affiliation(Affiliation::Player),
        Some(occupancy),
    )
    .unwrap();

    try_activate_interior_if_complete(
        &mut world,
        &building_catalog,
        &interior,
        &doodad_catalog,
        occupancy,
        None,
        record.id,
    )
    .unwrap();

    let activated = world.get_building(record.id).unwrap();
    assert!(activated.interior.activated);
    assert!(
        !world
            .space_registry()
            .building_space_ids(record.id)
            .is_empty()
    );
}

#[test]
fn completion_spawns_interior_children_and_doors_once() {
    let building_catalog = BuildingCatalog::default();
    let doodad_catalog = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let occupancy = occ(&building_catalog, &doodad_catalog, &footprint);
    let interior = interior_catalog();
    let mut world = layout_world();

    let id = place_player_building(
        &building_catalog,
        &mut world,
        &BuildingDefinitionId::new("hut"),
        position(64.0, 64.0),
        Quat::IDENTITY,
        BuildingOwnership::with_affiliation(Affiliation::Player),
        occupancy,
    )
    .unwrap()
    .id;

    set_building_lifecycle_stage(
        &mut world,
        &building_catalog,
        &interior,
        &doodad_catalog,
        occupancy,
        None,
        id,
        BuildingLifecycleState::Complete,
        1.0,
    )
    .unwrap();

    let record = world.get_building(id).unwrap();
    assert!(record.interior.activated);
    assert!(!record.interior.door_ids.is_empty());
    assert!(!record.interior.child_doodad_ids.is_empty());
    assert!(!record.interior.child_building_ids.is_empty());

    let door_id = DoorId::new(record.interior.door_ids[0]);
    let door = world.door_store().get(door_id).expect("door");
    assert_eq!(door.state, DoorState::Closed);
    assert!(
        !world
            .space_registry()
            .get_portal(door.portal_id)
            .expect("portal")
            .enabled
    );

    let child_count_before = world.sorted_doodad_ids().len();
    let _ = activate_building_interior(
        &mut world,
        &building_catalog,
        &interior,
        &doodad_catalog,
        occupancy,
        None,
        id,
        &InteriorProfileId::new("two_story_hut"),
    );
    assert_eq!(world.sorted_doodad_ids().len(), child_count_before);
}

#[test]
fn door_open_close_updates_portal_passability() {
    let building_catalog = BuildingCatalog::default();
    let doodad_catalog = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let occupancy = occ(&building_catalog, &doodad_catalog, &footprint);
    let interior = interior_catalog();
    let mut world = layout_world();

    let id = create_building(
        &building_catalog,
        &mut world,
        &BuildingDefinitionId::new("hut"),
        position(32.0, 32.0),
        Quat::IDENTITY,
        BuildingSource::Authored,
        BuildingOwnership::neutral(),
        Some(occupancy),
    )
    .unwrap()
    .id;

    activate_building_interior(
        &mut world,
        &building_catalog,
        &interior,
        &doodad_catalog,
        occupancy,
        None,
        id,
        &InteriorProfileId::new("two_story_hut"),
    )
    .unwrap();

    let door_id = DoorId::new(world.get_building(id).unwrap().interior.door_ids[0]);
    let portal_id = world.door_store().get(door_id).unwrap().portal_id;

    open_door(&mut world, door_id).unwrap();
    assert!(
        world
            .space_registry()
            .get_portal(portal_id)
            .unwrap()
            .enabled
    );
    assert!(portal_traversable(
        &world,
        portal_id,
        BuildingOwnership::neutral(),
        None,
    ));

    close_door(&mut world, door_id).unwrap();
    assert!(
        !world
            .space_registry()
            .get_portal(portal_id)
            .unwrap()
            .enabled
    );
}

#[test]
fn authorized_unit_can_open_closed_door() {
    let building_catalog = BuildingCatalog::default();
    let doodad_catalog = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let occupancy = occ(&building_catalog, &doodad_catalog, &footprint);
    let interior = interior_catalog();
    let mut world = layout_world();

    let owner = BuildingOwnership::with_affiliation(Affiliation::Player);
    let id = create_building(
        &building_catalog,
        &mut world,
        &BuildingDefinitionId::new("hut"),
        position(40.0, 40.0),
        Quat::IDENTITY,
        BuildingSource::Authored,
        owner,
        Some(occupancy),
    )
    .unwrap()
    .id;

    activate_building_interior(
        &mut world,
        &building_catalog,
        &interior,
        &doodad_catalog,
        occupancy,
        None,
        id,
        &InteriorProfileId::new("two_story_hut"),
    )
    .unwrap();

    let door_id = DoorId::new(world.get_building(id).unwrap().interior.door_ids[0]);
    let portal_id = world.door_store().get(door_id).unwrap().portal_id;
    let unit = UnitOwnership::player_default();

    assert!(try_open_door_for_unit(&mut world, door_id, owner, unit).unwrap());
    assert_eq!(
        world.door_store().get(door_id).unwrap().state,
        DoorState::Open
    );
    assert!(portal_traversable(&world, portal_id, owner, Some(unit)));
}

#[test]
fn locked_door_blocks_unauthorized_open() {
    let building_catalog = BuildingCatalog::default();
    let doodad_catalog = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let occupancy = occ(&building_catalog, &doodad_catalog, &footprint);
    let interior = interior_catalog();
    let mut world = layout_world();

    let owner = BuildingOwnership {
        owner_id: Some(OwnerId::new(1)),
        team_id: None,
        affiliation: Affiliation::Player,
    };
    let id = create_building(
        &building_catalog,
        &mut world,
        &BuildingDefinitionId::new("hut"),
        position(48.0, 48.0),
        Quat::IDENTITY,
        BuildingSource::Authored,
        owner,
        Some(occupancy),
    )
    .unwrap()
    .id;

    activate_building_interior(
        &mut world,
        &building_catalog,
        &interior,
        &doodad_catalog,
        occupancy,
        None,
        id,
        &InteriorProfileId::new("two_story_hut"),
    )
    .unwrap();

    let door_id = DoorId::new(world.get_building(id).unwrap().interior.door_ids[0]);
    world
        .door_store_mut()
        .get_mut(door_id)
        .expect("door")
        .access = DoorAccessPolicy::OwnerOnly;

    let stranger = UnitOwnership::hostile();
    assert!(!try_open_door_for_unit(&mut world, door_id, owner, stranger).unwrap());
    assert_eq!(
        world.door_store().get(door_id).unwrap().state,
        DoorState::Closed
    );
}

#[test]
fn ruins_transition_cleans_interior_state() {
    let building_catalog = BuildingCatalog::default();
    let doodad_catalog = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let occupancy = occ(&building_catalog, &doodad_catalog, &footprint);
    let interior = interior_catalog();
    let mut world = layout_world();

    let id = create_building(
        &building_catalog,
        &mut world,
        &BuildingDefinitionId::new("hut"),
        position(56.0, 56.0),
        Quat::IDENTITY,
        BuildingSource::Authored,
        BuildingOwnership::neutral(),
        Some(occupancy),
    )
    .unwrap()
    .id;

    activate_building_interior(
        &mut world,
        &building_catalog,
        &interior,
        &doodad_catalog,
        occupancy,
        None,
        id,
        &InteriorProfileId::new("two_story_hut"),
    )
    .unwrap();

    let door_ids: Vec<_> = world.get_building(id).unwrap().interior.door_ids.clone();
    let child_doodads: Vec<_> = world
        .get_building(id)
        .unwrap()
        .interior
        .child_doodad_ids
        .clone();

    deactivate_building_interior(
        &mut world,
        &doodad_catalog,
        &building_catalog,
        Some(occupancy),
        id,
    )
    .unwrap();

    let record = world.get_building(id).unwrap();
    assert!(!record.interior.activated);
    assert!(record.interior.door_ids.is_empty());
    for raw in door_ids {
        assert!(world.door_store().get(DoorId::new(raw)).is_none());
    }
    for raw in child_doodads {
        assert!(world.get_doodad(crate::world::DoodadId::new(raw)).is_none());
    }
    assert!(world.space_registry().building_space_ids(id).is_empty());
}

fn flat_world_with_terrain() -> WorldData {
    let layout = ChunkLayout {
        chunk_size_meters: 256.0,
        units_per_meter: 1.0,
    };
    let mut world = WorldData::new(layout);
    let heightfield = Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
    world.insert(
        ChunkId::new(ChunkCoord::new(0, 0)),
        ChunkData::new(heightfield, Vec::new()),
    );
    world
}

fn interior_centroid(
    outline: &[Vec2],
    layout: ChunkLayout,
    space_id: crate::world::SpaceId,
    world: &WorldData,
) -> WorldPosition {
    let centroid_xz = outline.iter().fold(Vec2::ZERO, |acc, v| acc + *v) / outline.len() as f32;
    let floor_y = world
        .space_registry()
        .get_space(space_id)
        .map(|space| space.floor_y_global)
        .unwrap_or(0.0);
    WorldPosition::from_global(Vec3::new(centroid_xz.x, floor_y, centroid_xz.y), layout)
}

#[test]
fn blueprint_runtime_registers_floors_and_paths_to_upper_level() {
    let building_catalog = BuildingCatalog::default();
    let doodad_catalog = DoodadCatalog::default();
    let footprint = FootprintCatalog::default();
    let occupancy = occ(&building_catalog, &doodad_catalog, &footprint);
    let interior = interior_catalog();
    let nav_catalog = BuildingNavigationBlueprintCatalog::default();
    let mut world = flat_world_with_terrain();

    let id = place_player_building(
        &building_catalog,
        &mut world,
        &BuildingDefinitionId::new("hut"),
        position(80.0, 80.0),
        Quat::IDENTITY,
        BuildingOwnership::with_affiliation(Affiliation::Player),
        occupancy,
    )
    .unwrap()
    .id;

    set_building_lifecycle_stage(
        &mut world,
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

    let runtime = world
        .building_navigation_runtime()
        .get(id)
        .expect("runtime navigation cache");
    assert_eq!(runtime.floors.len(), 2);

    let ground = runtime
        .floors
        .iter()
        .find(|floor| floor.floor_key == "ground_interior")
        .expect("ground floor");
    let upper = runtime
        .floors
        .iter()
        .find(|floor| floor.floor_key == "upper_interior")
        .expect("upper floor");

    let layout = world.layout();
    let ground_pos = interior_centroid(&ground.world_outline_xz, layout, ground.space_id, &world);
    let upper_pos = interior_centroid(&upper.world_outline_xz, layout, upper.space_id, &world);

    let catalogs = PassabilityCatalogs {
        doodad: &doodad_catalog,
        building: &building_catalog,
        footprint: &footprint,
    };

    let path = find_path_with_spaces(
        &world,
        catalogs,
        &NavigationConfig::default(),
        0.5,
        45.0,
        ground_pos,
        upper_pos,
        ground.space_id,
        upper.space_id,
        None,
    )
    .expect("interior path across blueprint stairs");

    assert!(path.waypoints.iter().any(|wp| wp.space_id == upper.space_id));
    assert!(path.waypoints.iter().any(|wp| wp.portal_id.is_some()));
}
