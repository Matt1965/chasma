//! Integration tests for world item piles and drop/pickup (ADR-090 I4).

use std::sync::OnceLock;

use super::*;
use crate::world::inventory::{
    InventoryCatalogCtx, InventoryOwnerRef, InventoryStore, ItemInstanceMetadata,
    ItemInstanceStore, TransferPlacementPolicy, create_inventory, create_item_instance,
    place_stack_first_fit, transfer_entry_full, transfer_half, transfer_one,
    transfer_stack_quantity,
};
use crate::world::{
    Affiliation, ChunkCoord, ChunkData, ChunkId, ChunkLayout, CorpseSettings, Heightfield,
    InventoryProfileCatalog, ItemCatalog, ItemCategoryCatalog, ItemDefinitionId, LocalPosition,
    SpaceId, UnitCatalog, UnitDefinitionId, UnitOwnership, UnitSource, WorldData, WorldPosition,
    loot_corpse_entry, starter_inventory_profile_definitions, starter_item_category_definitions,
    starter_item_definitions, starter_unit_definitions, step_unit_death_pipeline,
};
use bevy::prelude::Vec3;

fn test_ctx() -> &'static InventoryCatalogCtx<'static> {
    static CTX: OnceLock<InventoryCatalogCtx<'static>> = OnceLock::new();
    CTX.get_or_init(|| {
        let categories =
            ItemCategoryCatalog::from_definitions(starter_item_category_definitions()).unwrap();
        let items = ItemCatalog::from_definitions(starter_item_definitions(), &categories).unwrap();
        let profiles =
            InventoryProfileCatalog::from_definitions(starter_inventory_profile_definitions())
                .unwrap();
        let items = Box::leak(Box::new(items));
        let categories = Box::leak(Box::new(categories));
        let profiles = Box::leak(Box::new(profiles));
        InventoryCatalogCtx::new(items, categories, profiles)
    })
}

fn flat_world() -> WorldData {
    let mut world = WorldData::new(ChunkLayout {
        chunk_size_meters: 256.0,
        units_per_meter: 1.0,
    });
    let heightfield = Heightfield::from_samples(65, 4.0, vec![0.0; 65 * 65]).unwrap();
    world.insert(
        ChunkId::new(ChunkCoord::new(0, 0)),
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

fn pile_ownership() -> PileOwnership {
    PileOwnership {
        owner_id: None,
        team_id: None,
        affiliation: Affiliation::Player,
    }
}

#[test]
fn drop_stack_creates_pile() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let settings = ItemPileSettings::default();
    let inventory_id = create_inventory(
        world.inventory_store_mut(),
        ctx,
        crate::world::InventoryProfileId::new("unit_backpack_standard"),
        InventoryOwnerRef::Detached,
    )
    .unwrap();
    let (inventory_store, instance_store) = world.inventory_runtime_mut();
    place_stack_first_fit(
        inventory_store,
        instance_store,
        ctx,
        inventory_id,
        ItemDefinitionId::new("iron_ore"),
        10,
    )
    .unwrap();

    let report = drop_stack_from_inventory(
        &mut world,
        ctx,
        &settings,
        inventory_id,
        0,
        10,
        pos(5.0, 5.0),
        SpaceId::SURFACE,
        pile_ownership(),
        1,
    )
    .unwrap();
    assert_eq!(report.removed_from_inventory, 10);
    assert_eq!(report.created_pile_ids.len(), 1);
    let pile_id = report.created_pile_ids[0];
    let pile = world.item_pile_store().get(pile_id).unwrap();
    assert_eq!(pile.stack_quantity(), Some(10));
}

#[test]
fn compatible_pile_merge_is_deterministic() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let settings = ItemPileSettings::default();
    let ownership = pile_ownership();
    let drop = |world: &mut WorldData, x: f32| {
        let inventory_id = create_inventory(
            world.inventory_store_mut(),
            ctx,
            crate::world::InventoryProfileId::new("unit_backpack_standard"),
            InventoryOwnerRef::Detached,
        )
        .unwrap();
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        place_stack_first_fit(
            inventory_store,
            instance_store,
            ctx,
            inventory_id,
            ItemDefinitionId::new("gold"),
            5,
        )
        .unwrap();
        drop_stack_from_inventory(
            world,
            ctx,
            &settings,
            inventory_id,
            0,
            5,
            pos(x, 5.0),
            SpaceId::SURFACE,
            ownership,
            1,
        )
        .unwrap()
    };

    let first = drop(&mut world, 1.0);
    let second = drop(&mut world, 1.1);
    assert_eq!(first.created_pile_ids.len(), 1);
    assert!(second.created_pile_ids.is_empty());
    assert_eq!(second.merged_into_existing_piles, 5);
    assert_eq!(world.item_pile_store().sorted_item_pile_ids().len(), 1);
}

