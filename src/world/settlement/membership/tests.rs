//! Phase 2 settlement membership authority tests (ADR-133).

use bevy::prelude::{Quat, Vec3};

use crate::world::building::BuildingInteractionProfileCatalog;
use crate::world::{
    Affiliation, BuildingCatalog, BuildingOwnership, BuildingSource, ChunkCoord, ChunkData,
    ChunkLayout, Heightfield, InventoryCatalogCtx, InventoryProfileCatalog, ItemCatalog,
    ItemCategoryCatalog, LocalPosition, SettlementKind, SettlementOwnership, UnitCatalog,
    UnitDefinitionId, UnitOwnership, UnitSource, WorldData, WorldPosition,
    assign_building_settlement, assign_selected_units_at_position, assign_unit_settlement,
    clear_unit_settlement_on_removal, create_building, create_settlement,
    create_settlement_with_treasury, create_unit_with_inventory, move_building,
    rebuild_settlement_membership_indexes, starter_building_definitions,
    starter_inventory_profile_definitions, starter_item_category_definitions,
    starter_item_definitions, starter_unit_definitions,
};

fn layout() -> ChunkLayout {
    ChunkLayout {
        chunk_size_meters: 256.0,
        units_per_meter: 1.0,
    }
}

fn test_world() -> WorldData {
    let mut world = WorldData::new(layout());
    let heightfield = Heightfield::from_samples(65, 4.0, vec![0.0; 65 * 65]).unwrap();
    world.insert(
        crate::world::ChunkId::new(ChunkCoord::new(0, 0)),
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

fn inventory_ctx() -> InventoryCatalogCtx<'static> {
    let categories = Box::leak(Box::new(
        ItemCategoryCatalog::from_definitions(starter_item_category_definitions()).unwrap(),
    ));
    let items = Box::leak(Box::new(
        ItemCatalog::from_definitions(starter_item_definitions(), categories).unwrap(),
    ));
    let profiles = Box::leak(Box::new(
        InventoryProfileCatalog::from_definitions(starter_inventory_profile_definitions()).unwrap(),
    ));
    InventoryCatalogCtx::new(items, categories, profiles)
}

fn building_catalog() -> &'static BuildingCatalog {
    let categories = Box::leak(Box::new(crate::world::BuildingCategoryCatalog::default()));
    Box::leak(Box::new(
        BuildingCatalog::from_definitions(starter_building_definitions(), categories).unwrap(),
    ))
}

fn unit_catalog() -> UnitCatalog {
    UnitCatalog::from_definitions(starter_unit_definitions()).unwrap()
}

fn spawn_unit(
    world: &mut WorldData,
    x: f32,
    z: f32,
    affiliation: Affiliation,
) -> crate::world::UnitId {
    create_unit_with_inventory(
        &unit_catalog(),
        world,
        &UnitDefinitionId::new("bandit"),
        pos(x, z),
        UnitSource::Authored,
        UnitOwnership::with_affiliation(affiliation),
        &inventory_ctx(),
    )
    .unwrap()
    .id
}

fn spawn_settlement(
    world: &mut WorldData,
    x: f32,
    z: f32,
    name: &str,
) -> crate::world::SettlementId {
    create_settlement(
        world,
        pos(x, z),
        name,
        SettlementOwnership::player_default(),
        SettlementKind::Town,
        None,
        None,
        0,
    )
    .unwrap()
    .settlement_id
}

fn spawn_building(
    world: &mut WorldData,
    x: f32,
    z: f32,
    affiliation: Affiliation,
) -> crate::world::BuildingId {
    create_building(
        building_catalog(),
        world,
        &crate::world::BuildingDefinitionId::new("hut"),
        pos(x, z),
        Quat::IDENTITY,
        BuildingSource::Authored,
        BuildingOwnership::with_affiliation(affiliation),
        None,
    )
    .unwrap()
    .id
}

#[test]
fn unit_record_persists_some_settlement_id() {
    let mut world = test_world();
    let settlement_id = spawn_settlement(&mut world, 20.0, 20.0, "A");
    let unit_id = spawn_unit(&mut world, 5.0, 5.0, Affiliation::Player);
    assign_unit_settlement(&mut world, unit_id, Some(settlement_id)).unwrap();
    assert_eq!(
        world.get_unit(unit_id).unwrap().settlement_id,
        Some(settlement_id)
    );
}

#[test]
fn unit_none_membership_remains_none() {
    let mut world = test_world();
    let unit_id = spawn_unit(&mut world, 5.0, 5.0, Affiliation::Player);
    assert!(world.get_unit(unit_id).unwrap().settlement_id.is_none());
    assign_unit_settlement(&mut world, unit_id, None).unwrap();
    assert!(world.get_unit(unit_id).unwrap().settlement_id.is_none());
}

