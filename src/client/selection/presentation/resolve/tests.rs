//! Footprint resolution tests for selection presentation.

use super::*;
use crate::item_piles::ItemPilePresentationSettings;
use crate::world::FootprintShape;
use crate::world::asset_sizing::AssetSizingDefinition;
use crate::world::{
    BuildingCatalog, BuildingDefinitionId, BuildingId, BuildingOwnership, BuildingPlacement,
    BuildingRecord, BuildingSource, ChunkCoord, DoodadCatalog, DoodadDefinition,
    DoodadDefinitionId, DoodadId, DoodadKind, DoodadPlacement, DoodadRecord, DoodadRenderKey,
    DoodadSource, FootprintCatalog, ItemPileId, ItemPileSettings, LocalPosition, WorldData,
    WorldItemPileRecord, WorldPosition, create_building,
};
use bevy::math::{Quat, Vec3};

fn layout() -> crate::world::ChunkLayout {
    crate::world::ChunkLayout {
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

#[test]
fn building_footprint_uses_definition_and_scale() {
    let building_catalog = BuildingCatalog::default();
    let footprint_catalog = FootprintCatalog::default();
    let definition = building_catalog
        .get(&BuildingDefinitionId::new("hut"))
        .unwrap();
    let mut placement = BuildingPlacement::new(pos(10.0, 20.0), Quat::IDENTITY);
    placement = placement.with_uniform_scale(crate::world::FixedScale::from_f32(2.0).unwrap());
    let record = BuildingRecord::new(
        BuildingId::new(1),
        definition.id.clone(),
        placement,
        BuildingOwnership::neutral(),
        100,
        BuildingSource::Authored,
    );
    let resolved = resolve_building_selection_footprint(
        &record,
        definition,
        &footprint_catalog,
        layout(),
        1.0,
    )
    .unwrap();
    assert!(!resolved.terrain_conforming);
    match resolved.shape {
        FootprintShape::Rectangle {
            width_meters,
            depth_meters,
        } => {
            assert!((width_meters - 8.0).abs() < 0.01);
            assert!((depth_meters - 8.0).abs() < 0.01);
        }
        other => panic!("expected rectangle, got {other:?}"),
    }
}

#[test]
fn building_rotation_preserved_in_yaw() {
    let building_catalog = BuildingCatalog::default();
    let footprint_catalog = FootprintCatalog::default();
    let definition = building_catalog
        .get(&BuildingDefinitionId::new("hut"))
        .unwrap();
    let placement = BuildingPlacement::new(
        pos(0.0, 0.0),
        Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
    );
    let record = BuildingRecord::new(
        BuildingId::new(1),
        definition.id.clone(),
        placement,
        BuildingOwnership::neutral(),
        100,
        BuildingSource::Authored,
    );
    let resolved = resolve_building_selection_footprint(
        &record,
        definition,
        &footprint_catalog,
        layout(),
        1.0,
    )
    .unwrap();
    assert!((resolved.yaw_radians - std::f32::consts::FRAC_PI_2).abs() < 0.01);
}

#[test]
fn two_building_instances_resolve_independently() {
    let building_catalog = BuildingCatalog::default();
    let footprint_catalog = FootprintCatalog::default();
    let definition = building_catalog
        .get(&BuildingDefinitionId::new("hut"))
        .unwrap();
    let a = BuildingRecord::new(
        BuildingId::new(1),
        definition.id.clone(),
        BuildingPlacement::new(pos(0.0, 0.0), Quat::IDENTITY),
        BuildingOwnership::neutral(),
        100,
        BuildingSource::Authored,
    );
    let b = BuildingRecord::new(
        BuildingId::new(2),
        definition.id.clone(),
        BuildingPlacement::new(pos(50.0, 50.0), Quat::from_rotation_y(1.0)),
        BuildingOwnership::neutral(),
        100,
        BuildingSource::Authored,
    );
    let ra =
        resolve_building_selection_footprint(&a, definition, &footprint_catalog, layout(), 1.0)
            .unwrap();
    let rb =
        resolve_building_selection_footprint(&b, definition, &footprint_catalog, layout(), 1.0)
            .unwrap();
    assert_ne!(ra.anchor_render, rb.anchor_render);
    assert_ne!(ra.yaw_radians, rb.yaw_radians);
}

#[test]
fn doodad_collision_scale_and_rotation_applied() {
    let mut def = DoodadDefinition::new(
        DoodadDefinitionId::new("rock"),
        DoodadKind::Rock,
        "Rock",
        1.0,
        0.5,
        2.0,
        None,
        None,
        None,
        true,
        DoodadRenderKey::reserved("rock"),
    );
    def.blocks_movement = true;
    def.collision_shape = crate::world::asset_sizing::DoodadCollisionShape::Rectangle;
    def.base_collision_radius_x_meters = 2.0;
    def.base_collision_radius_z_meters = 1.0;
    def.asset_sizing = AssetSizingDefinition::default();
    let mut placement = DoodadPlacement::from_millidegrees_and_scale(
        pos(5.0, 5.0),
        45_000,
        0,
        0,
        2_000,
        1_000,
        2_000,
    )
    .unwrap();
    let record = DoodadRecord::new(
        DoodadId::new(1),
        def.id.clone(),
        def.kind,
        placement,
        DoodadSource::Authored,
    );
    let collision = crate::world::resolve_doodad_collision(&record, &def);
    let resolved =
        resolve_doodad_selection_footprint_with_collision(&record, &def, &collision, layout(), 1.0)
            .unwrap();
    match resolved.shape {
        FootprintShape::Rectangle {
            width_meters,
            depth_meters,
        } => {
            assert!((width_meters - 8.0).abs() < 0.01);
            assert!((depth_meters - 4.0).abs() < 0.01);
        }
        other => panic!("expected rectangle, got {other:?}"),
    }
    assert!((resolved.yaw_radians - 45.0_f32.to_radians()).abs() < 0.01);
}

#[test]
fn item_pile_uses_minimum_indicator_radius() {
    let record = WorldItemPileRecord::new_stack(
        ItemPileId::new(1),
        pos(1.0, 2.0),
        crate::world::SpaceId::SURFACE,
        crate::world::ItemDefinitionId::new("iron"),
        5,
        None,
        None,
        crate::world::Affiliation::Neutral,
        crate::world::ItemPileSource::Dropped,
        0,
    );
    let settings = ItemPileSettings {
        merge_radius_meters: 0.5,
        ..Default::default()
    };
    let presentation = ItemPilePresentationSettings {
        fallback_sphere_radius: 0.3,
        ..Default::default()
    };
    let resolved =
        resolve_item_pile_selection_footprint(&record, &settings, &presentation, layout(), 1.0);
    assert!(resolved.terrain_conforming);
    match resolved.shape {
        FootprintShape::Circle { radius_meters } => {
            assert!((radius_meters - ITEM_PILE_SELECTION_MIN_RADIUS_METERS).abs() < 0.01);
        }
        other => panic!("expected circle, got {other:?}"),
    }
}

#[test]
fn world_object_target_from_selection_category() {
    assert_eq!(
        WorldObjectPresentationTarget::from_selection(
            crate::client::selection::WorldSelectionCategory::Building,
            Some(BuildingId::new(3)),
            None,
            None,
        ),
        Some(WorldObjectPresentationTarget::Building(BuildingId::new(3)))
    );
    assert!(
        WorldObjectPresentationTarget::from_selection(
            crate::client::selection::WorldSelectionCategory::Units,
            None,
            None,
            None,
        )
        .is_none()
    );
}

#[test]
fn resolve_world_object_from_world_data() {
    let layout = layout();
    let mut world = WorldData::new(layout);
    let heightfield = crate::world::Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
    world.insert(
        crate::world::ChunkId::new(ChunkCoord::new(0, 0)),
        crate::world::ChunkData::new(heightfield, Vec::new()),
    );
    let building_catalog = BuildingCatalog::default();
    let footprint_catalog = FootprintCatalog::default();
    let doodad_catalog = DoodadCatalog::default();
    let occ = crate::world::OccupancyCatalogs {
        doodad: &doodad_catalog,
        building: &building_catalog,
        footprint: &footprint_catalog,
    };
    let record = create_building(
        &building_catalog,
        &mut world,
        &BuildingDefinitionId::new("hut"),
        pos(30.0, 30.0),
        Quat::IDENTITY,
        BuildingSource::Authored,
        BuildingOwnership::neutral(),
        Some(occ),
    )
    .unwrap();
    let resolved = resolve_world_object_footprint(
        WorldObjectPresentationTarget::Building(record.id),
        &world,
        &building_catalog,
        &footprint_catalog,
        &doodad_catalog,
        &ItemPileSettings::default(),
        &ItemPilePresentationSettings::default(),
        layout,
        1.0,
    );
    assert!(resolved.is_some());
}
