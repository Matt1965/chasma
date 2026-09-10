//! Misfiled storage relocation (sorting chests).

use std::collections::HashMap;

use crate::world::building::catalog::BuildingCatalog;
use crate::world::building::storage_policy::{
    building_is_storage_capable, building_storage_accepts_item, storage_delivery_inventory_ids,
    storage_item_is_misfiled,
};
use crate::world::inventory::{InventoryCatalogCtx, InventoryEntryContents, count_stack_item};
use crate::world::settlement::SettlementId;
use crate::world::{BuildingId, ItemDefinitionId, WorldData};

use super::execute::cancel_hauling_request;
use super::generation::{select_generic_storage_destination, upsert_hauling_request_for_storage};
use super::types::{HaulingGenerationReason, HaulingRequestPriority};

/// Reevaluate misfiled contents and sync relocation requests for one storage building.
pub fn sync_misfiled_storage_for_building(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    building_id: BuildingId,
    simulation_tick: u64,
    inventory_ctx: &InventoryCatalogCtx<'_>,
) {
    let Some(record) = world.get_building(building_id) else {
        return;
    };
    let Some(definition) = building_catalog.get(&record.definition_id) else {
        return;
    };
    if !building_is_storage_capable(definition) {
        return;
    }
    let settlement_id = world
        .settlement_store()
        .settlement_for_building(building_id);
    let Some(settlement_id) = settlement_id else {
        return;
    };

    cancel_obsolete_misfiled_requests(world, building_catalog, building_id, inventory_ctx);

    let inventory_ids = storage_delivery_inventory_ids(world, definition, building_id);
    let mut misfiled: HashMap<ItemDefinitionId, (u32, crate::world::InventoryId)> = HashMap::new();
    for inventory_id in inventory_ids {
        let Some(inventory) = world.inventory_store().get(inventory_id) else {
            continue;
        };
        for entry in inventory.placed_entries() {
            if let InventoryEntryContents::Stack {
                item_definition_id,
                quantity,
            } = &entry.contents
            {
                if storage_item_is_misfiled(world, building_id, item_definition_id, inventory_ctx) {
                    misfiled
                        .entry(item_definition_id.clone())
                        .and_modify(|(total, _)| *total = total.saturating_add(*quantity))
                        .or_insert((*quantity, inventory_id));
                }
            }
        }
    }

    for (item_id, (quantity, source_inventory)) in misfiled {
        if quantity == 0 {
            continue;
        }
        let Some((_, destination_inventory)) = select_generic_storage_destination(
            world,
            building_catalog,
            settlement_id,
            &item_id,
            quantity,
            inventory_ctx,
            Some(building_id),
        ) else {
            continue;
        };
        if destination_inventory == source_inventory {
            continue;
        }
        let _ = upsert_hauling_request_for_storage(
            world,
            HaulingRequestPriority::Normal,
            item_id,
            quantity,
            source_inventory,
            destination_inventory,
            building_id,
            HaulingGenerationReason::MisfiledRelocation,
            simulation_tick,
            inventory_ctx,
        );
    }
}

/// Process storage buildings marked dirty by policy or inventory changes.
pub fn sync_dirty_storage_logistics(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    simulation_tick: u64,
    inventory_ctx: &InventoryCatalogCtx<'_>,
) {
    let dirty = world
        .building_storage_policy_store_mut()
        .drain_logistics_dirty();
    for building_id in dirty {
        sync_misfiled_storage_for_building(
            world,
            building_catalog,
            building_id,
            simulation_tick,
            inventory_ctx,
        );
    }
}

/// Mark every building in a settlement for misfiled reevaluation.
pub fn mark_all_settlement_buildings_logistics_dirty(
    world: &mut WorldData,
    settlement_id: SettlementId,
) {
    for building_id in world
        .settlement_store()
        .buildings_for_settlement(settlement_id)
    {
        world
            .building_storage_policy_store_mut()
            .mark_logistics_dirty(building_id);
    }
}

/// Mark every storage building in a settlement for misfiled reevaluation.
pub fn mark_settlement_storage_logistics_dirty(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    settlement_id: SettlementId,
) {
    for building_id in world
        .settlement_store()
        .buildings_for_settlement(settlement_id)
    {
        let Some(record) = world.get_building(building_id) else {
            continue;
        };
        let Some(definition) = building_catalog.get(&record.definition_id) else {
            continue;
        };
        if building_is_storage_capable(definition) {
            world
                .building_storage_policy_store_mut()
                .mark_logistics_dirty(building_id);
        }
    }
}

fn cancel_obsolete_misfiled_requests(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    source_building_id: BuildingId,
    inventory_ctx: &InventoryCatalogCtx<'_>,
) {
    let request_ids: Vec<_> = world.hauling_request_store().sorted_request_ids();
    for request_id in request_ids {
        let Some(request) = world.hauling_request_store().get(request_id) else {
            continue;
        };
        if request.generation_reason != HaulingGenerationReason::MisfiledRelocation {
            continue;
        }
        if request.owning_building_id != source_building_id {
            continue;
        }
        if !request.status.is_open() {
            continue;
        }
        let physical = world
            .inventory_store()
            .get(request.source_inventory_id)
            .map(|record| count_stack_item(record, &request.item_id))
            .unwrap_or(0);
        let still_misfiled = physical > 0
            && storage_item_is_misfiled(world, source_building_id, &request.item_id, inventory_ctx);
        if still_misfiled {
            let settlement_id = world
                .settlement_store()
                .settlement_for_building(source_building_id);
            let destination_valid = settlement_id.is_some_and(|settlement_id| {
                select_generic_storage_destination(
                    world,
                    building_catalog,
                    settlement_id,
                    &request.item_id,
                    request.remaining_quantity.max(1),
                    inventory_ctx,
                    Some(source_building_id),
                )
                .is_some_and(|(_, dest)| dest == request.destination_inventory_id)
                    && building_storage_accepts_item(
                        world,
                        world
                            .building_inventory_binding_store()
                            .building_id_for_inventory(request.destination_inventory_id)
                            .unwrap_or(source_building_id),
                        &request.item_id,
                        inventory_ctx,
                    )
            });
            if destination_valid {
                continue;
            }
        }
        cancel_hauling_request(world, request_id);
    }
}
