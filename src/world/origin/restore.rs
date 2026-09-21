use crate::world::equipment::EquipmentSlot;
use crate::world::inventory::{
    InventoryOwnerRef, RestoreInventorySubgraphOptions, restore_inventory_subgraph,
};
use crate::world::{InventoryCatalogCtx, UnitId, WorldData};

use super::snapshot::OriginSquadMemberSnapshot;

pub fn apply_member_inventory_loadout(
    world: &mut WorldData,
    inventory_ctx: &InventoryCatalogCtx<'_>,
    unit_id: UnitId,
    member: &OriginSquadMemberSnapshot,
) -> Result<(), String> {
    let record = world
        .get_unit(unit_id)
        .ok_or_else(|| format!("unit `{unit_id:?}` not found"))?
        .clone();

    if let Some(snapshot) = &member.personal_inventory {
        let root_id = record
            .inventory_id
            .ok_or_else(|| format!("unit `{unit_id:?}` has no personal inventory container"))?;
        restore_inventory_subgraph(
            world,
            inventory_ctx,
            snapshot,
            RestoreInventorySubgraphOptions {
                root_owner: InventoryOwnerRef::Unit(unit_id),
                existing_root_inventory_id: Some(root_id),
            },
        )
        .map_err(|error| format!("restore personal inventory: {error:?}"))?;
    }

    let equipment = record
        .equipment
        .ok_or_else(|| format!("unit `{unit_id:?}` has no equipment inventories"))?;
    for slot_snapshot in &member.equipment_slots {
        let Some(snapshot) = &slot_snapshot.inventory else {
            continue;
        };
        let slot = EquipmentSlot::parse(&slot_snapshot.slot)?;
        let inventory_id = equipment.inventory_id(slot);
        restore_inventory_subgraph(
            world,
            inventory_ctx,
            snapshot,
            RestoreInventorySubgraphOptions {
                root_owner: InventoryOwnerRef::Unit(unit_id),
                existing_root_inventory_id: Some(inventory_id),
            },
        )
        .map_err(|error| format!("restore equipment `{}`: {error:?}", slot_snapshot.slot))?;
    }
    Ok(())
}
