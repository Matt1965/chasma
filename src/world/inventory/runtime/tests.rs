//! Integration tests for authoritative inventory runtime (ADR-088 I2).

use std::sync::OnceLock;

use super::*;
use crate::world::{
    InventoryProfileCatalog, InventoryProfileId, ItemCatalog, ItemCategoryCatalog,
    ItemDefinitionId, starter_inventory_profile_definitions, starter_item_category_definitions,
    starter_item_definitions,
};

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

struct TestHarness {
    inventory_store: InventoryStore,
    instance_store: ItemInstanceStore,
}

impl TestHarness {
    fn new() -> Self {
        Self {
            inventory_store: InventoryStore::default(),
            instance_store: ItemInstanceStore::default(),
        }
    }

    fn create_backpack(&mut self) -> InventoryId {
        create_inventory(
            &mut self.inventory_store,
            test_ctx(),
            InventoryProfileId::new("unit_backpack_standard"),
            InventoryOwnerRef::Detached,
        )
        .unwrap()
    }
}

#[test]
fn ids_allocate_monotonically_from_one() {
    let mut store = InventoryStore::default();
    let first = store.allocate_inventory_id();
    let second = store.allocate_inventory_id();
    assert_eq!(first.raw(), 1);
    assert_eq!(second.raw(), 2);
    assert!(!InventoryId::INVALID.is_valid());

    let mut instances = ItemInstanceStore::default();
    let a = instances.allocate_item_instance_id();
    let b = instances.allocate_item_instance_id();
    assert_eq!(a.raw(), 1);
    assert_eq!(b.raw(), 2);
}

#[test]
fn grid_rejects_overlap_and_out_of_bounds() {
    let mut harness = TestHarness::new();
    let inventory_id = harness.create_backpack();
    let ctx = test_ctx();
    place_stack(
        &mut harness.inventory_store,
        &harness.instance_store,
        &ctx,
        inventory_id,
        ItemDefinitionId::new("iron_ore"),
        5,
        0,
        0,
    )
    .unwrap();
    let err = place_stack(
        &mut harness.inventory_store,
        &harness.instance_store,
        &ctx,
        inventory_id,
        ItemDefinitionId::new("iron_ore"),
        1,
        1,
        1,
    )
    .unwrap_err();
    assert!(matches!(err, InventoryError::CellsOccupied { .. }));

    let err = place_stack(
        &mut harness.inventory_store,
        &harness.instance_store,
        &ctx,
        inventory_id,
        ItemDefinitionId::new("iron_ore"),
        1,
        5,
        5,
    )
    .unwrap_err();
    assert!(matches!(err, InventoryError::GridOutOfBounds { .. }));
}

#[test]
fn unique_item_uses_definition_footprint() {
    let mut harness = TestHarness::new();
    let inventory_id = harness.create_backpack();
    let ctx = test_ctx();
    let instance_id = create_item_instance(
        &mut harness.instance_store,
        &ctx,
        ItemDefinitionId::new("healing_kit"),
        ItemInstanceMetadata::default(),
    )
    .unwrap();
    place_unique(
        &mut harness.inventory_store,
        &mut harness.instance_store,
        &ctx,
        inventory_id,
        instance_id,
        0,
        0,
    )
    .unwrap();
    let record = harness.inventory_store.get(inventory_id).unwrap();
    assert_eq!(record.entry_at_cell(1, 1), Some(0));
    assert_eq!(record.entry_at_cell(2, 0), None);
}

#[test]
fn stack_limit_enforced_on_place() {
    let mut harness = TestHarness::new();
    let inventory_id = harness.create_backpack();
    let ctx = test_ctx();
    let err = place_stack(
        &mut harness.inventory_store,
        &harness.instance_store,
        &ctx,
        inventory_id,
        ItemDefinitionId::new("gold"),
        1000,
        0,
        0,
    )
    .unwrap_err();
    assert!(matches!(
        err,
        InventoryError::InvalidStackQuantity { .. } | InventoryError::StackLimitExceeded { .. }
    ));
}

