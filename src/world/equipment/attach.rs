//! Unit equipment inventory allocation and validation.

use crate::world::inventory::{
    InventoryError, InventoryOwnerRef, InventoryProfileCatalog, InventoryRecord, InventoryStore,
    remove_owned_inventory,
};
use crate::world::unit::{UnitId, UnitRecord};
use crate::world::{InventoryCatalogCtx, WorldData};

use super::inventories::UnitEquipmentInventories;
use super::slot::EquipmentSlot;

/// Minimal catalog context for equipment cleanup when no full item catalog is available.
pub fn minimal_catalog_ctx(profiles: &InventoryProfileCatalog) -> InventoryCatalogCtx<'_> {
    use std::sync::OnceLock;
    static CATEGORIES: OnceLock<crate::world::ItemCategoryCatalog> = OnceLock::new();
    static ITEMS: OnceLock<crate::world::ItemCatalog> = OnceLock::new();
    let categories = CATEGORIES
        .get_or_init(|| crate::world::ItemCategoryCatalog::from_definitions(Vec::new()).unwrap());
    let items = ITEMS.get_or_init(|| {
        crate::world::ItemCatalog::from_definitions(Vec::new(), categories).unwrap()
    });
    InventoryCatalogCtx::new(items, categories, profiles)
}

fn create_equipment_slot_inventory(
    inventory_store: &mut InventoryStore,
    profiles: &InventoryProfileCatalog,
    slot: EquipmentSlot,
    unit_id: UnitId,
) -> Result<crate::world::InventoryId, InventoryError> {
    let profile_id = slot.profile_id();
    let profile = profiles
        .get(&profile_id)
        .ok_or_else(|| InventoryError::ProfileNotFound(profile_id.clone()))?;
    if !profile.enabled {
        return Err(InventoryError::ProfileNotFound(profile_id.clone()));
    }
    let id = inventory_store.allocate_inventory_id();
    let record = InventoryRecord::new(
        id,
        InventoryOwnerRef::UnitEquipment { unit_id, slot },
        profile_id,
        profile.grid_width,
        profile.grid_height,
    );
    inventory_store.insert(record)?;
    Ok(id)
}

pub fn attach_equipment_on_unit_create(
    inventory_store: &mut InventoryStore,
    profiles: &InventoryProfileCatalog,
    unit_id: UnitId,
) -> Result<UnitEquipmentInventories, InventoryError> {
    Ok(UnitEquipmentInventories {
        head: create_equipment_slot_inventory(
            inventory_store,
            profiles,
            EquipmentSlot::Head,
            unit_id,
        )?,
        body: create_equipment_slot_inventory(
            inventory_store,
            profiles,
            EquipmentSlot::Body,
            unit_id,
        )?,
        arms: create_equipment_slot_inventory(
            inventory_store,
            profiles,
            EquipmentSlot::Arms,
            unit_id,
        )?,
        legs: create_equipment_slot_inventory(
            inventory_store,
            profiles,
            EquipmentSlot::Legs,
            unit_id,
        )?,
        feet: create_equipment_slot_inventory(
            inventory_store,
            profiles,
            EquipmentSlot::Feet,
            unit_id,
        )?,
        weapon: create_equipment_slot_inventory(
            inventory_store,
            profiles,
            EquipmentSlot::Weapon,
            unit_id,
        )?,
        offhand: create_equipment_slot_inventory(
            inventory_store,
            profiles,
            EquipmentSlot::Offhand,
            unit_id,
        )?,
        backpack: create_equipment_slot_inventory(
            inventory_store,
            profiles,
            EquipmentSlot::Backpack,
            unit_id,
        )?,
    })
}

/// True when every equipment-slot inventory has no placed entries.
pub fn unit_equipment_slots_are_empty(world: &WorldData, unit: &UnitRecord) -> bool {
    let Some(equipment) = unit.equipment.as_ref() else {
        return true;
    };
    for inventory_id in equipment.all_inventory_ids() {
        let Some(inventory) = world.inventory_store().get(inventory_id) else {
            return false;
        };
        if !inventory.placed_entries().is_empty() {
            return false;
        }
    }
    true
}

pub fn cleanup_unit_equipment_on_delete(
    world: &mut WorldData,
    ctx: &InventoryCatalogCtx<'_>,
    unit: &UnitRecord,
) -> Result<(), InventoryError> {
    let Some(equipment) = unit.equipment.as_ref() else {
        return Ok(());
    };
    let (inventory_store, instance_store) = world.inventory_runtime_mut();
    for inventory_id in equipment.all_inventory_ids() {
        let slot = equipment
            .contains_inventory(inventory_id)
            .expect("equipment inventory id must map to slot");
        remove_owned_inventory(
            inventory_store,
            instance_store,
            ctx,
            inventory_id,
            InventoryOwnerRef::UnitEquipment {
                unit_id: unit.id,
                slot,
            },
        )?;
    }
    Ok(())
}

/// Attach canonical equipment inventories to legacy units missing `equipment`.
pub fn reconcile_legacy_unit_equipment(
    world: &mut WorldData,
    profiles: &InventoryProfileCatalog,
) -> Result<(), InventoryError> {
    for unit_id in world.sorted_unit_ids() {
        let needs_equipment = world
            .get_unit(unit_id)
            .is_some_and(|unit| unit.equipment.is_none());
        if !needs_equipment {
            continue;
        }
        let equipment =
            attach_equipment_on_unit_create(world.inventory_store_mut(), profiles, unit_id)?;
        world.mutate_unit(unit_id, |record| {
            record.equipment = Some(equipment);
        });
    }
    Ok(())
}

pub fn validate_unit_equipment_links(
    world: &WorldData,
    unit_id: UnitId,
) -> Result<(), InventoryError> {
    let unit = world
        .get_unit(unit_id)
        .ok_or(InventoryError::InventoryAllocationFailed(unit_id))?;
    let equipment = unit
        .equipment
        .as_ref()
        .ok_or(InventoryError::UnitEquipmentMissing(unit_id))?;

    for slot in EquipmentSlot::ALL {
        let inventory_id = equipment.inventory_id(slot);
        let inventory = world
            .inventory_store()
            .get(inventory_id)
            .ok_or(InventoryError::InventoryNotFound(inventory_id))?;
        match inventory.owner() {
            InventoryOwnerRef::UnitEquipment {
                unit_id: owner_unit,
                slot: owner_slot,
            } if *owner_unit == unit_id && *owner_slot == slot => {}
            _ => {
                return Err(InventoryError::UnitEquipmentOwnerMismatch {
                    unit_id,
                    slot,
                    inventory_id,
                });
            }
        }
        let profile = inventory.profile_id();
        if profile != &slot.profile_id() {
            return Err(InventoryError::UnitEquipmentProfileMismatch {
                unit_id,
                slot,
                inventory_id,
                profile_id: profile.clone(),
            });
        }
    }
    Ok(())
}
