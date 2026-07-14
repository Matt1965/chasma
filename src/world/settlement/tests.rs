//! Settlement treasury integration tests (ADR-093 I7).

use bevy::prelude::{Quat, Vec3};

use super::{
    SettlementId, SettlementOwnership, SettlementRecord, SettlementTreasuryRecord,
    TreasuryAccessPolicy, create_settlement_with_treasury, deposit_gold,
};
use crate::world::building::BuildingInteractionProfileCatalog;
use crate::world::{
    Affiliation, BuildingCatalog, BuildingOwnership, BuildingSource, ChunkCoord, ChunkData,
    ChunkLayout, Heightfield, InventoryCatalogCtx, InventoryProfileCatalog, ItemCatalog,
    ItemCategoryCatalog, LocalPosition, UnitCatalog, UnitDefinitionId, UnitOwnership, UnitSource,
    WorldData, WorldPosition, count_physical_gold, create_building, create_building_with_inventory,
    create_unit_with_inventory, physical_gold_item_id, place_stack_first_fit,
    starter_building_definitions, starter_inventory_profile_definitions,
    starter_item_category_definitions, starter_item_definitions, starter_unit_definitions,
};

fn test_world() -> WorldData {
    let mut world = WorldData::new(ChunkLayout {
        chunk_size_meters: 256.0,
        units_per_meter: 1.0,
    });
    let heightfield = Heightfield::from_samples(65, 4.0, vec![0.0; 65 * 65]).unwrap();
    world.insert(
        crate::world::ChunkId::new(ChunkCoord::new(0, 0)),
        ChunkData::new(heightfield, Vec::new()),
    );
    world
}

struct TreasuryFixture {
    world: WorldData,
    ctx: InventoryCatalogCtx<'static>,
    building_catalog: BuildingCatalog,
    interaction_catalog: BuildingInteractionProfileCatalog,
    unit_id: crate::world::UnitId,
    treasury_id: super::TreasuryId,
    inventory_id: crate::world::InventoryId,
}

fn treasury_fixture() -> TreasuryFixture {
    let categories = Box::leak(Box::new(
        ItemCategoryCatalog::from_definitions(starter_item_category_definitions()).unwrap(),
    ));
    let items = Box::leak(Box::new(
        ItemCatalog::from_definitions(starter_item_definitions(), categories).unwrap(),
    ));
    let profiles = Box::leak(Box::new(
        InventoryProfileCatalog::from_definitions(starter_inventory_profile_definitions()).unwrap(),
    ));
    let building_categories = Box::leak(Box::new(crate::world::BuildingCategoryCatalog::default()));
    let building_catalog = Box::leak(Box::new(
        BuildingCatalog::from_definitions(starter_building_definitions(), building_categories)
            .unwrap(),
    ));
    let interaction_catalog = Box::leak(Box::new(BuildingInteractionProfileCatalog::default()));
    let unit_catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
    let mut world = test_world();
    let ctx = InventoryCatalogCtx::new(items, categories, profiles);
    let unit = create_unit_with_inventory(
        &unit_catalog,
        &mut world,
        &UnitDefinitionId::new("bandit"),
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(1.0, 0.0, 1.0)),
        ),
        UnitSource::Authored,
        UnitOwnership::with_affiliation(Affiliation::Player),
        &ctx,
    )
    .unwrap();
    let inventory_id = unit.inventory_id.unwrap();
    let inventory_id = unit.inventory_id.unwrap();
    let (inventory_store, instance_store) = world.inventory_runtime_mut();
    place_stack_first_fit(
        inventory_store,
        instance_store,
        &ctx,
        inventory_id,
        physical_gold_item_id(),
        20,
    )
    .unwrap();
    let building = create_building(
        building_catalog,
        &mut world,
        &crate::world::BuildingDefinitionId::new("settlement_core"),
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(1.5, 0.0, 1.5)),
        ),
        Quat::IDENTITY,
        BuildingSource::Authored,
        BuildingOwnership::with_affiliation(Affiliation::Player),
        None,
    )
    .unwrap();
    world.mutate_building(building.id, |b| {
        b.lifecycle_state = crate::world::BuildingLifecycleState::Complete;
    });
    let report = create_settlement_with_treasury(
        &mut world,
        building_catalog,
        interaction_catalog,
        building.id,
        "Test Settlement",
        SettlementOwnership::player_default(),
        building.placement.position,
        0,
    )
    .unwrap();
    TreasuryFixture {
        world,
        ctx,
        building_catalog: building_catalog.clone(),
        interaction_catalog: interaction_catalog.clone(),
        unit_id: unit.id,
        treasury_id: report.treasury_id,
        inventory_id,
    }
}

#[test]
fn deposit_success_conserves_total_wealth() {
    let fixture = treasury_fixture();
    let mut world = fixture.world;
    let before_physical =
        count_physical_gold(world.inventory_store().get(fixture.inventory_id).unwrap());
    let before_treasury = world
        .settlement_store()
        .get_treasury(fixture.treasury_id)
        .unwrap()
        .balance_gold;
    let report = deposit_gold(
        &mut world,
        &fixture.building_catalog,
        &fixture.interaction_catalog,
        &fixture.ctx,
        fixture.unit_id,
        fixture.inventory_id,
        fixture.treasury_id,
        8,
        TreasuryAccessPolicy::OwnerOnly,
        1,
    )
    .unwrap();
    assert_eq!(report.deposited_gold, 8);
    let after_physical =
        count_physical_gold(world.inventory_store().get(fixture.inventory_id).unwrap());
    let after_treasury = world
        .settlement_store()
        .get_treasury(fixture.treasury_id)
        .unwrap()
        .balance_gold;
    assert_eq!(before_physical - after_physical, 8);
    assert_eq!(after_treasury - before_treasury, 8);
    assert_eq!(
        u64::from(before_physical) + before_treasury,
        u64::from(after_physical) + after_treasury
    );
}

