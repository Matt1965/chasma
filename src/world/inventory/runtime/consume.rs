//! Physical gold helpers and inventory consumption (ADR-093 I7).

use super::catalog_ctx::InventoryCatalogCtx;
use super::entry::{EntryIndex, InventoryEntryContents};
use super::error::InventoryError;
use super::id::InventoryId;
use super::record::InventoryRecord;
use super::store::{InventoryStore, ItemInstanceStore};
use crate::world::ItemDefinitionId;

/// Canonical physical gold item id (ADR-087 I1).
pub fn physical_gold_item_id() -> ItemDefinitionId {
    ItemDefinitionId::new("gold")
}

pub fn count_physical_gold(record: &InventoryRecord) -> u32 {
    let gold_id = physical_gold_item_id();
    record
        .placed_entries()
        .iter()
        .filter_map(|entry| match &entry.contents {
            InventoryEntryContents::Stack {
                item_definition_id,
                quantity,
            } if item_definition_id == &gold_id => Some(*quantity),
            _ => None,
        })
        .sum()
}

/// Remove stackable item quantity from an inventory atomically.
pub fn consume_stack_item(
    inventory_store: &mut InventoryStore,
    instance_store: &mut ItemInstanceStore,
    ctx: &InventoryCatalogCtx<'_>,
    inventory_id: InventoryId,
    item_definition_id: &ItemDefinitionId,
    quantity: u32,
) -> Result<u32, InventoryError> {
    if quantity == 0 {
        return Ok(0);
    }
    let backup = inventory_store
        .get(inventory_id)
        .ok_or(InventoryError::InventoryNotFound(inventory_id))?
        .clone();

    let mut remaining = quantity;
    let mut indices_to_remove: Vec<EntryIndex> = Vec::new();
    {
        let record = inventory_store
            .get_mut(inventory_id)
            .ok_or(InventoryError::InventoryNotFound(inventory_id))?;
        for (index, entry) in record.placed_entries_mut().iter_mut().enumerate() {
            if remaining == 0 {
                break;
            }
            if let InventoryEntryContents::Stack {
                item_definition_id: stack_id,
                quantity: stack_qty,
            } = &mut entry.contents
            {
                if stack_id != item_definition_id {
                    continue;
                }
                let take = (*stack_qty).min(remaining);
                *stack_qty -= take;
                remaining -= take;
                if *stack_qty == 0 {
                    indices_to_remove.push(index);
                }
            }
        }
    }

    for index in indices_to_remove.into_iter().rev() {
        super::ops::remove_entry(inventory_store, instance_store, ctx, inventory_id, index)?;
    }

    let consumed = quantity - remaining;
    if consumed == 0 {
        return Ok(0);
    }

    if let Err(error) = {
        let record = inventory_store
            .get_mut(inventory_id)
            .ok_or(InventoryError::InventoryNotFound(inventory_id))?;
        super::ops::rebuild_inventory(record, ctx, instance_store)
    } {
        if let Some(record) = inventory_store.get_mut(inventory_id) {
            *record = backup;
        }
        return Err(error);
    }

    Ok(consumed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        InventoryOwnerRef, InventoryProfileCatalog, ItemCatalog, ItemCategoryCatalog,
        create_inventory, place_stack_first_fit, starter_inventory_profile_definitions,
        starter_item_category_definitions, starter_item_definitions,
    };

    fn ctx<'a>(
        items: &'a ItemCatalog,
        categories: &'a ItemCategoryCatalog,
        profiles: &'a InventoryProfileCatalog,
    ) -> InventoryCatalogCtx<'a> {
        InventoryCatalogCtx::new(items, categories, profiles)
    }

    #[test]
    fn consume_physical_gold_reduces_stacks() {
        let categories =
            ItemCategoryCatalog::from_definitions(starter_item_category_definitions()).unwrap();
        let items = ItemCatalog::from_definitions(starter_item_definitions(), &categories).unwrap();
        let profiles =
            InventoryProfileCatalog::from_definitions(starter_inventory_profile_definitions())
                .unwrap();
        let catalog = ctx(&items, &categories, &profiles);
        let mut inventory_store = InventoryStore::default();
        let mut instance_store = ItemInstanceStore::default();
        let inventory_id = create_inventory(
            &mut inventory_store,
            &catalog,
            crate::world::InventoryProfileId::new("unit_backpack_standard"),
            InventoryOwnerRef::Detached,
        )
        .unwrap();
        place_stack_first_fit(
            &mut inventory_store,
            &instance_store,
            &catalog,
            inventory_id,
            physical_gold_item_id(),
            10,
        )
        .unwrap();
        place_stack_first_fit(
            &mut inventory_store,
            &instance_store,
            &catalog,
            inventory_id,
            physical_gold_item_id(),
            5,
        )
        .unwrap();
        let consumed = consume_stack_item(
            &mut inventory_store,
            &mut instance_store,
            &catalog,
            inventory_id,
            &physical_gold_item_id(),
            7,
        )
        .unwrap();
        assert_eq!(consumed, 7);
        let record = inventory_store.get(inventory_id).unwrap();
        assert_eq!(count_physical_gold(record), 8);
    }
}