#[test]
fn merge_moves_partial_and_keeps_remainder_in_source() {
    let mut harness = TestHarness::new();
    let inventory_id = harness.create_backpack();
    let ctx = test_ctx();
    let dest = place_stack(
        &mut harness.inventory_store,
        &harness.instance_store,
        &ctx,
        inventory_id,
        ItemDefinitionId::new("gold"),
        10,
        0,
        0,
    )
    .unwrap();
    let src = place_stack(
        &mut harness.inventory_store,
        &harness.instance_store,
        &ctx,
        inventory_id,
        ItemDefinitionId::new("gold"),
        20,
        1,
        0,
    )
    .unwrap();
    let outcome = merge_stacks(
        &mut harness.inventory_store,
        &mut harness.instance_store,
        ctx,
        inventory_id,
        src,
        dest,
    )
    .unwrap();
    assert_eq!(outcome.merged, 20);
    assert_eq!(outcome.remaining_in_source, 0);
}

#[test]
fn split_half_uses_ceiling_and_one_item_moves_whole_stack() {
    let mut harness = TestHarness::new();
    let inventory_id = harness.create_backpack();
    let ctx = test_ctx();
    let source = place_stack(
        &mut harness.inventory_store,
        &harness.instance_store,
        &ctx,
        inventory_id,
        ItemDefinitionId::new("gold"),
        9,
        0,
        0,
    )
    .unwrap();
    let outcome = split_stack_half(
        &mut harness.inventory_store,
        &harness.instance_store,
        &ctx,
        inventory_id,
        source,
    )
    .unwrap();
    assert_eq!(outcome.moved, 5);
    assert_eq!(outcome.source_remaining, 4);

    let inventory_id = harness.create_backpack();
    let source = place_stack(
        &mut harness.inventory_store,
        &harness.instance_store,
        &ctx,
        inventory_id,
        ItemDefinitionId::new("gold"),
        1,
        0,
        0,
    )
    .unwrap();
    let outcome = split_stack_half(
        &mut harness.inventory_store,
        &harness.instance_store,
        &ctx,
        inventory_id,
        source,
    )
    .unwrap();
    assert_eq!(outcome.moved, 1);
    assert_eq!(outcome.source_remaining, 0);
}

#[test]
fn unique_instance_cannot_be_duplicated_in_store() {
    let mut harness = TestHarness::new();
    let ctx = test_ctx();
    let instance_id = create_item_instance(
        &mut harness.instance_store,
        &ctx,
        ItemDefinitionId::new("healing_kit"),
        ItemInstanceMetadata { quality: Some(3) },
    )
    .unwrap();
    let duplicate = ItemInstance::new(instance_id, ItemDefinitionId::new("healing_kit"));
    let err = harness.instance_store.insert(duplicate).unwrap_err();
    assert!(matches!(err, InventoryError::DuplicateItemInstance(_)));
    assert_eq!(
        harness
            .instance_store
            .get(instance_id)
            .unwrap()
            .metadata
            .quality,
        Some(3)
    );
}

#[test]
fn weight_tracks_exact_mass_and_allows_over_reference() {
    let mut harness = TestHarness::new();
    let inventory_id = harness.create_backpack();
    let ctx = test_ctx();
    for _ in 0..8 {
        let _ = place_stack_first_fit(
            &mut harness.inventory_store,
            &harness.instance_store,
            &ctx,
            inventory_id,
            ItemDefinitionId::new("iron_ore"),
            50,
        );
    }
    let record = harness.inventory_store.get(inventory_id).unwrap();
    assert_eq!(record.total_mass_grams(), 8 * 50 * 2_000);
    let weight = query_inventory_weight(record, &ctx).unwrap();
    assert!(weight.over_reference_grams > 0);
    validate_inventory(
        &harness.inventory_store,
        &harness.instance_store,
        &ctx,
        inventory_id,
    )
    .unwrap();
}