#[test]
fn deposit_insufficient_gold_is_no_op() {
    let fixture = treasury_fixture();
    let mut world = fixture.world;
    let before_inv = world
        .inventory_store()
        .get(fixture.inventory_id)
        .unwrap()
        .clone();
    let before_treasury = world
        .settlement_store()
        .get_treasury(fixture.treasury_id)
        .unwrap()
        .balance_gold;
    let err = deposit_gold(
        &mut world,
        &fixture.building_catalog,
        &fixture.interaction_catalog,
        &fixture.ctx,
        fixture.unit_id,
        fixture.inventory_id,
        fixture.treasury_id,
        10_000,
        TreasuryAccessPolicy::OwnerOnly,
        1,
    )
    .unwrap_err();
    assert!(matches!(
        err,
        super::TreasuryError::InsufficientPhysicalGold { .. }
    ));
    assert_eq!(
        world.inventory_store().get(fixture.inventory_id).unwrap(),
        &before_inv
    );
    assert_eq!(
        world
            .settlement_store()
            .get_treasury(fixture.treasury_id)
            .unwrap()
            .balance_gold,
        before_treasury
    );
}

#[test]
fn chest_cannot_host_settlement() {
    let categories =
        ItemCategoryCatalog::from_definitions(starter_item_category_definitions()).unwrap();
    let items = ItemCatalog::from_definitions(starter_item_definitions(), &categories).unwrap();
    let profiles =
        InventoryProfileCatalog::from_definitions(starter_inventory_profile_definitions()).unwrap();
    let ctx = InventoryCatalogCtx::new(&items, &categories, &profiles);
    let building_categories = crate::world::BuildingCategoryCatalog::default();
    let building_catalog =
        BuildingCatalog::from_definitions(starter_building_definitions(), &building_categories)
            .unwrap();
    let interaction_catalog = BuildingInteractionProfileCatalog::default();
    let mut world = test_world();
    let building = create_building_with_inventory(
        &building_catalog,
        &mut world,
        &crate::world::BuildingDefinitionId::new("storage_chest"),
        WorldPosition::new(ChunkCoord::new(0, 0), LocalPosition::new(Vec3::ZERO)),
        Quat::IDENTITY,
        BuildingSource::Authored,
        BuildingOwnership::with_affiliation(Affiliation::Player),
        None,
        &ctx,
    )
    .unwrap();
    world.mutate_building(building.id, |b| {
        b.lifecycle_state = crate::world::BuildingLifecycleState::Complete;
    });
    let err = create_settlement_with_treasury(
        &mut world,
        &building_catalog,
        &interaction_catalog,
        building.id,
        "Illegal",
        SettlementOwnership::player_default(),
        building.placement.position,
        0,
    )
    .unwrap_err();
    assert!(matches!(
        err,
        super::TreasuryError::BuildingNotSettlementCapable(_)
    ));
}

#[test]
fn duplicate_treasury_id_rejected() {
    let fixture = treasury_fixture();
    let store = fixture.world.settlement_store();
    let settlement = store
        .get_settlement(fixture.world.settlement_store().sorted_settlement_ids()[0])
        .unwrap()
        .clone();
    let treasury = store.get_treasury(fixture.treasury_id).unwrap().clone();
    let mut duplicate_store = store.clone();
    let err = duplicate_store
        .insert_settlement(
            SettlementRecord {
                id: SettlementId::new(9999),
                display_name: "Dup".into(),
                treasury_id: fixture.treasury_id,
                anchor_building_id: settlement.anchor_building_id,
                ownership: settlement.ownership,
                interaction_position: settlement.interaction_position,
                created_tick: 0,
            },
            SettlementTreasuryRecord {
                id: fixture.treasury_id,
                settlement_id: SettlementId::new(9999),
                ownership: treasury.ownership,
                balance_gold: 0,
                created_tick: 0,
                metadata: String::new(),
            },
        )
        .unwrap_err();
    assert!(matches!(err, super::TreasuryError::DuplicateTreasuryId(_)));
}

#[test]
fn settlement_store_restore_roundtrip() {
    let fixture = treasury_fixture();
    let store = fixture.world.settlement_store();
    let settlements: Vec<_> = store
        .sorted_settlement_ids()
        .into_iter()
        .filter_map(|id| store.get_settlement(id).cloned())
        .collect();
    let treasuries: Vec<_> = store
        .sorted_treasury_ids()
        .into_iter()
        .filter_map(|id| store.get_treasury(id).cloned())
        .collect();
    let mut restored = crate::world::SettlementStore::default();
    restored
        .restore_snapshot(
            settlements.clone(),
            treasuries.clone(),
            store.next_settlement_id(),
            store.next_treasury_id(),
        )
        .unwrap();
    assert_eq!(
        restored.sorted_settlement_ids(),
        store.sorted_settlement_ids()
    );
    assert_eq!(
        restored
            .get_treasury(fixture.treasury_id)
            .unwrap()
            .balance_gold,
        store
            .get_treasury(fixture.treasury_id)
            .unwrap()
            .balance_gold
    );
}
