use bevy::prelude::*;

use crate::world::building::BuildingOwnership;
use crate::world::{
    AppearanceProfileCatalog, BuildingDefinitionId, BuildingSource, ChunkCoord, ChunkLayout,
    DoodadDefinitionId, DoodadPlacementOverrides, DoodadSource, LocalPosition, UnitCatalog,
    UnitDefinitionId, UnitSource, WorldData, WorldPosition, create_building, create_doodad,
    create_unit, starter_unit_definitions,
};

use super::building::{
    BuildingArchetypeMemberKind, DEFAULT_BUILDING_ARCHETYPE_CAPTURE_MARGIN_METERS,
};
use super::capture_volume::{
    compute_building_archetype_capture_region, compute_local_pose, expand_footprint_shape,
    pivot_in_capture_region, point_in_expanded_footprint, query_building_archetype_members,
};
use crate::world::FootprintShape;

fn layout() -> ChunkLayout {
    ChunkLayout {
        chunk_size_meters: 256.0,
        units_per_meter: 1.0,
    }
}

fn pos(x: f32, z: f32) -> WorldPosition {
    WorldPosition::new(ChunkCoord::new(0, 0), LocalPosition::new(Vec3::new(x, 0.0, z)))
}

fn catalogs() -> (
    crate::world::DoodadCatalog,
    crate::world::BuildingCatalog,
    crate::world::FootprintCatalog,
) {
    (
        crate::world::DoodadCatalog::default(),
        crate::world::BuildingCatalog::default(),
        crate::world::FootprintCatalog::default(),
    )
}

#[test]
fn root_excluded_from_members() {
    let (doodad, building, footprint) = catalogs();
    let mut world = WorldData::new(layout());
    let root = create_building(
        &building,
        &mut world,
        &BuildingDefinitionId::new("hut"),
        pos(50.0, 50.0),
        Quat::IDENTITY,
        BuildingSource::Dev,
        BuildingOwnership::neutral(),
        None,
    )
    .unwrap();
    let region = compute_building_archetype_capture_region(
        &world,
        world.get_building(root.id).unwrap(),
        &building,
        &footprint,
        DEFAULT_BUILDING_ARCHETYPE_CAPTURE_MARGIN_METERS,
    )
    .unwrap();
    let members = query_building_archetype_members(
        &world,
        &region,
        root.id,
        &building,
        &doodad,
    );
    assert!(members.is_empty());
}

#[test]
fn building_inside_region_included_outside_excluded() {
    let (doodad, building, footprint) = catalogs();
    let mut world = WorldData::new(layout());
    let root = create_building(
        &building,
        &mut world,
        &BuildingDefinitionId::new("hut"),
        pos(50.0, 50.0),
        Quat::IDENTITY,
        BuildingSource::Dev,
        BuildingOwnership::neutral(),
        None,
    )
    .unwrap();
    create_building(
        &building,
        &mut world,
        &BuildingDefinitionId::new("hut"),
        pos(52.0, 50.0),
        Quat::IDENTITY,
        BuildingSource::Dev,
        BuildingOwnership::neutral(),
        None,
    )
    .unwrap();
    create_building(
        &building,
        &mut world,
        &BuildingDefinitionId::new("hut"),
        pos(80.0, 80.0),
        Quat::IDENTITY,
        BuildingSource::Dev,
        BuildingOwnership::neutral(),
        None,
    )
    .unwrap();
    let region = compute_building_archetype_capture_region(
        &world,
        world.get_building(root.id).unwrap(),
        &building,
        &footprint,
        3.0,
    )
    .unwrap();
    let members = query_building_archetype_members(
        &world,
        &region,
        root.id,
        &building,
        &doodad,
    );
    assert_eq!(members.len(), 1);
    assert_eq!(members[0].kind, BuildingArchetypeMemberKind::Building);
    assert_eq!(members[0].definition_id, "hut");
}

#[test]
fn doodad_inside_region_included() {
    let (doodad, building, footprint) = catalogs();
    let mut world = WorldData::new(layout());
    let root = create_building(
        &building,
        &mut world,
        &BuildingDefinitionId::new("hut"),
        pos(50.0, 50.0),
        Quat::IDENTITY,
        BuildingSource::Dev,
        BuildingOwnership::neutral(),
        None,
    )
    .unwrap();
    create_doodad(
        &doodad,
        &mut world,
        &DoodadDefinitionId::new("tree_oak"),
        pos(51.0, 51.0),
        DoodadSource::Dev,
        DoodadPlacementOverrides::default(),
        None,
    )
    .unwrap();
    let region = compute_building_archetype_capture_region(
        &world,
        world.get_building(root.id).unwrap(),
        &building,
        &footprint,
        3.0,
    )
    .unwrap();
    let members = query_building_archetype_members(
        &world,
        &region,
        root.id,
        &building,
        &doodad,
    );
    assert_eq!(members.len(), 1);
    assert_eq!(members[0].kind, BuildingArchetypeMemberKind::Doodad);
    assert_eq!(members[0].definition_id, "tree_oak");
}

