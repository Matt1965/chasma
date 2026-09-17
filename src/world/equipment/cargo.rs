//! Worker cargo inventory resolution (Slice 5).

use crate::world::inventory::{InventoryCatalogCtx, InventoryEntryContents, InventoryId};
use crate::world::unit::UnitRecord;
use crate::world::{ItemDefinitionId, ItemInstanceId, WorldData};

use super::inventories::UnitEquipmentInventories;

/// Why worker cargo inventories could not be resolved from authoritative state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkerCargoResolveError {
    MissingBackpackSlotInventory,
    InvalidBackpackEquippedEntry,
    MissingBackpackItemInstance { item_instance_id: ItemInstanceId },
    MissingBackpackInternalInventoryLink { item_instance_id: ItemInstanceId },
    MissingBackpackInternalInventoryRecord { inventory_id: InventoryId },
}

impl std::fmt::Display for WorkerCargoResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingBackpackSlotInventory => {
                write!(f, "missing backpack equipment-slot inventory")
            }
            Self::InvalidBackpackEquippedEntry => {
                write!(f, "invalid equipped backpack entry contents")
            }
            Self::MissingBackpackItemInstance { item_instance_id } => write!(
                f,
                "missing backpack item instance `{}`",
                item_instance_id.raw()
            ),
            Self::MissingBackpackInternalInventoryLink { item_instance_id } => write!(
                f,
                "equipped backpack item `{}` lacks contained_inventory_id",
                item_instance_id.raw()
            ),
            Self::MissingBackpackInternalInventoryRecord { inventory_id } => write!(
                f,
                "missing backpack internal inventory `{}`",
                inventory_id.raw()
            ),
        }
    }
}

impl std::error::Error for WorkerCargoResolveError {}

/// Ordered cargo inventories for worker hauling: personal, then equipped backpack internal.
pub fn worker_cargo_inventories(
    world: &WorldData,
    unit: &UnitRecord,
) -> Result<Vec<InventoryId>, WorkerCargoResolveError> {
    let mut inventories = Vec::new();
    if let Some(personal) = unit.inventory_id {
        inventories.push(personal);
    }
    if let Some(equipment) = unit.equipment {
        if let Some(internal) = equipped_backpack_internal_inventory(world, &equipment)? {
            inventories.push(internal);
        }
    }
    Ok(inventories)
}

/// Resolve equipped backpack internal inventory when a backpack is equipped.
///
/// Returns `Ok(None)` when the backpack slot is empty.
pub fn equipped_backpack_internal_inventory(
    world: &WorldData,
    equipment: &UnitEquipmentInventories,
) -> Result<Option<InventoryId>, WorkerCargoResolveError> {
    let backpack_slot = equipment.backpack;
    let Some(record) = world.inventory_store().get(backpack_slot) else {
        return Err(WorkerCargoResolveError::MissingBackpackSlotInventory);
    };
    let Some(entry) = record.placed_entries().first() else {
        return Ok(None);
    };
    let instance_id = match &entry.contents {
        InventoryEntryContents::Unique { item_instance_id } => *item_instance_id,
        _ => return Err(WorkerCargoResolveError::InvalidBackpackEquippedEntry),
    };
    let Some(instance) = world.item_instance_store().get(instance_id) else {
        return Err(WorkerCargoResolveError::MissingBackpackItemInstance {
            item_instance_id: instance_id,
        });
    };
    let Some(internal_id) = instance.contained_inventory_id else {
        return Err(
            WorkerCargoResolveError::MissingBackpackInternalInventoryLink {
                item_instance_id: instance_id,
            },
        );
    };
    if world.inventory_store().get(internal_id).is_none() {
        return Err(
            WorkerCargoResolveError::MissingBackpackInternalInventoryRecord {
                inventory_id: internal_id,
            },
        );
    }
    Ok(Some(internal_id))
}

/// UI-safe backpack internal lookup. Broken equipped links are treated as absent.
pub fn resolve_equipped_backpack_internal(
    world: &WorldData,
    equipment: &UnitEquipmentInventories,
) -> Option<InventoryId> {
    equipped_backpack_internal_inventory(world, equipment)
        .ok()
        .flatten()
}

/// Total carried quantity of `item_id` across worker cargo inventories.
pub fn carried_quantity_in_worker_cargo(
    world: &WorldData,
    cargo_inventories: &[InventoryId],
    item_id: &ItemDefinitionId,
) -> u32 {
    cargo_inventories
        .iter()
        .map(|inventory_id| {
            world
                .inventory_store()
                .get(*inventory_id)
                .map(|record| crate::world::inventory::count_stack_item(record, item_id))
                .unwrap_or(0)
        })
        .sum()
}

/// Total merge/first-fit capacity for `item_id` across worker cargo inventories.
pub fn worker_cargo_capacity_for_item(
    world: &WorldData,
    cargo_inventories: &[InventoryId],
    item_id: &ItemDefinitionId,
    inventory_ctx: &InventoryCatalogCtx<'_>,
) -> u32 {
    cargo_inventories
        .iter()
        .map(|inventory_id| {
            world
                .inventory_store()
                .get(*inventory_id)
                .map(|record| {
                    crate::world::inventory::max_accept_stack_quantity(
                        record,
                        inventory_ctx,
                        item_id,
                    )
                })
                .unwrap_or(0)
        })
        .sum()
}
