//! Settlement inventory aggregation for the production planner (EP9).

use std::collections::HashMap;

use crate::world::ItemDefinitionId;
use crate::world::building::catalog::{BuildingCatalog, BuildingDefinitionId};
use crate::world::building::inventory_binding::BuildingInventoryBindingId;
use crate::world::inventory::{InventoryCatalogCtx, InventoryEntryContents, count_stack_item};
use crate::world::settlement::SettlementId;
use crate::world::{BuildingId, WorldData};

use super::types::BuildingLocalRetention;

/// Sum item quantities across settlement storage supply buildings (EP9).
pub fn aggregate_settlement_stock(
    world: &WorldData,
    building_catalog: &BuildingCatalog,
    settlement_id: SettlementId,
    local_retentions: &[BuildingLocalRetention],
    _inventory_ctx: &InventoryCatalogCtx<'_>,
) -> HashMap<ItemDefinitionId, u32> {
    let mut totals = HashMap::new();
    let building_ids = world
        .settlement_store()
        .buildings_for_settlement(settlement_id);

    for building_id in building_ids {
        let Some(record) = world.get_building(building_id) else {
            continue;
        };
        let Some(definition) = building_catalog.get(&record.definition_id) else {
            continue;
        };
        if !building_advertises_settlement_supply(definition) {
            continue;
        }
        let binding_store = world.building_inventory_binding_store();
        let Some(bindings) = binding_store.get(building_id) else {
            continue;
        };
        for binding in bindings.bindings() {
            if !binding.role.advertises_logistics_supply() {
                continue;
            }
            let inventory_id = binding.inventory_id;
            let Some(inventory) = world.inventory_store().get(inventory_id) else {
                continue;
            };
            for entry in inventory.placed_entries() {
                if let InventoryEntryContents::Stack {
                    item_definition_id,
                    quantity,
                } = &entry.contents
                {
                    let retain = retention_for_binding(
                        local_retentions,
                        &record.definition_id,
                        &binding.binding_id,
                        item_definition_id,
                    );
                    let net = quantity.saturating_sub(retain);
                    if net > 0 {
                        *totals.entry(item_definition_id.clone()).or_default() += net;
                    }
                }
            }
        }
    }
    totals
}

fn building_advertises_settlement_supply(
    definition: &crate::world::building::catalog::BuildingDefinition,
) -> bool {
    if definition.id == BuildingDefinitionId::new("storage_chest") {
        return true;
    }
    definition
        .inventory_bindings
        .iter()
        .any(|binding| binding.role.advertises_logistics_supply())
}

fn retention_for_binding(
    retentions: &[BuildingLocalRetention],
    definition_id: &BuildingDefinitionId,
    binding_id: &BuildingInventoryBindingId,
    item_id: &ItemDefinitionId,
) -> u32 {
    retentions
        .iter()
        .find(|retention| {
            retention.building_definition_id == *definition_id
                && retention.binding_id == *binding_id
                && retention.item_id == *item_id
        })
        .map(|retention| retention.retain_quantity)
        .unwrap_or(0)
}

/// Count items held in a specific building binding (for local demand reporting).
pub fn count_binding_stock(
    world: &WorldData,
    building_id: BuildingId,
    binding_id: &BuildingInventoryBindingId,
    item_id: &ItemDefinitionId,
) -> u32 {
    let binding_store = world.building_inventory_binding_store();
    let Some(bindings) = binding_store.get(building_id) else {
        return 0;
    };
    let Some(binding) = bindings
        .bindings()
        .iter()
        .find(|binding| binding.binding_id == *binding_id)
    else {
        return 0;
    };
    world
        .inventory_store()
        .get(binding.inventory_id)
        .map(|inventory| count_stack_item(inventory, item_id))
        .unwrap_or(0)
}
