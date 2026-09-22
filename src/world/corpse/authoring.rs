//! Corpse authoring APIs (ADR-089 I3).

use super::error::CorpseError;
use super::id::CorpseId;
use super::record::CorpseRecord;
use super::settings::CorpseSettings;
use crate::world::equipment::{
    EquipmentSlot, UnitEquipmentInventories, container_inventory_is_loaded,
};
use crate::world::inventory::{
    InventoryCatalogCtx, InventoryEntryContents, InventoryId, InventoryOwnerRef, InventoryStore,
    ItemInstanceStore, remove_owned_inventory,
};
use crate::world::unit::{UnitDefinition, UnitId, UnitRecord};
use crate::world::{ChunkId, WorldData};

pub fn corpse_lifetime_ticks(definition: &UnitDefinition, settings: &CorpseSettings) -> u64 {
    definition
        .corpse_lifetime_ticks
        .unwrap_or(settings.default_lifetime_ticks)
}

/// Create an authoritative corpse from a dying unit record.
pub fn create_corpse_from_unit(
    world: &mut WorldData,
    unit: &UnitRecord,
    definition: &UnitDefinition,
    settings: &CorpseSettings,
    tick: u64,
) -> Result<CorpseRecord, CorpseError> {
    let id = world.corpse_store_mut().allocate_corpse_id();
    let lifetime = corpse_lifetime_ticks(definition, settings);
    if lifetime == 0 {
        return Err(CorpseError::CorpseLifetimeInvalid { corpse_id: id });
    }
    let record = CorpseRecord::new(
        id,
        unit.id,
        unit.definition_id.clone(),
        unit.placement.clone(),
        unit.current_space_id,
        unit.inventory_id,
        unit.equipment.clone(),
        unit.owner_id,
        unit.team_id,
        unit.affiliation,
        tick,
        lifetime,
    );
    let chunk = ChunkId::new(unit.placement.position.chunk);
    world
        .corpse_store_mut()
        .insert(chunk, record.clone())
        .map_err(|_| CorpseError::CorpseIdCollision(id))?;
    Ok(record)
}

/// Retarget a unit-owned inventory to a corpse without copying entries.
pub fn transfer_inventory_to_corpse(
    inventory_store: &mut InventoryStore,
    instance_store: &mut ItemInstanceStore,
    inventory_id: InventoryId,
    unit_id: UnitId,
    corpse_id: CorpseId,
) -> Result<(), CorpseError> {
    let Some(record) = inventory_store.get_mut(inventory_id) else {
        return Err(CorpseError::CorpseInventoryTransferFailed {
            unit_id,
            inventory_id,
            message: "inventory not found".to_string(),
        });
    };
    match record.owner() {
        InventoryOwnerRef::Unit(owner) if *owner == unit_id => {}
        other => {
            return Err(CorpseError::CorpseInventoryTransferFailed {
                unit_id,
                inventory_id,
                message: format!("unexpected owner {other:?}"),
            });
        }
    }
    record.set_owner(InventoryOwnerRef::Corpse(corpse_id));
    for (entry_index, entry) in record.placed_entries().iter().enumerate() {
        if let crate::world::inventory::InventoryEntryContents::Unique { item_instance_id } =
            &entry.contents
        {
            instance_store.set_inventory_location(*item_instance_id, inventory_id, entry_index);
        }
    }
    Ok(())
}

/// Retarget unit equipment-slot inventories to a corpse without copying entries.
pub fn transfer_equipment_to_corpse(
    inventory_store: &mut InventoryStore,
    instance_store: &mut ItemInstanceStore,
    equipment: &UnitEquipmentInventories,
    unit_id: UnitId,
    corpse_id: CorpseId,
) -> Result<(), CorpseError> {
    for slot in EquipmentSlot::ALL {
        let inventory_id = equipment.inventory_id(slot);
        let Some(record) = inventory_store.get_mut(inventory_id) else {
            return Err(CorpseError::CorpseEquipmentTransferFailed {
                unit_id,
                inventory_id,
                message: "equipment inventory not found".to_string(),
            });
        };
        match record.owner() {
            InventoryOwnerRef::UnitEquipment {
                unit_id: owner,
                slot: owner_slot,
            } if *owner == unit_id && *owner_slot == slot => {}
            other => {
                return Err(CorpseError::CorpseEquipmentTransferFailed {
                    unit_id,
                    inventory_id,
                    message: format!("unexpected owner {other:?}"),
                });
            }
        }
        record.set_owner(InventoryOwnerRef::CorpseEquipment { corpse_id, slot });
        for (entry_index, entry) in record.placed_entries().iter().enumerate() {
            if let InventoryEntryContents::Unique { item_instance_id } = &entry.contents {
                instance_store.set_inventory_location(*item_instance_id, inventory_id, entry_index);
            }
        }
    }
    Ok(())
}

fn corpse_has_loaded_equipped_container(
    world: &WorldData,
    equipment: &UnitEquipmentInventories,
) -> bool {
    let inventory_store = world.inventory_store();
    let instance_store = world.item_instance_store();
    let backpack_inventory_id = equipment.backpack;
    let Some(backpack_record) = inventory_store.get(backpack_inventory_id) else {
        return false;
    };
    for entry in backpack_record.placed_entries() {
        if let InventoryEntryContents::Unique { item_instance_id } = &entry.contents {
            if container_inventory_is_loaded(inventory_store, instance_store, *item_instance_id)
                .unwrap_or(false)
            {
                return true;
            }
        }
    }
    false
}

/// Remove a corpse and delete any owned inventory contents.
pub fn remove_corpse_with_inventory(
    world: &mut WorldData,
    ctx: &InventoryCatalogCtx<'_>,
    corpse_id: CorpseId,
) -> Result<CorpseRecord, CorpseError> {
    let record = world
        .corpse_store()
        .get(corpse_id)
        .ok_or(CorpseError::CorpseNotFound(corpse_id))?
        .clone();
    if let Some(equipment) = record.equipment.as_ref() {
        if corpse_has_loaded_equipped_container(world, equipment) {
            return Err(CorpseError::CorpseExpiryDeferred {
                corpse_id,
                reason: "loaded equipped container loot remains".into(),
            });
        }
    }
    world
        .corpse_store_mut()
        .remove(corpse_id)
        .ok_or(CorpseError::CorpseNotFound(corpse_id))?;
    if let Some(inventory_id) = record.inventory_id {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        remove_owned_inventory(
            inventory_store,
            instance_store,
            ctx,
            inventory_id,
            InventoryOwnerRef::Corpse(corpse_id),
        )
        .map_err(|_| CorpseError::ContainedItemCleanupFailed { inventory_id })?;
    }
    if let Some(equipment) = record.equipment.as_ref() {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        for inventory_id in equipment.all_inventory_ids() {
            let slot = equipment
                .contains_inventory(inventory_id)
                .expect("corpse equipment inventory id must map to slot");
            remove_owned_inventory(
                inventory_store,
                instance_store,
                ctx,
                inventory_id,
                InventoryOwnerRef::CorpseEquipment { corpse_id, slot },
            )
            .map_err(|_| CorpseError::ContainedItemCleanupFailed { inventory_id })?;
        }
    }
    Ok(record)
}