#[test]
fn overflow_creates_multiple_piles_with_offsets() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let settings = ItemPileSettings::default();
    let inventory_id = create_inventory(
        world.inventory_store_mut(),
        ctx,
        crate::world::InventoryProfileId::new("unit_backpack_standard"),
        InventoryOwnerRef::Detached,
    )
    .unwrap();
    let (inventory_store, instance_store) = world.inventory_runtime_mut();
    place_stack_first_fit(
        inventory_store,
        instance_store,
        ctx,
        inventory_id,
        ItemDefinitionId::new("gold"),
        999,
    )
    .unwrap();
    place_stack_first_fit(
        inventory_store,
        instance_store,
        ctx,
        inventory_id,
        ItemDefinitionId::new("gold"),
        500,
    )
    .unwrap();

    let first = drop_stack_from_inventory(
        &mut world,
        ctx,
        &settings,
        inventory_id,
        0,
        999,
        pos(10.0, 10.0),
        SpaceId::SURFACE,
        pile_ownership(),
        1,
    )
    .unwrap();
    let second = drop_stack_from_inventory(
        &mut world,
        ctx,
        &settings,
        inventory_id,
        0,
        500,
        pos(10.0, 10.0),
        SpaceId::SURFACE,
        pile_ownership(),
        1,
    )
    .unwrap();
    assert_eq!(first.created_pile_ids.len(), 1);
    assert!(!second.created_pile_ids.is_empty());
    assert!(world.item_pile_store().sorted_item_pile_ids().len() >= 2);
}

#[test]
fn full_pickup_removes_pile() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let settings = ItemPileSettings::default();
    let source = create_inventory(
        world.inventory_store_mut(),
        ctx,
        crate::world::InventoryProfileId::new("unit_backpack_standard"),
        InventoryOwnerRef::Detached,
    )
    .unwrap();
    let dest = create_inventory(
        world.inventory_store_mut(),
        ctx,
        crate::world::InventoryProfileId::new("unit_backpack_standard"),
        InventoryOwnerRef::Detached,
    )
    .unwrap();
    let (inventory_store, instance_store) = world.inventory_runtime_mut();
    place_stack_first_fit(
        inventory_store,
        instance_store,
        ctx,
        source,
        ItemDefinitionId::new("iron_ore"),
        8,
    )
    .unwrap();
    let drop = drop_stack_from_inventory(
        &mut world,
        ctx,
        &settings,
        source,
        0,
        8,
        pos(3.0, 3.0),
        SpaceId::SURFACE,
        pile_ownership(),
        1,
    )
    .unwrap();
    let pile_id = drop.created_pile_ids[0];

    let pickup = pickup_pile_into_inventory(
        &mut world,
        ctx,
        pile_id,
        dest,
        None,
        None,
        None,
        Affiliation::Player,
    )
    .unwrap();
    assert!(pickup.pile_removed);
    assert_eq!(pickup.transfer.moved, 8);
    assert!(world.item_pile_store().get(pile_id).is_none());
}

