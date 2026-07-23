//! B6 integration tests for spaces, portals, and cross-space navigation.

use bevy::prelude::*;

use super::{
    PortalId, PortalRecord, PortalType, SpaceId, SpaceRecord, SpaceRegistry,
    UnitPortalTransitionState, register_building_space_profile, try_portal_transition,
    two_story_hut_profile,
};
use crate::world::{
    BuildingCatalog, BuildingDefinitionId, BuildingId, BuildingPlacement, BuildingRecord,
    BuildingSource, ChunkCoord, ChunkData, ChunkId, ChunkLayout, DoodadCatalog, FootprintCatalog,
    Heightfield, LocalPosition, NavigationConfig, PassabilityCatalogs, UnitCatalog,
    UnitDefinitionId, UnitPlacement, UnitSource, WorldData, WorldPosition, create_unit,
    find_path_with_spaces,
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

fn flat_world() -> WorldData {
    let mut world = WorldData::new(layout());
    let heightfield = Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
    world.insert(
        ChunkId::new(ChunkCoord::new(0, 0)),
        ChunkData::new(heightfield, Vec::new()),
    );
    world
}

fn passability() -> PassabilityCatalogs<'static> {
    PassabilityCatalogs {
        doodad: Box::leak(Box::new(DoodadCatalog::default())),
        building: Box::leak(Box::new(BuildingCatalog::default())),
        footprint: Box::leak(Box::new(FootprintCatalog::default())),
    }
}

fn portal_stair(id: u32, from: SpaceId, to: SpaceId, x: f32, z: f32, floor_y: f32) -> PortalRecord {
    PortalRecord {
        id: PortalId::new(id),
        portal_type: PortalType::Stair,
        from_space: from,
        to_space: to,
        from_center_global_xz: Vec2::new(x, z),
        from_radius_meters: 1.5,
        to_position: WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(x, floor_y, z)),
        ),
        traversal_cost: 1.0,
        bidirectional: true,
        enabled: true,
        owning_building_id: None,
    }
}

#[test]
fn surface_is_default_unit_space() {
    let world = flat_world();
    let catalog = UnitCatalog::default();
    let mut world = world;
    let unit_id = create_unit(
        &catalog,
        &mut world,
        &UnitDefinitionId::new("wolf"),
        pos(1.0, 2.0),
        UnitSource::Authored,
    )
    .unwrap()
    .id;
    assert_eq!(
        world.get_unit(unit_id).unwrap().current_space_id,
        SpaceId::SURFACE
    );
}

#[test]
fn portal_transition_is_deterministic_with_hysteresis() {
    let mut registry = SpaceRegistry::new();
    let upper = registry.allocate_space_id();
    registry.insert_space(SpaceRecord {
        id: upper,
        owning_building_id: None,
        display_floor_label: "Upper".into(),
        visibility_group_id: 2,
        reference_elevation: 4.0,
        floor_y_global: 4.0,
        room_tag: None,
        enabled: true,
        walkable: true,
    });
    registry.insert_portal(portal_stair(1, SpaceId::SURFACE, upper, 10.0, 10.0, 4.0));

    let agent = pos(10.0, 10.0);
    let mut state = UnitPortalTransitionState::default();
    let first = try_portal_transition(
        &registry,
        layout(),
        SpaceId::SURFACE,
        agent,
        &mut state,
        None,
    );
    assert!(first.is_some());
    let second = try_portal_transition(
        &registry,
        layout(),
        SpaceId::SURFACE,
        agent,
        &mut state,
        None,
    );
    assert!(second.is_none());
}

#[test]
fn cross_space_path_includes_space_ids() {
    let mut world = flat_world();
    let mut registry = SpaceRegistry::new();
    let ground = registry.allocate_space_id();
    let upper = registry.allocate_space_id();
    for (id, label, elevation) in [(ground, "Ground", 0.0), (upper, "Upper", 4.0)] {
        registry.insert_space(SpaceRecord {
            id,
            owning_building_id: None,
            display_floor_label: label.into(),
            visibility_group_id: id.raw(),
            reference_elevation: elevation,
            floor_y_global: elevation,
            room_tag: None,
            enabled: true,
            walkable: true,
        });
    }
    registry.insert_portal(portal_stair(1, SpaceId::SURFACE, ground, 8.0, 8.0, 0.0));
    registry.insert_portal(portal_stair(2, ground, upper, 9.0, 9.0, 4.0));
    *world.space_registry_mut() = registry;

    let path = find_path_with_spaces(
        &world,
        passability(),
        &NavigationConfig::default(),
        0.5,
        40.0,
        pos(4.0, 4.0),
        pos(9.0, 9.0),
        SpaceId::SURFACE,
        upper,
        None,
    )
    .unwrap();
    assert!(path.waypoints.iter().any(|wp| wp.space_id == upper));
}

#[test]
fn hut_profile_registers_multiple_spaces() {
    let world = flat_world();
    let mut registry = world.space_registry().clone();
    let record = BuildingRecord::new(
        BuildingId::new(1),
        BuildingDefinitionId::new("hut"),
        BuildingPlacement::new(pos(20.0, 20.0), Quat::IDENTITY),
        crate::world::BuildingOwnership::neutral(),
        100,
        BuildingSource::Authored,
    );
    let (spaces, portals) = two_story_hut_profile();
    register_building_space_profile(&mut registry, &record, layout(), &spaces, &portals);
    assert!(registry.building_space_ids(record.id).len() >= 2);
}