#[test]
fn affiliation_does_not_imply_unit_membership() {
    let mut world = test_world();
    let settlement_id = spawn_settlement(&mut world, 20.0, 20.0, "A");
    // Outside DEFAULT_TOWN_BOUNDARY_RADIUS_METERS (64m) from settlement center.
    let unit_id = spawn_unit(&mut world, 100.0, 100.0, Affiliation::Player);
    assert!(world.get_unit(unit_id).unwrap().settlement_id.is_none());
    assert!(
        world
            .settlement_store()
            .units_for_settlement(settlement_id)
            .is_empty()
    );
}

#[test]
fn unit_created_inside_settlement_seeds_membership() {
    let mut world = test_world();
    let settlement_id = spawn_settlement(&mut world, 20.0, 20.0, "A");
    let unit_id = spawn_unit(&mut world, 20.0, 20.0, Affiliation::Player);
    assert_eq!(
        world.get_unit(unit_id).unwrap().settlement_id,
        Some(settlement_id)
    );
    assert!(
        world
            .settlement_store()
            .units_for_settlement(settlement_id)
            .contains(&unit_id)
    );
}

#[test]
fn derived_unit_index_reflects_authoritative_field() {
    let mut world = test_world();
    let settlement_id = spawn_settlement(&mut world, 30.0, 30.0, "A");
    let unit_id = spawn_unit(&mut world, 8.0, 8.0, Affiliation::Player);
    assign_unit_settlement(&mut world, unit_id, Some(settlement_id)).unwrap();
    assert_eq!(
        world.settlement_store().settlement_for_unit(unit_id),
        Some(settlement_id)
    );
    assert_eq!(
        world.settlement_store().units_for_settlement(settlement_id),
        vec![unit_id]
    );
}

#[test]
fn explicit_unit_assignment_moves_between_settlements() {
    let mut world = test_world();
    let a = spawn_settlement(&mut world, 20.0, 20.0, "A");
    let b = spawn_settlement(&mut world, 140.0, 140.0, "B");
    let unit_id = spawn_unit(&mut world, 10.0, 10.0, Affiliation::Player);
    assign_unit_settlement(&mut world, unit_id, Some(a)).unwrap();
    assign_unit_settlement(&mut world, unit_id, Some(b)).unwrap();
    assert_eq!(world.get_unit(unit_id).unwrap().settlement_id, Some(b));
    assert!(world.settlement_store().units_for_settlement(a).is_empty());
    assert_eq!(
        world.settlement_store().units_for_settlement(b),
        vec![unit_id]
    );
}

#[test]
fn unit_death_clears_membership_and_index() {
    let mut world = test_world();
    let settlement_id = spawn_settlement(&mut world, 20.0, 20.0, "A");
    let unit_id = spawn_unit(&mut world, 10.0, 10.0, Affiliation::Player);
    assign_unit_settlement(&mut world, unit_id, Some(settlement_id)).unwrap();
    clear_unit_settlement_on_removal(&mut world, unit_id);
    let _ = world.remove_unit_by_id(unit_id);
    assert!(world.get_unit(unit_id).is_none());
    assert!(
        world
            .settlement_store()
            .units_for_settlement(settlement_id)
            .is_empty()
    );
    assert!(
        world
            .settlement_store()
            .settlement_for_unit(unit_id)
            .is_none()
    );
}

#[test]
fn building_record_persists_some_settlement_id() {
    let mut world = test_world();
    let settlement_id = spawn_settlement(&mut world, 20.0, 20.0, "A");
    let building_id = spawn_building(&mut world, 20.0, 20.0, Affiliation::Player);
    assign_building_settlement(&mut world, building_id, Some(settlement_id)).unwrap();
    assert_eq!(
        world.get_building(building_id).unwrap().settlement_id,
        Some(settlement_id)
    );
}

#[test]
fn building_inside_settlement_seeded_at_creation() {
    let mut world = test_world();
    let settlement_id = spawn_settlement(&mut world, 24.0, 24.0, "A");
    let building_id = spawn_building(&mut world, 24.0, 24.0, Affiliation::Neutral);
    assert_eq!(
        world.get_building(building_id).unwrap().settlement_id,
        Some(settlement_id)
    );
}

#[test]
fn building_outside_settlement_seeded_none_at_creation() {
    let mut world = test_world();
    spawn_settlement(&mut world, 24.0, 24.0, "A");
    let building_id = spawn_building(&mut world, 180.0, 180.0, Affiliation::Player);
    assert!(
        world
            .get_building(building_id)
            .unwrap()
            .settlement_id
            .is_none()
    );
}

#[test]
fn moving_building_does_not_change_settlement_id() {
    let mut world = test_world();
    spawn_settlement(&mut world, 24.0, 24.0, "A");
    let building_id = spawn_building(&mut world, 24.0, 24.0, Affiliation::Player);
    let original = world.get_building(building_id).unwrap().settlement_id;
    move_building(&mut world, building_id, pos(180.0, 180.0), None).unwrap();
    assert_eq!(
        world.get_building(building_id).unwrap().settlement_id,
        original
    );
}