#[test]
fn partial_pickup_reduces_stack() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let settings = ItemPileSettings::default();
    let source = create_inventory(
        world.inventory_store_mut(),
        ctx,
        crate::world::InventoryProfileId::new("unit_backpack_standard"),
        InventoryOwnerRef::Detached,
    )
    .unwrap();
    let dest = create_inventory(
        world.inventory_store_mut(),
        ctx,
        crate::world::InventoryProfileId::new("unit_backpack_standard"),
        InventoryOwnerRef::Detached,
    )
    .unwrap();
    let (inventory_store, instance_store) = world.inventory_runtime_mut();
    place_stack_first_fit(
        inventory_store,
        instance_store,
        ctx,
        source,
        ItemDefinitionId::new("gold"),
        9,
    )
    .unwrap();
    let drop = drop_stack_from_inventory(
        &mut world,
        ctx,
        &settings,
        source,
        0,
        9,
        pos(4.0, 4.0),
        SpaceId::SURFACE,
        pile_ownership(),
        1,
    )
    .unwrap();
    let pile_id = drop.created_pile_ids[0];

    let pickup = pickup_pile_into_inventory(
        &mut world,
        ctx,
        pile_id,
        dest,
        Some(4),
        None,
        None,
        Affiliation::Player,
    )
    .unwrap();
    assert!(!pickup.pile_removed);
    assert_eq!(pickup.pile_remaining_quantity, Some(5));
    assert_eq!(pickup.transfer.moved, 4);
}

#[test]
fn unauthorized_pickup_rejected() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let settings = ItemPileSettings::default();
    let inventory_id = create_inventory(
        world.inventory_store_mut(),
        ctx,
        crate::world::InventoryProfileId::new("unit_backpack_standard"),
        InventoryOwnerRef::Detached,
    )
    .unwrap();
    let (inventory_store, instance_store) = world.inventory_runtime_mut();
    place_stack_first_fit(
        inventory_store,
        instance_store,
        ctx,
        inventory_id,
        ItemDefinitionId::new("gold"),
        3,
    )
    .unwrap();
    let drop = drop_stack_from_inventory(
        &mut world,
        ctx,
        &settings,
        inventory_id,
        0,
        3,
        pos(6.0, 6.0),
        SpaceId::SURFACE,
        PileOwnership {
            owner_id: Some(crate::world::OwnerId::new(99)),
            team_id: None,
            affiliation: Affiliation::Player,
        },
        1,
    )
    .unwrap();
    let dest = create_inventory(
        world.inventory_store_mut(),
        ctx,
        crate::world::InventoryProfileId::new("unit_backpack_standard"),
        InventoryOwnerRef::Detached,
    )
    .unwrap();
    let err = pickup_pile_into_inventory(
        &mut world,
        ctx,
        drop.created_pile_ids[0],
        dest,
        None,
        None,
        None,
        Affiliation::Player,
    )
    .unwrap_err();
    assert!(matches!(err, ItemPileError::Unauthorized));
}

#[test]
fn pile_survives_chunk_unload() {
    let mut world = flat_world();
    let ctx = test_ctx();
    let settings = ItemPileSettings::default();
    let inventory_id = create_inventory(
        world.inventory_store_mut(),
        ctx,
        crate::world::InventoryProfileId::new("unit_backpack_standard"),
        InventoryOwnerRef::Detached,
    )
    .unwrap();
    let (inventory_store, instance_store) = world.inventory_runtime_mut();
    place_stack_first_fit(
        inventory_store,
        instance_store,
        ctx,
        inventory_id,
        ItemDefinitionId::new("iron_ore"),
        2,
    )
    .unwrap();
    let drop = drop_stack_from_inventory(
        &mut world,
        ctx,
        &settings,
        inventory_id,
        0,
        2,
        pos(7.0, 7.0),
        SpaceId::SURFACE,
        pile_ownership(),
        1,
    )
    .unwrap();
    let pile_id = drop.created_pile_ids[0];
    world.remove(ChunkId::new(ChunkCoord::new(0, 0)));
    assert!(world.get(ChunkId::new(ChunkCoord::new(0, 0))).is_none());
    assert!(world.item_pile_store().get(pile_id).is_some());
}

