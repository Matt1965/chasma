//! Shared storage acceptance and capability queries.

use crate::world::building::catalog::BuildingDefinition;
use crate::world::building::inventory_binding::{
    BuildingInventoryBindingId, BuildingInventoryRole, effective_inventory_binding_definitions,
};
use crate::world::inventory::InventoryCatalogCtx;
use crate::world::item::{ItemCategoryCatalog, ItemCategoryDefinition};
use crate::world::{BuildingId, ItemDefinitionId, WorldData};

use super::policy::BuildingStoragePolicy;

/// Whether this building definition is a generic storage destination (not production I/O).
pub fn building_is_storage_capable(definition: &BuildingDefinition) -> bool {
    let bindings = effective_inventory_binding_definitions(definition);
    let has_delivery = bindings
        .iter()
        .any(|binding| binding.role.accepts_logistics_delivery());
    if !has_delivery {
        return false;
    }
    let has_operation_io = bindings.iter().any(|binding| {
        matches!(
            binding.role,
            BuildingInventoryRole::Input
                | BuildingInventoryRole::Output
                | BuildingInventoryRole::Fuel
                | BuildingInventoryRole::Waste
                | BuildingInventoryRole::Catalyst
        )
    });
    !has_operation_io
}

/// Default inbound storage binding for a storage-capable building.
pub fn default_storage_delivery_binding_id(
    definition: &BuildingDefinition,
) -> Option<BuildingInventoryBindingId> {
    let bindings = effective_inventory_binding_definitions(definition);
    bindings
        .iter()
        .find(|binding| binding.role.accepts_logistics_delivery())
        .map(|binding| binding.binding_id.clone())
}

pub fn building_storage_policy(
    world: &WorldData,
    building_id: BuildingId,
) -> BuildingStoragePolicy {
    world
        .building_storage_policy_store()
        .policy(building_id)
        .cloned()
        .unwrap_or_default()
}

pub fn storage_policy_accepts_category(
    policy: &BuildingStoragePolicy,
    category_id: &crate::world::ItemCategoryId,
) -> bool {
    policy.accepts_category(category_id)
}

pub fn building_storage_accepts_item(
    world: &WorldData,
    building_id: BuildingId,
    item_id: &ItemDefinitionId,
    inventory_ctx: &InventoryCatalogCtx<'_>,
) -> bool {
    let policy = building_storage_policy(world, building_id);
    let Some(item) = inventory_ctx.items.get(item_id) else {
        return false;
    };
    storage_policy_accepts_category(&policy, &item.category_id)
}

pub fn building_storage_accepts_item_for_definition(
    world: &WorldData,
    definition: &BuildingDefinition,
    building_id: BuildingId,
    item_id: &ItemDefinitionId,
    inventory_ctx: &InventoryCatalogCtx<'_>,
) -> bool {
    if !building_is_storage_capable(definition) {
        return true;
    }
    building_storage_accepts_item(world, building_id, item_id, inventory_ctx)
}

/// Whether surplus from this binding may feed production output storage demand.
pub fn binding_is_production_output_surplus_source(role: BuildingInventoryRole) -> bool {
    matches!(
        role,
        BuildingInventoryRole::Output | BuildingInventoryRole::Waste
    )
}

/// Production buildings may supply output surplus; storage buildings do not.
pub fn building_is_production_output_surplus_source(definition: &BuildingDefinition) -> bool {
    !building_is_storage_capable(definition)
}

/// Whether an item stored in a storage building is misfiled under current policy.
pub fn storage_item_is_misfiled(
    world: &WorldData,
    building_id: BuildingId,
    item_id: &ItemDefinitionId,
    inventory_ctx: &InventoryCatalogCtx<'_>,
) -> bool {
    !building_storage_accepts_item(world, building_id, item_id, inventory_ctx)
}

/// Inventory bindings on storage-capable buildings that may hold relocated items.
pub fn storage_delivery_inventory_ids(
    world: &WorldData,
    definition: &BuildingDefinition,
    building_id: BuildingId,
) -> Vec<crate::world::InventoryId> {
    let Some(binding_id) = default_storage_delivery_binding_id(definition) else {
        return Vec::new();
    };
    world
        .building_inventory_binding_store()
        .resolve_inventory(building_id, &binding_id)
        .into_iter()
        .collect()
}

pub fn mark_settlement_storage_logistics_wakeup(
    world: &mut WorldData,
    building_catalog: &crate::world::building::catalog::BuildingCatalog,
    building_id: BuildingId,
) {
    let settlement_id = world
        .settlement_store()
        .settlement_for_building(building_id);
    if let Some(settlement_id) = settlement_id {
        for candidate in world
            .settlement_store()
            .buildings_for_settlement(settlement_id)
        {
            let Some(record) = world.get_building(candidate) else {
                continue;
            };
            let Some(definition) = building_catalog.get(&record.definition_id) else {
                continue;
            };
            if building_is_storage_capable(definition) {
                world
                    .building_storage_policy_store_mut()
                    .mark_logistics_dirty(candidate);
            }
        }
    } else {
        world
            .building_storage_policy_store_mut()
            .mark_logistics_dirty(building_id);
    }
}

pub fn mark_storage_logistics_dirty_for_inventory(
    world: &mut WorldData,
    building_catalog: &crate::world::building::catalog::BuildingCatalog,
    inventory_id: crate::world::InventoryId,
) {
    let building_id = world
        .building_inventory_binding_store()
        .building_id_for_inventory(inventory_id);
    if let Some(building_id) = building_id {
        mark_settlement_storage_logistics_wakeup(world, building_catalog, building_id);
    }
}

pub fn all_storage_filter_categories(
    category_catalog: &ItemCategoryCatalog,
) -> Vec<ItemCategoryDefinition> {
    let mut categories: Vec<_> = category_catalog.enabled_definitions().cloned().collect();
    categories.sort_by(|left, right| {
        left.sort_priority
            .unwrap_or(0)
            .cmp(&right.sort_priority.unwrap_or(0))
            .then_with(|| left.id.as_str().cmp(right.id.as_str()))
    });
    categories
}