#[test]
fn affiliation_match_alone_does_not_link_building() {
    let mut world = test_world();
    let settlement_id = spawn_settlement(&mut world, 30.0, 30.0, "A");
    let building_id = spawn_building(&mut world, 180.0, 180.0, Affiliation::Player);
    assert!(
        world
            .get_building(building_id)
            .unwrap()
            .settlement_id
            .is_none()
    );
    assert!(
        world
            .settlement_store()
            .buildings_for_settlement(settlement_id)
            .is_empty()
    );
}

#[test]
fn derived_building_index_reflects_record_field() {
    let mut world = test_world();
    let settlement_id = spawn_settlement(&mut world, 16.0, 16.0, "A");
    let building_id = spawn_building(&mut world, 16.0, 16.0, Affiliation::Player);
    rebuild_settlement_membership_indexes(&mut world);
    assert_eq!(
        world
            .settlement_store()
            .settlement_for_building(building_id),
        Some(settlement_id)
    );
}

fn interaction_catalog() -> &'static BuildingInteractionProfileCatalog {
    Box::leak(Box::new(BuildingInteractionProfileCatalog::default()))
}

#[test]
fn legacy_treasury_path_writes_authoritative_building_field() {
    let mut world = test_world();
    let building = create_building(
        building_catalog(),
        &mut world,
        &crate::world::BuildingDefinitionId::new("settlement_core"),
        pos(12.0, 12.0),
        Quat::IDENTITY,
        BuildingSource::Authored,
        BuildingOwnership::with_affiliation(Affiliation::Player),
        None,
    )
    .unwrap();
    world.mutate_building(building.id, |record| {
        record.lifecycle_state = crate::world::BuildingLifecycleState::Complete;
    });
    let report = create_settlement_with_treasury(
        &mut world,
        building_catalog(),
        interaction_catalog(),
        building.id,
        "Legacy Town",
        SettlementOwnership::player_default(),
        pos(12.0, 12.0),
        0,
    )
    .unwrap();
    assert_eq!(
        world.get_building(building.id).unwrap().settlement_id,
        Some(report.settlement_id)
    );
    assert_eq!(
        world
            .settlement_store()
            .settlement_for_building(building.id),
        Some(report.settlement_id)
    );
}

#[test]
fn same_affiliation_settlements_do_not_share_building() {
    let mut world = test_world();
    let a = spawn_settlement(&mut world, 20.0, 20.0, "A");
    let b = spawn_settlement(&mut world, 140.0, 140.0, "B");
    let building_id = spawn_building(&mut world, 20.0, 20.0, Affiliation::Player);
    assert_eq!(
        world.get_building(building_id).unwrap().settlement_id,
        Some(a)
    );
    assert!(
        !world
            .settlement_store()
            .buildings_for_settlement(b)
            .contains(&building_id)
    );
}

#[test]
fn rebuild_indexes_from_record_fields_after_manual_mutation() {
    let mut world = test_world();
    let settlement_id = spawn_settlement(&mut world, 18.0, 18.0, "A");
    let unit_id = spawn_unit(&mut world, 4.0, 4.0, Affiliation::Player);
    world
        .mutate_unit(unit_id, |record| record.settlement_id = Some(settlement_id))
        .unwrap();
    world.settlement_store_mut().clear_membership_indexes();
    rebuild_settlement_membership_indexes(&mut world);
    assert_eq!(
        world.settlement_store().settlement_for_unit(unit_id),
        Some(settlement_id)
    );
}

#[test]
fn none_membership_not_in_settlement_roster() {
    let mut world = test_world();
    let settlement_id = spawn_settlement(&mut world, 22.0, 22.0, "A");
    // Outside DEFAULT_TOWN_BOUNDARY_RADIUS_METERS (64m) from settlement center.
    let unit_id = spawn_unit(&mut world, 100.0, 100.0, Affiliation::Player);
    assert!(
        !world
            .settlement_store()
            .units_for_settlement(settlement_id)
            .contains(&unit_id)
    );
}

#[test]
fn assign_selected_units_at_position_assigns_inside_boundary() {
    let mut world = test_world();
    let settlement_id = spawn_settlement(&mut world, 40.0, 40.0, "A");
    let u1 = spawn_unit(&mut world, 5.0, 5.0, Affiliation::Player);
    let u2 = spawn_unit(&mut world, 6.0, 6.0, Affiliation::Player);
    let (assigned_id, count) =
        assign_selected_units_at_position(&mut world, &[u1, u2], pos(40.0, 40.0)).unwrap();
    assert_eq!(assigned_id, settlement_id);
    assert_eq!(count, 2);
    assert_eq!(
        world.get_unit(u1).unwrap().settlement_id,
        Some(settlement_id)
    );
    assert_eq!(
        world.get_unit(u2).unwrap().settlement_id,
        Some(settlement_id)
    );
}