#[test]
fn failed_move_preserves_state() {
    let mut harness = TestHarness::new();
    let inventory_id = harness.create_backpack();
    let ctx = test_ctx();
    let a = place_stack(
        &mut harness.inventory_store,
        &harness.instance_store,
        &ctx,
        inventory_id,
        ItemDefinitionId::new("iron_ore"),
        1,
        0,
        0,
    )
    .unwrap();
    let _ = place_stack(
        &mut harness.inventory_store,
        &harness.instance_store,
        &ctx,
        inventory_id,
        ItemDefinitionId::new("iron_ore"),
        1,
        2,
        0,
    )
    .unwrap();
    let before = harness.inventory_store.get(inventory_id).unwrap().clone();
    let err = move_entry(
        &mut harness.inventory_store,
        &harness.instance_store,
        &ctx,
        inventory_id,
        a,
        1,
        0,
    )
    .unwrap_err();
    assert!(matches!(err, InventoryError::CellsOccupied { .. }));
    assert_eq!(harness.inventory_store.get(inventory_id).unwrap(), &before);
}

#[test]
fn swap_different_footprints_succeeds_when_both_fit() {
    let mut harness = TestHarness::new();
    let inventory_id = harness.create_backpack();
    let ctx = test_ctx();
    let stack = place_stack(
        &mut harness.inventory_store,
        &harness.instance_store,
        &ctx,
        inventory_id,
        ItemDefinitionId::new("gold"),
        5,
        0,
        0,
    )
    .unwrap();
    let instance_id = create_item_instance(
        &mut harness.instance_store,
        &ctx,
        ItemDefinitionId::new("healing_kit"),
        ItemInstanceMetadata::default(),
    )
    .unwrap();
    let unique = place_unique(
        &mut harness.inventory_store,
        &mut harness.instance_store,
        &ctx,
        inventory_id,
        instance_id,
        2,
        0,
    )
    .unwrap();
    swap_entries(
        &mut harness.inventory_store,
        &mut harness.instance_store,
        ctx,
        inventory_id,
        stack,
        unique,
    )
    .unwrap();
    let record = harness.inventory_store.get(inventory_id).unwrap();
    assert_eq!(record.placed_entries()[stack].anchor_x, 2);
    assert_eq!(record.placed_entries()[unique].anchor_x, 0);
}

#[test]
fn auto_sort_is_deterministic_and_merges_stacks() {
    let mut harness = TestHarness::new();
    let inventory_id = harness.create_backpack();
    let ctx = test_ctx();
    place_stack(
        &mut harness.inventory_store,
        &harness.instance_store,
        &ctx,
        inventory_id,
        ItemDefinitionId::new("gold"),
        5,
        3,
        3,
    )
    .unwrap();
    place_stack(
        &mut harness.inventory_store,
        &harness.instance_store,
        &ctx,
        inventory_id,
        ItemDefinitionId::new("gold"),
        7,
        0,
        2,
    )
    .unwrap();
    auto_sort(
        &mut harness.inventory_store,
        &mut harness.instance_store,
        &ctx,
        inventory_id,
    )
    .unwrap();
    let first_layout = harness.inventory_store.get(inventory_id).unwrap().clone();
    auto_sort(
        &mut harness.inventory_store,
        &mut harness.instance_store,
        &ctx,
        inventory_id,
    )
    .unwrap();
    let second_layout = harness.inventory_store.get(inventory_id).unwrap();
    assert_eq!(
        first_layout.placed_entries(),
        second_layout.placed_entries()
    );
    let gold_stacks: u32 = second_layout
        .placed_entries()
        .iter()
        .filter_map(|entry| match &entry.contents {
            InventoryEntryContents::Stack { quantity, .. } => Some(*quantity),
            _ => None,
        })
        .sum();
    assert_eq!(gold_stacks, 12);
}

