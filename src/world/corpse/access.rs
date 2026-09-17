//! Corpse loot inventory access during an active loot session.

use super::id::CorpseId;
use crate::world::equipment::UnitEquipmentInventories;
use crate::world::inventory::{InventoryId, InventoryOwnerRef};
use crate::world::{ItemInstanceId, WorldData};

/// Whether `inventory_id` is lootable from `corpse_id` (personal, equipment, or equipped-container internal).
pub fn is_corpse_loot_inventory(
    world: &WorldData,
    corpse_id: CorpseId,
    inventory_id: InventoryId,
) -> bool {
    let Some(corpse) = world.corpse_store().get(corpse_id) else {
        return false;
    };
    if corpse.inventory_id == Some(inventory_id) {
        return true;
    }
    if let Some(equipment) = corpse.equipment.as_ref() {
        if equipment.all_inventory_ids().contains(&inventory_id) {
            return true;
        }
        if corpse_equipment_container_internal(world, equipment, inventory_id).is_some() {
            return true;
        }
    }
    false
}

fn corpse_equipment_container_internal(
    world: &WorldData,
    equipment: &UnitEquipmentInventories,
    inventory_id: InventoryId,
) -> Option<ItemInstanceId> {
    let record = world.inventory_store().get(inventory_id)?;
    let InventoryOwnerRef::ItemContainer(instance_id) = record.owner() else {
        return None;
    };
    let Some((parent_inventory_id, _)) =
        world.item_instance_store().inventory_location(*instance_id)
    else {
        return None;
    };
    if equipment.all_inventory_ids().contains(&parent_inventory_id) {
        Some(*instance_id)
    } else {
        None
    }
}
