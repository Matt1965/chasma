//! Container item instance internal inventory ownership.

use crate::world::inventory::{
    InventoryCatalogCtx, InventoryError, InventoryId, InventoryOwnerRef, InventoryStore,
    ItemInstanceId, ItemInstanceStore,
};
use crate::world::{InventoryProfileId, ItemDefinition};

/// Whether an item definition represents a container (backpack or generic container).
pub fn is_container_item(item: &ItemDefinition) -> bool {
    item.backpack_profile_id.is_some() || item.container_profile_id.is_some()
}

fn container_profile_id(item: &ItemDefinition) -> Option<&InventoryProfileId> {
    item.backpack_profile_id
        .as_ref()
        .or(item.container_profile_id.as_ref())
}

/// Create and attach the persistent internal inventory for a new container instance.
pub fn create_container_inventory(
    inventory_store: &mut InventoryStore,
    instance_store: &mut ItemInstanceStore,
    ctx: &InventoryCatalogCtx<'_>,
    instance_id: ItemInstanceId,
    item: &ItemDefinition,
) -> Result<Option<InventoryId>, InventoryError> {
    let profile_id = container_profile_id(item)
        .ok_or(InventoryError::ProfileNotFound(
            item.backpack_profile_id
                .clone()
                .or_else(|| item.container_profile_id.clone())
                .unwrap_or_else(|| InventoryProfileId::new("missing_container_profile")),
        ))?
        .clone();
    let inventory_id = crate::world::inventory::create_inventory(
        inventory_store,
        ctx,
        profile_id.clone(),
        InventoryOwnerRef::ItemContainer(instance_id),
    )?;
    let instance = instance_store
        .get_mut(instance_id)
        .ok_or(InventoryError::ItemInstanceNotFound(instance_id))?;
    if instance.contained_inventory_id.is_some() {
        return Err(InventoryError::DuplicateContainerInventory(instance_id));
    }
    instance.contained_inventory_id = Some(inventory_id);
    Ok(Some(inventory_id))
}

pub fn container_inventory_is_empty(
    inventory_store: &InventoryStore,
    instance_store: &ItemInstanceStore,
    instance_id: ItemInstanceId,
) -> Result<bool, InventoryError> {
    let Some(instance) = instance_store.get(instance_id) else {
        return Err(InventoryError::ItemInstanceNotFound(instance_id));
    };
    let Some(inventory_id) = instance.contained_inventory_id else {
        return Ok(true);
    };
    let inventory = inventory_store
        .get(inventory_id)
        .ok_or(InventoryError::InventoryNotFound(inventory_id))?;
    Ok(inventory.placed_entries().is_empty())
}

pub fn container_inventory_is_loaded(
    inventory_store: &InventoryStore,
    instance_store: &ItemInstanceStore,
    instance_id: ItemInstanceId,
) -> Result<bool, InventoryError> {
    Ok(!container_inventory_is_empty(
        inventory_store,
        instance_store,
        instance_id,
    )?)
}

/// Remove a container's internal inventory when it is empty.
pub fn release_container_inventory_if_empty(
    inventory_store: &mut InventoryStore,
    instance_store: &mut ItemInstanceStore,
    instance_id: ItemInstanceId,
) -> Result<(), InventoryError> {
    let Some(instance) = instance_store.get(instance_id) else {
        return Err(InventoryError::ItemInstanceNotFound(instance_id));
    };
    let Some(inventory_id) = instance.contained_inventory_id else {
        return Ok(());
    };
    if !container_inventory_is_empty(inventory_store, instance_store, instance_id)? {
        return Err(InventoryError::ContainerNotEmptyOnRelease {
            item_instance_id: instance_id,
            inventory_id,
        });
    }
    inventory_store
        .remove(inventory_id)
        .ok_or(InventoryError::InventoryNotFound(inventory_id))?;
    instance_store
        .get_mut(instance_id)
        .ok_or(InventoryError::ItemInstanceNotFound(instance_id))?
        .contained_inventory_id = None;
    Ok(())
}
