//! Inventory stress and persistence integration tests (ADR-094 I8).

use std::time::Instant;

use super::{
    InventoryCatalogCtx, InventoryId, InventoryOwnerRef, InventoryProfileCatalog, InventoryRecord,
    auto_sort, merge_stacks, physical_gold_item_id, place_stack_first_fit,
    rebuild_all_inventory_derived, resolve_instance_definition, split_stack_half,
    validate_world_inventory_state,
};
use crate::world::{ItemCatalog, ItemCategoryCatalog};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        starter_inventory_profile_definitions, starter_item_category_definitions,
        starter_item_definitions,
    };

    fn test_ctx() -> InventoryCatalogCtx<'static> {
        let categories = Box::leak(Box::new(
            ItemCategoryCatalog::from_definitions(starter_item_category_definitions()).unwrap(),
        ));
        let items = Box::leak(Box::new(
            ItemCatalog::from_definitions(starter_item_definitions(), categories).unwrap(),
        ));
        let profiles = Box::leak(Box::new(
            InventoryProfileCatalog::from_definitions(starter_inventory_profile_definitions())
                .unwrap(),
        ));
        InventoryCatalogCtx::new(items, categories, profiles)
    }

    fn detached_inventory(ctx: &InventoryCatalogCtx<'_>) -> (InventoryId, crate::world::WorldData) {
        let mut world = crate::world::WorldData::new(crate::world::ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        });
        let id = world.inventory_store_mut().allocate_inventory_id();
        let mut record = InventoryRecord::new(
            id,
            InventoryOwnerRef::Detached,
            crate::world::InventoryProfileId::new("unit_backpack_standard"),
            8,
            8,
        );
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        inventory_store.insert(record).unwrap();
        inventory_store
            .get_mut(id)
            .unwrap()
            .rebuild_derived(ctx, |iid| resolve_instance_definition(instance_store, iid))
            .unwrap();
        (id, world)
    }

    #[test]
    fn stress_many_inventories_validate() {
        let ctx = test_ctx();
        let mut world = crate::world::WorldData::new(crate::world::ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        });
        let started = Instant::now();
        for _ in 0..200 {
            let id = world.inventory_store_mut().allocate_inventory_id();
            let record = InventoryRecord::new(
                id,
                InventoryOwnerRef::Detached,
                crate::world::InventoryProfileId::new("unit_backpack_standard"),
                4,
                4,
            );
            world.inventory_store_mut().insert(record).unwrap();
        }
        let elapsed = started.elapsed();
        let report = validate_world_inventory_state(&world, &ctx);
        assert!(report.is_ok(), "{report:?}");
        assert!(elapsed.as_secs() < 5, "allocator stress took {:?}", elapsed);
    }

    #[test]
    fn stress_many_stacks_split_merge_autosort() {
        let ctx = test_ctx();
        let (inventory_id, mut world) = detached_inventory(&ctx);
        let started = Instant::now();
        for i in 0..50 {
            let (inventory_store, instance_store) = world.inventory_runtime_mut();
            place_stack_first_fit(
                inventory_store,
                instance_store,
                &ctx,
                inventory_id,
                physical_gold_item_id(),
                3 + (i % 5),
            )
            .unwrap();
        }
        for _ in 0..20 {
            let (inventory_store, instance_store) = world.inventory_runtime_mut();
            let _ = split_stack_half(inventory_store, instance_store, &ctx, inventory_id, 0);
        }
        for i in 0..10 {
            if i + 1
                >= world
                    .inventory_store()
                    .get(inventory_id)
                    .unwrap()
                    .placed_entries()
                    .len()
            {
                break;
            }
            let (inventory_store, instance_store) = world.inventory_runtime_mut();
            let _ = merge_stacks(
                inventory_store,
                instance_store,
                &ctx,
                inventory_id,
                0,
                i + 1,
            );
        }
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        auto_sort(inventory_store, instance_store, &ctx, inventory_id).unwrap();
        let elapsed = started.elapsed();
        let report = validate_world_inventory_state(&world, &ctx);
        assert!(report.is_ok(), "{report:?}");
        assert!(elapsed.as_secs() < 10, "mutation stress took {:?}", elapsed);
    }

    #[test]
    fn inventory_store_save_load_roundtrip_preserves_entries() {
        let ctx = test_ctx();
        let (inventory_id, mut world) = detached_inventory(&ctx);
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        place_stack_first_fit(
            inventory_store,
            instance_store,
            &ctx,
            inventory_id,
            physical_gold_item_id(),
            42,
        )
        .unwrap();
        let snapshot: Vec<_> = world
            .inventory_store()
            .sorted_inventory_ids()
            .into_iter()
            .filter_map(|id| world.inventory_store().get(id).cloned())
            .collect();
        let next_id = world.inventory_store().next_id();
        let mut restored = crate::world::InventoryStore::default();
        restored.restore_snapshot(snapshot, next_id).unwrap();
        let mut restore_world = crate::world::WorldData::new(crate::world::ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        });
        *restore_world.inventory_store_mut() = restored;
        crate::world::rebuild_all_inventory_derived(&mut restore_world, &ctx).unwrap();
        let before = world
            .inventory_store()
            .get(inventory_id)
            .unwrap()
            .placed_entries()
            .len();
        let after = restore_world
            .inventory_store()
            .get(inventory_id)
            .unwrap()
            .placed_entries()
            .len();
        assert_eq!(before, after);
        let report = validate_world_inventory_state(&restore_world, &ctx);
        assert!(report.is_ok(), "{report:?}");
    }
}
