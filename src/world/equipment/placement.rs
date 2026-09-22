//! Shared inventory placement validation for equipment and containers.

use crate::world::inventory::{
    InventoryCatalogCtx, InventoryError, InventoryOwnerRef, InventoryRecord, InventoryStore,
    ItemInstanceId, ItemInstanceStore, PlacedInventoryEntry, can_place_entry,
};
use crate::world::{ItemDefinition, ItemDefinitionId};

use super::container::is_container_item;

/// Authoritative placement validation for inventory mutations and transfers.
pub fn validate_item_placement(
    inventory_store: &InventoryStore,
    instance_store: &ItemInstanceStore,
    ctx: &InventoryCatalogCtx<'_>,
    record: &InventoryRecord,
    entry: &PlacedInventoryEntry,
    definition_id: &ItemDefinitionId,
    item_instance_id: Option<ItemInstanceId>,
    exclude_entry: Option<crate::world::EntryIndex>,
) -> Result<(), InventoryError> {
    can_place_entry(record, entry, definition_id, exclude_entry, ctx)?;

    let profile = ctx.require_profile(record.profile_id())?;
    if let Some(max_entries) = profile.max_placed_entries {
        let occupied = record
            .placed_entries()
            .iter()
            .enumerate()
            .filter(|(index, _)| exclude_entry != Some(*index))
            .count();
        if occupied >= usize::from(max_entries) {
            return Err(InventoryError::MaxPlacedEntriesExceeded {
                inventory_id: record.id(),
                max_entries,
            });
        }
    }

    let item = ctx.require_item(definition_id)?;
    if let Some(slot) = profile.equipment_slot {
        if !item.equipment_slots.contains(&slot) {
            return Err(InventoryError::IncompatibleEquipmentSlot {
                inventory_id: record.id(),
                slot,
                item_definition_id: definition_id.clone(),
            });
        }
    }

    validate_itemcontainer_internal_placement(inventory_store, instance_store, record)?;

    validate_container_destination_rules(
        inventory_store,
        instance_store,
        record,
        item,
        item_instance_id,
    )?;

    Ok(())
}

/// Containers in a unit's personal inventory must remain empty.
fn validate_itemcontainer_internal_placement(
    inventory_store: &InventoryStore,
    instance_store: &ItemInstanceStore,
    record: &InventoryRecord,
) -> Result<(), InventoryError> {
    let InventoryOwnerRef::ItemContainer(container_instance_id) = record.owner() else {
        return Ok(());
    };
    let Some((parent_inventory_id, _)) = instance_store.inventory_location(*container_instance_id)
    else {
        return Ok(());
    };
    let parent = inventory_store
        .get(parent_inventory_id)
        .ok_or(InventoryError::InventoryNotFound(parent_inventory_id))?;
    if matches!(parent.owner(), InventoryOwnerRef::Unit(_)) {
        return Err(
            InventoryError::ContainerLoadingForbiddenInPersonalInventory {
                unit_id: match parent.owner() {
                    InventoryOwnerRef::Unit(unit_id) => *unit_id,
                    _ => unreachable!(),
                },
                container_instance_id: *container_instance_id,
                inventory_id: record.id(),
            },
        );
    }
    Ok(())
}

fn validate_container_destination_rules(
    inventory_store: &InventoryStore,
    instance_store: &ItemInstanceStore,
    record: &InventoryRecord,
    item: &ItemDefinition,
    item_instance_id: Option<ItemInstanceId>,
) -> Result<(), InventoryError> {
    if !is_container_item(item) {
        return Ok(());
    }

    match record.owner() {
        InventoryOwnerRef::Unit(unit_id) => {
            if container_instance_is_loaded(inventory_store, instance_store, item_instance_id)? {
                return Err(InventoryError::LoadedContainerInPersonalInventory {
                    unit_id: *unit_id,
                    item_definition_id: item.id.clone(),
                });
            }
        }
        InventoryOwnerRef::ItemContainer(_) => {
            return Err(InventoryError::ContainerRecursionForbidden {
                item_definition_id: item.id.clone(),
            });
        }
        _ => {}
    }

    Ok(())
}

fn container_instance_is_loaded(
    inventory_store: &InventoryStore,
    instance_store: &ItemInstanceStore,
    item_instance_id: Option<ItemInstanceId>,
) -> Result<bool, InventoryError> {
    let Some(instance_id) = item_instance_id else {
        return Ok(false);
    };
    let Some(instance) = instance_store.get(instance_id) else {
        return Err(InventoryError::ItemInstanceNotFound(instance_id));
    };
    let Some(container_inventory_id) = instance.contained_inventory_id else {
        return Ok(false);
    };
    let container = inventory_store
        .get(container_inventory_id)
        .ok_or(InventoryError::InventoryNotFound(container_inventory_id))?;
    Ok(!container.placed_entries().is_empty())
}

