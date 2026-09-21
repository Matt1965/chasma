//! Unit access to owned equipment and container inventories.

use crate::world::inventory::{InventoryId, InventoryOwnerRef};
use crate::world::{ItemInstanceId, UnitId, WorldData};

/// Whether `unit_id` owns or may use `inventory_id` through personal, equipment, or equipped-container chains.
pub fn unit_owns_inventory(world: &WorldData, unit_id: UnitId, inventory_id: InventoryId) -> bool {
    let Some(record) = world.inventory_store().get(inventory_id) else {
        return false;
    };
    match record.owner() {
        InventoryOwnerRef::Unit(owner) => owner == &unit_id,
        InventoryOwnerRef::UnitEquipment { unit_id: owner, .. } => owner == &unit_id,
        InventoryOwnerRef::ItemContainer(instance_id) => {
            container_instance_accessible_to_unit(world, unit_id, *instance_id)
        }
        _ => false,
    }
}

fn container_instance_accessible_to_unit(
    world: &WorldData,
    unit_id: UnitId,
    instance_id: ItemInstanceId,
) -> bool {
    let Some((parent_inventory_id, _)) =
        world.item_instance_store().inventory_location(instance_id)
    else {
        return false;
    };
    unit_owns_inventory(world, unit_id, parent_inventory_id)
}