#[test]
fn unit_inside_region_excluded() {
    let (doodad, building, footprint) = catalogs();
    let unit_catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
    let appearance = AppearanceProfileCatalog::default();
    let mut world = WorldData::new(layout());
    let root = create_building(
        &building,
        &mut world,
        &BuildingDefinitionId::new("hut"),
        pos(50.0, 50.0),
        Quat::IDENTITY,
        BuildingSource::Dev,
        BuildingOwnership::neutral(),
        None,
    )
    .unwrap();
    create_unit(
        &unit_catalog,
        &appearance,
        &mut world,
        &UnitDefinitionId::new("bandit"),
        pos(51.0, 51.0),
        UnitSource::Dev,
    )
    .unwrap();
    let region = compute_building_archetype_capture_region(
        &world,
        world.get_building(root.id).unwrap(),
        &building,
        &footprint,
        3.0,
    )
    .unwrap();
    let members = query_building_archetype_members(
        &world,
        &region,
        root.id,
        &building,
        &doodad,
    );
    assert!(members.is_empty());
}

#[test]
fn margin_changes_inclusion() {
    let anchor = Vec2::new(50.0, 50.0);
    let shape = FootprintShape::Rectangle {
        width_meters: 4.0,
        depth_meters: 4.0,
    };
    let small = expand_footprint_shape(&shape, 1.0);
    let large = expand_footprint_shape(&shape, 5.0);
    let near = Vec2::new(54.5, 50.0);
    assert!(!point_in_expanded_footprint(near, anchor, 0.0, &small, 1.0));
    assert!(point_in_expanded_footprint(near, anchor, 0.0, &large, 5.0));
}

#[test]
fn rotated_root_capture_follows_orientation() {
    let anchor = Vec2::ZERO;
    let shape = FootprintShape::Rectangle {
        width_meters: 4.0,
        depth_meters: 8.0,
    };
    let expanded = expand_footprint_shape(&shape, 0.0);
    let yaw = std::f32::consts::FRAC_PI_2;
    let inside_rotated = Vec2::new(3.0, 0.0);
    assert!(point_in_expanded_footprint(inside_rotated, anchor, yaw, &expanded, 0.0));
    let outside_rotated = Vec2::new(0.0, 3.0);
    assert!(!point_in_expanded_footprint(outside_rotated, anchor, yaw, &expanded, 0.0));
}

#[test]
fn local_pose_round_trip_with_root_rotation() {
    let layout = layout();
    let root = create_building(
        &catalogs().1,
        &mut WorldData::new(layout),
        &BuildingDefinitionId::new("hut"),
        pos(10.0, 10.0),
        Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
        BuildingSource::Dev,
        BuildingOwnership::neutral(),
        None,
    )
    .unwrap();
    let member_world = Vec3::new(12.0, 0.0, 10.0);
    let local = compute_local_pose(
        layout,
        &root,
        member_world,
        Quat::IDENTITY,
        Some(1.0),
        None,
    );
    let root_global = root.placement.position.to_global(layout);
    let rebuilt = root_global + root.placement.rotation * Vec3::from_array(local.local_position);
    assert!((rebuilt.x - member_world.x).abs() < 0.02);
    assert!((rebuilt.z - member_world.z).abs() < 0.02);
}

#[test]
fn member_snapshot_has_no_runtime_ids() {
    let (doodad, building, footprint) = catalogs();
    let mut world = WorldData::new(layout());
    let root = create_building(
        &building,
        &mut world,
        &BuildingDefinitionId::new("hut"),
        pos(50.0, 50.0),
        Quat::IDENTITY,
        BuildingSource::Dev,
        BuildingOwnership::neutral(),
        None,
    )
    .unwrap();
    create_building(
        &building,
        &mut world,
        &BuildingDefinitionId::new("hut"),
        pos(52.0, 50.0),
        Quat::IDENTITY,
        BuildingSource::Dev,
        BuildingOwnership::neutral(),
        None,
    )
    .unwrap();
    let region = compute_building_archetype_capture_region(
        &world,
        world.get_building(root.id).unwrap(),
        &building,
        &footprint,
        3.0,
    )
    .unwrap();
    let members = query_building_archetype_members(
        &world,
        &region,
        root.id,
        &building,
        &doodad,
    );
    assert_eq!(members.len(), 1);
    let serialized = ron::ser::to_string(&members[0]).unwrap();
    assert!(!serialized.contains("building_id"));
    assert!(!serialized.contains("doodad_id"));
}

#[test]
fn pivot_in_capture_region_uses_xz_only() {
    let (doodad, building, footprint) = catalogs();
    let mut world = WorldData::new(layout());
    let root = create_building(
        &building,
        &mut world,
        &BuildingDefinitionId::new("hut"),
        pos(50.0, 50.0),
        Quat::IDENTITY,
        BuildingSource::Dev,
        BuildingOwnership::neutral(),
        None,
    )
    .unwrap();
    let region = compute_building_archetype_capture_region(
        &world,
        world.get_building(root.id).unwrap(),
        &building,
        &footprint,
        3.0,
    )
    .unwrap();
    let high = WorldPosition::new(
        ChunkCoord::new(0, 0),
        LocalPosition::new(Vec3::new(51.0, 25.0, 51.0)),
    );
    assert!(pivot_in_capture_region(high, layout(), &region));
}
