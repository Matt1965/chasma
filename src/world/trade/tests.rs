//! Trade pricing and transaction tests.

use super::pricing::{vendor_buy_price_gold, vendor_sell_price_gold};
use super::transaction::{TradeTransferMode, TradeTransferRequest, execute_trade_transfer};
use crate::world::{
    ChunkCoord, ChunkData, ChunkId, ChunkLayout, Heightfield, InventoryCatalogCtx,
    InventoryProfileCatalog, ItemCatalog, ItemCategoryCatalog, ItemDefinitionId, LocalPosition,
    UnitCatalog, UnitDefinitionId, UnitOwnership, UnitSource, WorldData, WorldPosition,
    create_unit_with_inventory, physical_gold_item_id, place_stack_first_fit,
    starter_inventory_profile_definitions, starter_item_category_definitions,
    starter_item_definitions, starter_unit_definitions,
};
use bevy::prelude::Vec3;

#[test]
fn vendor_buy_price_is_half_of_sell_rounded_down() {
    assert_eq!(vendor_sell_price_gold(10), 10);
    assert_eq!(vendor_buy_price_gold(10), 5);
    assert_eq!(vendor_buy_price_gold(11), 5);
}

#[test]
fn buy_deducts_gold_and_moves_stock() {
    let mut world = WorldData::new(ChunkLayout {
        chunk_size_meters: 256.0,
        units_per_meter: 1.0,
    });
    let heightfield = Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
    world.insert(
        ChunkId::new(ChunkCoord::new(0, 0)),
        ChunkData::new(heightfield, Vec::new()),
    );
    let unit_catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
    let categories =
        ItemCategoryCatalog::from_definitions(starter_item_category_definitions()).unwrap();
    let items = ItemCatalog::from_definitions(starter_item_definitions(), &categories).unwrap();
    let profiles =
        InventoryProfileCatalog::from_definitions(starter_inventory_profile_definitions()).unwrap();
    let ctx = InventoryCatalogCtx::new(&items, &categories, &profiles);
    let buyer = create_unit_with_inventory(
        &unit_catalog,
        &crate::world::AppearanceProfileCatalog::empty(),
        &mut world,
        &UnitDefinitionId::new("bandit"),
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(1.0, 0.0, 1.0)),
        ),
        UnitSource::Authored,
        UnitOwnership::player_default(),
        &ctx,
    )
    .unwrap();
    let seller = create_unit_with_inventory(
        &unit_catalog,
        &crate::world::AppearanceProfileCatalog::empty(),
        &mut world,
        &UnitDefinitionId::new("bandit"),
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(3.0, 0.0, 1.0)),
        ),
        UnitSource::Authored,
        UnitOwnership::neutral(),
        &ctx,
    )
    .unwrap();
    let buyer_inv = buyer.inventory_id.expect("buyer");
    let seller_inv = seller.inventory_id.expect("seller");
    let (inventory_store, instance_store) = world.inventory_runtime_mut();
    place_stack_first_fit(
        inventory_store,
        instance_store,
        &ctx,
        buyer_inv,
        physical_gold_item_id(),
        100,
    )
    .unwrap();
    place_stack_first_fit(
        inventory_store,
        instance_store,
        &ctx,
        seller_inv,
        ItemDefinitionId::new("iron_ore"),
        2,
    )
    .unwrap();
    let ore_price = vendor_sell_price_gold(
        items
            .get(&ItemDefinitionId::new("iron_ore"))
            .unwrap()
            .base_value_gold,
    );
    let outcome = execute_trade_transfer(
        &mut world,
        &ctx,
        &items,
        &TradeTransferRequest {
            buyer_unit_id: buyer.id,
            buyer_inventory_id: buyer_inv,
            seller_inventory_id: seller_inv,
            source_inventory_id: seller_inv,
            source_entry_index: 0,
            destination_inventory_id: buyer_inv,
            mode: TradeTransferMode::One,
        },
    )
    .expect("buy");
    assert_eq!(outcome.report.moved, 1);
    assert_eq!(outcome.gold_delta_for_buyer, -(ore_price as i64));
    let gold_after = world
        .inventory_store()
        .get(buyer_inv)
        .map(crate::world::count_physical_gold)
        .unwrap_or(0);
    assert_eq!(gold_after, 100 - ore_price);
}