#[test]
fn auto_sort_failure_rolls_back() {
    let mut harness = TestHarness::new();
    let inventory_id = harness.create_backpack();
    let ctx = test_ctx();
    place_stack(
        &mut harness.inventory_store,
        &harness.instance_store,
        &ctx,
        inventory_id,
        ItemDefinitionId::new("gold"),
        4,
        0,
        0,
    )
    .unwrap();
    let before = harness.inventory_store.get(inventory_id).unwrap().clone();
    let empty_items =
        ItemCatalog::from_definitions(Vec::new(), &ItemCategoryCatalog::default()).unwrap();
    let empty_items = Box::leak(Box::new(empty_items));
    let bad_ctx = InventoryCatalogCtx::new(empty_items, test_ctx().categories, test_ctx().profiles);
    let record = harness.inventory_store.get_mut(inventory_id).unwrap();
    let err = auto_sort_inventory(record, &bad_ctx, &mut harness.instance_store).unwrap_err();
    assert!(matches!(err, InventoryError::ItemDefinitionNotFound(_)));
    assert_eq!(
        harness
            .inventory_store
            .get(inventory_id)
            .unwrap()
            .placed_entries(),
        before.placed_entries()
    );
}

#[test]
fn migration_splits_oversized_stacks_and_returns_leftovers() {
    let mut harness = TestHarness::new();
    let inventory_id = harness.create_backpack();
    let ctx = test_ctx();
    place_stack(
        &mut harness.inventory_store,
        &harness.instance_store,
        &ctx,
        inventory_id,
        ItemDefinitionId::new("gold"),
        999,
        0,
        0,
    )
    .unwrap();
    let result = migrate_inventory_profile_with_leftovers(
        &mut harness.inventory_store,
        &mut harness.instance_store,
        &ctx,
        inventory_id,
        InventoryProfileId::new("unit_backpack_small"),
    )
    .unwrap();
    let in_inventory: u32 = harness
        .inventory_store
        .get(inventory_id)
        .unwrap()
        .placed_entries()
        .iter()
        .filter_map(|entry| match &entry.contents {
            InventoryEntryContents::Stack { quantity, .. } => Some(*quantity),
            _ => None,
        })
        .sum();
    let in_leftovers: u32 = result
        .leftovers
        .iter()
        .filter_map(|leftover| match &leftover.contents {
            InventoryEntryContents::Stack { quantity, .. } => Some(*quantity),
            _ => None,
        })
        .sum();
    assert_eq!(in_inventory + in_leftovers, 999);
}

#[test]
fn store_invariants_hold_after_operations() {
    let mut harness = TestHarness::new();
    let inventory_id = harness.create_backpack();
    let ctx = test_ctx();
    let instance_id = create_item_instance(
        &mut harness.instance_store,
        &ctx,
        ItemDefinitionId::new("healing_kit"),
        ItemInstanceMetadata::default(),
    )
    .unwrap();
    place_unique_first_fit(
        &mut harness.inventory_store,
        &mut harness.instance_store,
        &ctx,
        inventory_id,
        instance_id,
    )
    .unwrap();
    place_stack_first_fit(
        &mut harness.inventory_store,
        &harness.instance_store,
        &ctx,
        inventory_id,
        ItemDefinitionId::new("gold"),
        25,
    )
    .unwrap();
    let report = validate_inventory_stores(&harness.inventory_store, &harness.instance_store, &ctx);
    assert!(report.is_ok(), "{report:?}");
}

#[test]
fn remove_entry_updates_mass_and_cells() {
    let mut harness = TestHarness::new();
    let inventory_id = harness.create_backpack();
    let ctx = test_ctx();
    let entry = place_stack(
        &mut harness.inventory_store,
        &harness.instance_store,
        &ctx,
        inventory_id,
        ItemDefinitionId::new("iron_ore"),
        3,
        0,
        0,
    )
    .unwrap();
    let mass_before = harness
        .inventory_store
        .get(inventory_id)
        .unwrap()
        .total_mass_grams();
    remove_entry(
        &mut harness.inventory_store,
        &mut harness.instance_store,
        &ctx,
        inventory_id,
        entry,
    )
    .unwrap();
    let record = harness.inventory_store.get(inventory_id).unwrap();
    assert_eq!(record.total_mass_grams(), 0);
    assert!(record.cell_owner().iter().all(|cell| cell.is_none()));
    assert!(mass_before > 0);
}