#[test]
fn corpse_loot_uses_transfer_pipeline() {
    let catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
    let mut world = flat_world();
    let ctx = test_ctx();
    let unit = crate::world::create_unit_with_inventory(
        &catalog,
        &mut world,
        &UnitDefinitionId::new("bandit"),
        pos(8.0, 8.0),
        UnitSource::Authored,
        UnitOwnership::hostile(),
        ctx,
    )
    .unwrap();
    let unit_inventory = unit.inventory_id.unwrap();
    let dest = create_inventory(
        world.inventory_store_mut(),
        ctx,
        crate::world::InventoryProfileId::new("unit_backpack_standard"),
        InventoryOwnerRef::Detached,
    )
    .unwrap();
    let (inventory_store, instance_store) = world.inventory_runtime_mut();
    place_stack_first_fit(
        inventory_store,
        instance_store,
        ctx,
        unit_inventory,
        ItemDefinitionId::new("gold"),
        10,
    )
    .unwrap();
    world.damage_unit(unit.id, 999).unwrap();
    let death = step_unit_death_pipeline(
        &mut world,
        &catalog,
        Some(ctx),
        &CorpseSettings::default(),
        1,
    );
    let corpse_id = death.corpse_ids[0];
    let corpse_inventory = world
        .corpse_store()
        .get(corpse_id)
        .unwrap()
        .inventory_id
        .unwrap();

    let (inventory_store, instance_store) = world.inventory_runtime_mut();
    let report = loot_corpse_entry(
        inventory_store,
        instance_store,
        ctx,
        corpse_inventory,
        0,
        dest,
        None,
        TransferPlacementPolicy::MergeThenFirstFit,
    )
    .unwrap();
    assert_eq!(report.moved, 10);
    assert!(
        world
            .inventory_store()
            .get(corpse_inventory)
            .unwrap()
            .placed_entries()
            .is_empty()
    );
}

#[test]
fn cross_inventory_transfer_one_and_half() {
    let mut harness_inventory = InventoryStore::default();
    let mut instance_store = ItemInstanceStore::default();
    let ctx = test_ctx();
    let source = create_inventory(
        &mut harness_inventory,
        ctx,
        crate::world::InventoryProfileId::new("unit_backpack_standard"),
        InventoryOwnerRef::Detached,
    )
    .unwrap();
    let dest = create_inventory(
        &mut harness_inventory,
        ctx,
        crate::world::InventoryProfileId::new("unit_backpack_standard"),
        InventoryOwnerRef::Detached,
    )
    .unwrap();
    place_stack_first_fit(
        &mut harness_inventory,
        &instance_store,
        ctx,
        source,
        ItemDefinitionId::new("gold"),
        9,
    )
    .unwrap();

    let one = transfer_one(
        &mut harness_inventory,
        &mut instance_store,
        ctx,
        source,
        0,
        dest,
        TransferPlacementPolicy::MergeThenFirstFit,
    )
    .unwrap();
    assert_eq!(one.moved, 1);
    assert_eq!(
        match &harness_inventory.get(source).unwrap().placed_entries()[0].contents {
            crate::world::inventory::InventoryEntryContents::Stack { quantity, .. } => *quantity,
            _ => 0,
        },
        8
    );

    let half = transfer_half(
        &mut harness_inventory,
        &mut instance_store,
        ctx,
        source,
        0,
        dest,
        TransferPlacementPolicy::MergeThenFirstFit,
    )
    .unwrap();
    assert_eq!(half.moved, 4);
}

#[test]
fn destination_full_rejects_atomically() {
    let mut harness_inventory = InventoryStore::default();
    let mut instance_store = ItemInstanceStore::default();
    let ctx = test_ctx();
    let source = create_inventory(
        &mut harness_inventory,
        ctx,
        crate::world::InventoryProfileId::new("unit_backpack_standard"),
        InventoryOwnerRef::Detached,
    )
    .unwrap();
    let dest = create_inventory(
        &mut harness_inventory,
        ctx,
        crate::world::InventoryProfileId::new("unit_backpack_small"),
        InventoryOwnerRef::Detached,
    )
    .unwrap();
    place_stack_first_fit(
        &mut harness_inventory,
        &instance_store,
        ctx,
        source,
        ItemDefinitionId::new("gold"),
        10,
    )
    .unwrap();
    for _ in 0..4 {
        place_stack_first_fit(
            &mut harness_inventory,
            &instance_store,
            ctx,
            dest,
            ItemDefinitionId::new("iron_ore"),
            10,
        )
        .unwrap();
    }
    let source_before = harness_inventory.get(source).unwrap().clone();
    let dest_before = harness_inventory.get(dest).unwrap().clone();
    let err = transfer_entry_full(
        &mut harness_inventory,
        &mut instance_store,
        ctx,
        source,
        0,
        dest,
        TransferPlacementPolicy::MergeThenFirstFit,
    )
    .unwrap_err();
    assert!(matches!(
        err,
        crate::world::inventory::TransferError::DestinationNoFit
            | crate::world::inventory::TransferError::TransferPartialNotAllowed { .. }
    ));
    assert_eq!(harness_inventory.get(source).unwrap(), &source_before);
    assert_eq!(harness_inventory.get(dest).unwrap(), &dest_before);
}

