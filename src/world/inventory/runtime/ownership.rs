//! Inventory ownership transfer and cleanup (ADR-089 I3).

use super::catalog_ctx::InventoryCatalogCtx;
use super::error::InventoryError;
use super::id::{InventoryId, ItemInstanceId};
use super::instance::ItemInstance;
use super::owner::InventoryOwnerRef;
use super::record::InventoryRecord;
use super::store::{InventoryStore, ItemInstanceStore};
use crate::world::corpse::CorpseId;
use crate::world::unit::UnitDefinition;
use crate::world::{InventoryProfileId, UnitId};

/// Removed inventory contents summary for explicit deletion policies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemovedInventoryContents {
    pub inventory_id: Option<InventoryId>,
    pub destroyed_instance_ids: Vec<ItemInstanceId>,
}

pub fn create_unit_inventory(
    inventory_store: &mut InventoryStore,
    ctx: &InventoryCatalogCtx<'_>,
    profile_id: InventoryProfileId,
    unit_id: UnitId,
) -> Result<InventoryId, InventoryError> {
    let profile = ctx.require_profile(&profile_id)?;
    if !profile.enabled {
        return Err(InventoryError::ProfileNotFound(profile_id));
    }
    let id = inventory_store.allocate_inventory_id();
    let record = InventoryRecord::new(
        id,
        InventoryOwnerRef::Unit(unit_id),
        profile_id,
        profile.grid_width,
        profile.grid_height,
    );
    inventory_store.insert(record)?;
    Ok(id)
}

pub fn transfer_inventory_owner(
    inventory_store: &mut InventoryStore,
    inventory_id: InventoryId,
    from: InventoryOwnerRef,
    to: InventoryOwnerRef,
) -> Result<(), InventoryError> {
    let record = inventory_store
        .get_mut(inventory_id)
        .ok_or(InventoryError::InventoryNotFound(inventory_id))?;
    if record.owner() != &from {
        return Err(InventoryError::OwnerMismatch {
            inventory_id,
            expected: from,
        });
    }
    record.set_owner(to);
    Ok(())
}

pub fn remove_owned_inventory(
    inventory_store: &mut InventoryStore,
    instance_store: &mut ItemInstanceStore,
    ctx: &InventoryCatalogCtx<'_>,
    inventory_id: InventoryId,
    expected_owner: InventoryOwnerRef,
) -> Result<RemovedInventoryContents, InventoryError> {
    let record = inventory_store
        .get(inventory_id)
        .ok_or(InventoryError::InventoryNotFound(inventory_id))?
        .clone();
    if record.owner() != &expected_owner {
        return Err(InventoryError::OwnerMismatch {
            inventory_id,
            expected: expected_owner,
        });
    }
    for entry in record.placed_entries() {
        if let super::entry::InventoryEntryContents::Unique { item_instance_id } = &entry.contents {
            instance_store.clear_location(*item_instance_id);
        }
    }
    let mut destroyed_instance_ids = Vec::new();
    for entry in record.placed_entries() {
        if let super::entry::InventoryEntryContents::Unique { item_instance_id } = &entry.contents {
            if instance_store.remove(*item_instance_id).is_some() {
                destroyed_instance_ids.push(*item_instance_id);
            }
        }
    }
    inventory_store
        .remove(inventory_id)
        .ok_or(InventoryError::InventoryNotFound(inventory_id))?;
    let _ = ctx;
    Ok(RemovedInventoryContents {
        inventory_id: Some(inventory_id),
        destroyed_instance_ids,
    })
}

pub fn profile_for_unit_definition(definition: &UnitDefinition) -> Option<InventoryProfileId> {
    definition.inventory_profile_id.clone()
}
