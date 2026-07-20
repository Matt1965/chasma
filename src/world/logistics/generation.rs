//! Building-generated hauling request creation (EP7).

use crate::world::building::catalog::BuildingCatalog;
use crate::world::building::inventory_binding::BuildingInventoryBindingId;
use crate::world::building::operation::{
    ProductionExecutionAssessment, ProductionExecutionFailure,
};
use crate::world::inventory::{InventoryCatalogCtx, count_stack_item};
use crate::world::{BuildingId, ItemDefinitionId, WorldData};

use super::id::HaulingRequestId;
use super::request::HaulingRequest;
use super::store::HaulingRequestStore;
use super::types::{HaulingGenerationReason, LogisticsRouteTrigger};

/// Sync hauling requests from a production assessment (EP7).
pub fn sync_logistics_requests_from_assessment(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    building_id: BuildingId,
    assessment: &ProductionExecutionAssessment,
    simulation_tick: u64,
    inventory_ctx: &InventoryCatalogCtx<'_>,
) {
    let Some(record) = world.get_building(building_id) else {
        return;
    };
    let Some(definition) = building_catalog.get(&record.definition_id) else {
        return;
    };
    if definition.logistics_routes.is_empty() {
        return;
    }

    if let Some(ProductionExecutionFailure::MissingInput {
        item_id,
        required,
        available,
    }) = assessment.blocking.as_ref()
    {
        let deficit = required.saturating_sub(*available);
        if deficit > 0 {
            generate_for_trigger(
                world,
                building_catalog,
                definition,
                building_id,
                LogisticsRouteTrigger::InputDeficit,
                item_id,
                deficit,
                HaulingGenerationReason::InputDeficit,
                simulation_tick,
                inventory_ctx,
            );
        }
    }

    if let Some(ProductionExecutionFailure::OutputFull { item_id, .. }) =
        assessment.blocking.as_ref()
    {
        generate_for_trigger(
            world,
            building_catalog,
            definition,
            building_id,
            LogisticsRouteTrigger::OutputSurplus,
            item_id,
            surplus_quantity(world, building_id, item_id, inventory_ctx),
            HaulingGenerationReason::OutputSurplus,
            simulation_tick,
            inventory_ctx,
        );
    }
}

/// Generate output surplus requests after successful production (EP7).
pub fn sync_output_surplus_after_production(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    building_id: BuildingId,
    item_id: &ItemDefinitionId,
    simulation_tick: u64,
    inventory_ctx: &InventoryCatalogCtx<'_>,
) {
    let Some(record) = world.get_building(building_id) else {
        return;
    };
    let Some(definition) = building_catalog.get(&record.definition_id) else {
        return;
    };
    let quantity = surplus_quantity(world, building_id, item_id, inventory_ctx);
    if quantity == 0 {
        return;
    }
    generate_for_trigger(
        world,
        building_catalog,
        definition,
        building_id,
        LogisticsRouteTrigger::OutputSurplus,
        item_id,
        quantity,
        HaulingGenerationReason::OutputSurplus,
        simulation_tick,
        inventory_ctx,
    );
}

fn surplus_quantity(
    world: &WorldData,
    building_id: BuildingId,
    item_id: &ItemDefinitionId,
    _inventory_ctx: &InventoryCatalogCtx<'_>,
) -> u32 {
    let binding_store = world.building_inventory_binding_store();
    let Some(set) = binding_store.get(building_id) else {
        return 0;
    };
    let mut total = 0u32;
    for binding in set.bindings() {
        if !binding.role.accepts_operation_output() {
            continue;
        }
        if let Some(record) = world.inventory_store().get(binding.inventory_id) {
            total = total.saturating_add(count_stack_item(record, item_id));
        }
    }
    total
}

fn generate_for_trigger(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    definition: &crate::world::BuildingDefinition,
    building_id: BuildingId,
    trigger: LogisticsRouteTrigger,
    item_id: &ItemDefinitionId,
    quantity: u32,
    reason: HaulingGenerationReason,
    simulation_tick: u64,
    inventory_ctx: &InventoryCatalogCtx<'_>,
) {
    if quantity == 0 {
        return;
    }
    for route in definition
        .logistics_routes
        .iter()
        .filter(|route| route.trigger == trigger && route.item_id == *item_id)
    {
        let Some(local_inventory) = world
            .building_inventory_binding_store()
            .resolve_inventory(building_id, &route.local_binding_id)
        else {
            continue;
        };
        let remote_building_id = resolve_remote_building(world, building_id, route);
        let Some(remote_building_id) = remote_building_id else {
            continue;
        };
        if !route_endpoint_roles_valid(
            building_catalog,
            definition,
            &route.local_binding_id,
            &route.remote_building_definition_id,
            &route.remote_binding_id,
            trigger,
        ) {
            continue;
        }
        let Some(remote_inventory) = world
            .building_inventory_binding_store()
            .resolve_inventory(remote_building_id, &route.remote_binding_id)
        else {
            continue;
        };

        let (source, destination) = match trigger {
            LogisticsRouteTrigger::OutputSurplus => (local_inventory, remote_inventory),
            LogisticsRouteTrigger::InputDeficit => (remote_inventory, local_inventory),
        };
        if source == destination {
            continue;
        }

        let qty = match trigger {
            LogisticsRouteTrigger::OutputSurplus => quantity,
            LogisticsRouteTrigger::InputDeficit => quantity,
        };
        upsert_hauling_request(
            world,
            route.priority,
            item_id.clone(),
            qty,
            source,
            destination,
            building_id,
            reason.clone(),
            simulation_tick,
            inventory_ctx,
        );
    }
}

