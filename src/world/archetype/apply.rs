use crate::world::equipment::EquipmentSlot;

use crate::world::inventory::{

    InventoryEntryContents, TransferPlacementPolicy, create_item_instance, place_unique_first_fit,

    transfer_unique_item,

};

use crate::world::unit::UnitRecord;

use crate::world::{

    InventoryCatalogCtx, ItemCatalog, ItemDefinitionId, ItemInstanceMetadata, WorldData,

};



use super::resolve::ResolvedUnitSpawnSpec;

use super::unit::ArchetypeEquipmentEntry;

use crate::world::UnitId;



#[derive(Debug, Clone, PartialEq, Eq)]

pub enum ArchetypeApplyError {

    UnitHasNoPersonalInventory,

    UnitHasNoEquipment,

    ItemNotFound(ItemDefinitionId),

    InventoryPlacementFailed,

    EquipmentTransferFailed,

}



/// Bake archetype dialogue configuration onto the spawned unit record.
pub fn apply_unit_archetype_dialogue_config(
    world: &mut WorldData,
    unit_id: UnitId,
    spec: &ResolvedUnitSpawnSpec,
) {
    if spec.dialogue.is_none() {
        return;
    }
    let dialogue = spec.dialogue.clone();
    let _ = world.mutate_unit(unit_id, |record| record.dialogue = dialogue);
}

/// Apply archetype-authored equipment and inventory items after unit creation.

pub fn apply_unit_archetype_spawn_overrides(

    world: &mut WorldData,

    unit: &UnitRecord,

    spec: &ResolvedUnitSpawnSpec,

    ctx: &InventoryCatalogCtx,

    item_catalog: &ItemCatalog,

) -> Result<(), ArchetypeApplyError> {

    if spec.equipment.is_empty() && spec.inventory_stacks.is_empty() {

        return Ok(());

    }



    let personal = unit

        .inventory_id

        .ok_or(ArchetypeApplyError::UnitHasNoPersonalInventory)?;

    let equipment = unit

        .equipment

        .as_ref()

        .ok_or(ArchetypeApplyError::UnitHasNoEquipment)?;



    for stack in &spec.inventory_stacks {

        ensure_item_exists(item_catalog, &stack.item_id)?;

        for _ in 0..stack.quantity {

            let (inventory_store, instance_store) = world.inventory_runtime_mut();

            let instance_id = create_item_instance(

                inventory_store,

                instance_store,

                ctx,

                stack.item_id.clone(),

                ItemInstanceMetadata::default(),

            )

            .map_err(|_| ArchetypeApplyError::InventoryPlacementFailed)?;

            place_unique_first_fit(inventory_store, instance_store, ctx, personal, instance_id)

                .map_err(|_| ArchetypeApplyError::InventoryPlacementFailed)?;

        }

    }



    for entry in &spec.equipment {

        equip_authored_item(world, ctx, item_catalog, personal, equipment, entry)?;

    }



    Ok(())

}



fn equip_authored_item(

    world: &mut WorldData,

    ctx: &InventoryCatalogCtx,

    item_catalog: &ItemCatalog,

    personal: crate::world::InventoryId,

    equipment: &crate::world::equipment::UnitEquipmentInventories,

    entry: &ArchetypeEquipmentEntry,

) -> Result<(), ArchetypeApplyError> {

    ensure_item_exists(item_catalog, &entry.item_id)?;

    let slot_inventory = equipment_slot_inventory(equipment, entry.slot);



    let (inventory_store, instance_store) = world.inventory_runtime_mut();

    let instance_id = create_item_instance(

        inventory_store,

        instance_store,

        ctx,

        entry.item_id.clone(),

        ItemInstanceMetadata::default(),

    )

    .map_err(|_| ArchetypeApplyError::InventoryPlacementFailed)?;

    place_unique_first_fit(inventory_store, instance_store, ctx, personal, instance_id)

        .map_err(|_| ArchetypeApplyError::InventoryPlacementFailed)?;



    let source_entry_index = world

        .inventory_store()

        .get(personal)

        .and_then(|record| {

            record.placed_entries().iter().position(|placed| {

                matches!(

                    &placed.contents,

                    InventoryEntryContents::Unique { item_instance_id } if *item_instance_id

                        == instance_id

                )

            })

        })

        .ok_or(ArchetypeApplyError::InventoryPlacementFailed)?;



    let (inventory_store, instance_store) = world.inventory_runtime_mut();

    transfer_unique_item(

        inventory_store,

        instance_store,

        ctx,

        personal,

        source_entry_index,

        instance_id,

        slot_inventory,

        TransferPlacementPolicy::ExactCell { x: 0, y: 0 },

    )

    .map_err(|_| ArchetypeApplyError::EquipmentTransferFailed)?;



    Ok(())

}



fn equipment_slot_inventory(

    equipment: &crate::world::equipment::UnitEquipmentInventories,

    slot: EquipmentSlot,

) -> crate::world::InventoryId {

    equipment.inventory_id(slot)

}



fn ensure_item_exists(

    item_catalog: &ItemCatalog,

    item_id: &ItemDefinitionId,

) -> Result<(), ArchetypeApplyError> {

    if item_catalog.get(item_id).is_none() {

        return Err(ArchetypeApplyError::ItemNotFound(item_id.clone()));

    }

    Ok(())

}