#[test]
fn partial_transfer_only_when_allowed() {
    let mut harness_inventory = InventoryStore::default();
    let mut instance_store = ItemInstanceStore::default();
    let ctx = test_ctx();
    let source = create_inventory(
        &mut harness_inventory,
        ctx,
        crate::world::InventoryProfileId::new("unit_backpack_standard"),
        InventoryOwnerRef::Detached,
    )
    .unwrap();
    let dest = create_inventory(
        &mut harness_inventory,
        ctx,
        crate::world::InventoryProfileId::new("unit_backpack_small"),
        InventoryOwnerRef::Detached,
    )
    .unwrap();
    place_stack_first_fit(
        &mut harness_inventory,
        &instance_store,
        ctx,
        source,
        ItemDefinitionId::new("gold"),
        20,
    )
    .unwrap();
    for _ in 0..4 {
        place_stack_first_fit(
            &mut harness_inventory,
            &instance_store,
            ctx,
            dest,
            ItemDefinitionId::new("iron_ore"),
            10,
        )
        .unwrap();
    }
    let err = transfer_stack_quantity(
        &mut harness_inventory,
        &mut instance_store,
        ctx,
        source,
        0,
        dest,
        20,
        TransferPlacementPolicy::MergeThenFirstFit,
        false,
    )
    .unwrap_err();
    assert!(matches!(
        err,
        crate::world::inventory::TransferError::DestinationNoFit
            | crate::world::inventory::TransferError::TransferPartialNotAllowed { .. }
    ));
}

#[test]
fn unique_item_transfer_preserves_instance() {
    let mut harness_inventory = InventoryStore::default();
    let mut instance_store = ItemInstanceStore::default();
    let ctx = test_ctx();
    let source = create_inventory(
        &mut harness_inventory,
        ctx,
        crate::world::InventoryProfileId::new("unit_backpack_standard"),
        InventoryOwnerRef::Detached,
    )
    .unwrap();
    let dest = create_inventory(
        &mut harness_inventory,
        ctx,
        crate::world::InventoryProfileId::new("unit_backpack_standard"),
        InventoryOwnerRef::Detached,
    )
    .unwrap();
    let instance_id = create_item_instance(
        &mut instance_store,
        ctx,
        ItemDefinitionId::new("healing_kit"),
        ItemInstanceMetadata {
            quality: Some(77),
            ..Default::default()
        },
    )
    .unwrap();
    place_stack_first_fit(
        &mut harness_inventory,
        &instance_store,
        ctx,
        source,
        ItemDefinitionId::new("gold"),
        1,
    )
    .unwrap();
    crate::world::inventory::place_unique_first_fit(
        &mut harness_inventory,
        &mut instance_store,
        ctx,
        source,
        instance_id,
    )
    .unwrap();

    let report = crate::world::inventory::transfer_unique_item(
        &mut harness_inventory,
        &mut instance_store,
        ctx,
        source,
        1,
        instance_id,
        dest,
        TransferPlacementPolicy::MergeThenFirstFit,
    )
    .unwrap();
    assert_eq!(report.moved, 1);
    let instance = instance_store.get(instance_id).unwrap();
    assert_eq!(instance.metadata.quality, Some(77));
    assert!(instance_store.inventory_location(instance_id).is_some());
}