fn resolve_remote_building(
    world: &WorldData,
    requesting_building_id: BuildingId,
    route: &super::route::BuildingLogisticsRouteDefinition,
) -> Option<BuildingId> {
    let candidates = world
        .logistics_endpoint_index()
        .resolve(
            &route.remote_building_definition_id,
            &route.remote_binding_id,
        )?;
    if candidates.is_empty() {
        return None;
    }
    let settlement = world
        .settlement_store()
        .settlement_for_building(requesting_building_id);
    if let Some(settlement_id) = settlement {
        for candidate in candidates {
            if world
                .settlement_store()
                .settlement_for_building(*candidate)
                == Some(settlement_id)
            {
                return Some(*candidate);
            }
        }
    }
    Some(candidates[0])
}

fn route_endpoint_roles_valid(
    building_catalog: &BuildingCatalog,
    local_definition: &crate::world::BuildingDefinition,
    local_binding_id: &BuildingInventoryBindingId,
    remote_definition_id: &crate::world::BuildingDefinitionId,
    remote_binding_id: &BuildingInventoryBindingId,
    trigger: LogisticsRouteTrigger,
) -> bool {
    let local_role = binding_role(local_definition, local_binding_id);
    let remote_role = building_catalog
        .get(remote_definition_id)
        .and_then(|definition| binding_role(definition, remote_binding_id));
    let (Some(local_role), Some(remote_role)) = (local_role, remote_role) else {
        return false;
    };
    match trigger {
        LogisticsRouteTrigger::InputDeficit => {
            remote_role.advertises_logistics_supply() && local_role.accepts_logistics_delivery()
        }
        LogisticsRouteTrigger::OutputSurplus => {
            local_role.advertises_logistics_supply() && remote_role.accepts_logistics_delivery()
        }
    }
}

fn binding_role(
    definition: &crate::world::BuildingDefinition,
    binding_id: &BuildingInventoryBindingId,
) -> Option<crate::world::building::inventory_binding::BuildingInventoryRole> {
    use crate::world::building::inventory_binding::effective_inventory_binding_definitions;
    effective_inventory_binding_definitions(definition)
        .into_iter()
        .find(|binding| binding.binding_id == *binding_id)
        .map(|binding| binding.role)
}

fn upsert_hauling_request(
    world: &mut WorldData,
    priority: super::types::HaulingRequestPriority,
    item_id: ItemDefinitionId,
    quantity: u32,
    source_inventory_id: crate::world::InventoryId,
    destination_inventory_id: crate::world::InventoryId,
    owning_building_id: BuildingId,
    generation_reason: HaulingGenerationReason,
    simulation_tick: u64,
    inventory_ctx: &InventoryCatalogCtx<'_>,
) -> Option<HaulingRequestId> {
    if quantity == 0 {
        return None;
    }
    if source_inventory_id == destination_inventory_id {
        return None;
    }
    if world.inventory_store().get(source_inventory_id).is_none()
        || world.inventory_store().get(destination_inventory_id).is_none()
    {
        return None;
    }

    if let Some(existing_id) = world
        .hauling_request_store()
        .open_request_for_key(source_inventory_id, destination_inventory_id, &item_id)
    {
        let store = world.hauling_request_store_mut();
        let request = store.get_mut(existing_id)?;
        request.quantity = request.quantity.saturating_add(quantity);
        request.remaining_quantity = request.remaining_quantity.saturating_add(quantity);
        request.priority = priority;
        return Some(existing_id);
    }

    let id = world.hauling_request_store_mut().allocate_id();
    let request = HaulingRequest::new(
        id,
        priority,
        item_id,
        quantity,
        source_inventory_id,
        destination_inventory_id,
        owning_building_id,
        generation_reason,
        simulation_tick,
    );
    world.hauling_request_store_mut().insert(request);
    let _ = inventory_ctx;
    Some(id)
}

/// Dev/manual hauling request spawn (EP7).
pub fn spawn_manual_hauling_request(
    world: &mut WorldData,
    priority: super::types::HaulingRequestPriority,
    item_id: ItemDefinitionId,
    quantity: u32,
    source_inventory_id: crate::world::InventoryId,
    destination_inventory_id: crate::world::InventoryId,
    owning_building_id: BuildingId,
    simulation_tick: u64,
    inventory_ctx: &InventoryCatalogCtx<'_>,
) -> Option<HaulingRequestId> {
    upsert_hauling_request(
        world,
        priority,
        item_id,
        quantity,
        source_inventory_id,
        destination_inventory_id,
        owning_building_id,
        HaulingGenerationReason::ManualDev,
        simulation_tick,
        inventory_ctx,
    )
}
