//! Building-generated hauling request creation (EP7).

use crate::world::building::catalog::BuildingCatalog;
use crate::world::building::inventory_binding::BuildingInventoryBindingId;
use crate::world::building::operation::{
    ProductionExecutionAssessment, ProductionExecutionFailure, building_work_priority_u8,
};
use crate::world::building::storage_policy::{
    binding_is_production_output_surplus_source, building_is_production_output_surplus_source,
    building_is_storage_capable, building_storage_accepts_item,
    default_storage_delivery_binding_id,
};
use crate::world::inventory::{InventoryCatalogCtx, count_stack_item};
use crate::world::logistics::destination_can_fit_stack_quantity;
use crate::world::settlement::SettlementId;
use crate::world::{BuildingId, ItemDefinitionId, WorldData};

use super::id::HaulingRequestId;
use super::request::HaulingRequest;
use super::reservation::release_request_reservations;
use super::store::HaulingRequestStore;
use super::types::{
    HaulingGenerationReason, HaulingRequestPriority, HaulingRequestStatus, LogisticsRouteTrigger,
};

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

    if let Some(ProductionExecutionFailure::MissingInput {
        item_id,
        required,
        available,
    }) = assessment.blocking.as_ref()
    {
        let deficit = required.saturating_sub(*available);
        if deficit > 0 && !definition.logistics_routes.is_empty() {
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
        let quantity = surplus_quantity(world, building_id, item_id, inventory_ctx);
        sync_output_surplus_hauling(
            world,
            building_catalog,
            building_id,
            definition,
            item_id,
            quantity,
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
    sync_output_surplus_hauling(
        world,
        building_catalog,
        building_id,
        definition,
        item_id,
        quantity,
        simulation_tick,
        inventory_ctx,
    );
}

fn sync_output_surplus_hauling(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    building_id: BuildingId,
    definition: &crate::world::BuildingDefinition,
    item_id: &ItemDefinitionId,
    quantity: u32,
    simulation_tick: u64,
    inventory_ctx: &InventoryCatalogCtx<'_>,
) {
    if quantity == 0 {
        return;
    }
    let route_matches = if definition.logistics_routes.is_empty() {
        0
    } else {
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
        )
    };
    if route_matches == 0 {
        sync_generic_storage_inbound(
            world,
            building_catalog,
            building_id,
            definition,
            item_id,
            quantity,
            simulation_tick,
            inventory_ctx,
        );
    }
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
) -> usize {
    if quantity == 0 {
        return 0;
    }
    let mut matches = 0usize;
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

        let (source, destination, destination_building_id) = match trigger {
            LogisticsRouteTrigger::OutputSurplus => {
                (local_inventory, remote_inventory, remote_building_id)
            }
            LogisticsRouteTrigger::InputDeficit => (remote_inventory, local_inventory, building_id),
        };
        if source == destination {
            continue;
        }
        if !destination_accepts_inbound_haul(
            world,
            building_catalog,
            destination_building_id,
            item_id,
            inventory_ctx,
        ) {
            continue;
        }
        if !destination_can_fit_stack_quantity(
            world.inventory_store(),
            world.inventory_reservation_store(),
            inventory_ctx,
            destination,
            item_id,
            quantity,
        ) {
            continue;
        }

        if upsert_hauling_request(
            world,
            route.priority,
            item_id.clone(),
            quantity,
            source,
            destination,
            building_id,
            reason.clone(),
            simulation_tick,
            inventory_ctx,
        )
        .is_some()
        {
            matches += 1;
        }
    }
    matches
}

fn sync_generic_storage_inbound(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    source_building_id: BuildingId,
    source_definition: &crate::world::BuildingDefinition,
    item_id: &ItemDefinitionId,
    quantity: u32,
    simulation_tick: u64,
    inventory_ctx: &InventoryCatalogCtx<'_>,
) {
    if quantity == 0 || !building_is_production_output_surplus_source(source_definition) {
        return;
    }
    let settlement_id = world
        .settlement_store()
        .settlement_for_building(source_building_id);
    let Some(settlement_id) = settlement_id else {
        return;
    };

    let binding_store = world.building_inventory_binding_store();
    let Some(source_bindings) = binding_store.get(source_building_id) else {
        return;
    };
    let source_inventory = source_bindings
        .bindings()
        .iter()
        .find(|binding| binding_is_production_output_surplus_source(binding.role))
        .and_then(|binding| {
            let record = world.inventory_store().get(binding.inventory_id)?;
            if count_stack_item(record, item_id) > 0 {
                Some(binding.inventory_id)
            } else {
                None
            }
        });
    let Some(source_inventory) = source_inventory else {
        return;
    };

    let destination = select_generic_storage_destination(
        world,
        building_catalog,
        settlement_id,
        item_id,
        quantity,
        inventory_ctx,
        Some(source_building_id),
    );
    let Some((destination_building_id, destination_inventory)) = destination else {
        return;
    };
    if destination_inventory == source_inventory {
        return;
    }

    let _ = upsert_hauling_request(
        world,
        HaulingRequestPriority::Normal,
        item_id.clone(),
        quantity,
        source_inventory,
        destination_inventory,
        source_building_id,
        HaulingGenerationReason::OutputSurplus,
        simulation_tick,
        inventory_ctx,
    );
    let _ = destination_building_id;
}

struct StorageDestinationCandidate {
    building_id: BuildingId,
    inventory_id: crate::world::InventoryId,
    priority: u8,
}

pub(crate) fn select_generic_storage_destination(
    world: &WorldData,
    building_catalog: &BuildingCatalog,
    settlement_id: SettlementId,
    item_id: &ItemDefinitionId,
    quantity: u32,
    inventory_ctx: &InventoryCatalogCtx<'_>,
    exclude_source_building_id: Option<BuildingId>,
) -> Option<(BuildingId, crate::world::InventoryId)> {
    let mut candidates = Vec::new();
    for building_id in world
        .settlement_store()
        .buildings_for_settlement(settlement_id)
    {
        if exclude_source_building_id.is_some_and(|exclude| exclude == building_id) {
            continue;
        }
        let Some(record) = world.get_building(building_id) else {
            continue;
        };
        let Some(definition) = building_catalog.get(&record.definition_id) else {
            continue;
        };
        if !building_is_storage_capable(definition) {
            continue;
        }
        if !building_storage_accepts_item(world, building_id, item_id, inventory_ctx) {
            continue;
        }
        let binding_id = default_storage_delivery_binding_id(definition)?;
        let inventory_id = world
            .building_inventory_binding_store()
            .resolve_inventory(building_id, &binding_id)?;
        if !destination_can_fit_stack_quantity(
            world.inventory_store(),
            world.inventory_reservation_store(),
            inventory_ctx,
            inventory_id,
            item_id,
            quantity,
        ) {
            continue;
        }
        candidates.push(StorageDestinationCandidate {
            building_id,
            inventory_id,
            priority: building_work_priority_u8(world, building_id),
        });
    }
    candidates.sort_by(|left, right| {
        right
            .priority
            .cmp(&left.priority)
            .then_with(|| left.building_id.raw().cmp(&right.building_id.raw()))
    });
    candidates
        .first()
        .map(|candidate| (candidate.building_id, candidate.inventory_id))
}

fn destination_accepts_inbound_haul(
    world: &WorldData,
    building_catalog: &BuildingCatalog,
    destination_building_id: BuildingId,
    item_id: &ItemDefinitionId,
    inventory_ctx: &InventoryCatalogCtx<'_>,
) -> bool {
    let Some(record) = world.get_building(destination_building_id) else {
        return false;
    };
    let Some(definition) = building_catalog.get(&record.definition_id) else {
        return false;
    };
    if !building_is_storage_capable(definition) {
        return true;
    }
    building_storage_accepts_item(world, destination_building_id, item_id, inventory_ctx)
}

fn resolve_remote_building(
    world: &WorldData,
    requesting_building_id: BuildingId,
    route: &super::route::BuildingLogisticsRouteDefinition,
) -> Option<BuildingId> {
    let candidates = world.logistics_endpoint_index().resolve(
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
            if world.settlement_store().settlement_for_building(*candidate) == Some(settlement_id) {
                return Some(*candidate);
            }
        }
        return None;
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
        || world
            .inventory_store()
            .get(destination_inventory_id)
            .is_none()
    {
        return None;
    }

    if let Some(existing_id) = world.hauling_request_store().open_request_for_key(
        source_inventory_id,
        destination_inventory_id,
        &item_id,
    ) {
        let store = world.hauling_request_store_mut();
        let request = store.get_mut(existing_id)?;
        request.quantity = request.quantity.saturating_add(quantity);
        request.remaining_quantity = request.remaining_quantity.saturating_add(quantity);
        request.priority = priority;
        return Some(existing_id);
    }

    if let Some(blocked_id) = world.hauling_request_store().blocked_request_for_key(
        source_inventory_id,
        destination_inventory_id,
        &item_id,
    ) {
        release_request_reservations(
            world.inventory_reservation_store_mut(),
            blocked_id,
            &item_id,
        );
        let store = world.hauling_request_store_mut();
        let request = store.get_mut(blocked_id)?;
        request.quantity = request.quantity.saturating_add(quantity);
        request.remaining_quantity = request.remaining_quantity.saturating_add(quantity);
        request.priority = priority;
        request.status = HaulingRequestStatus::Pending;
        request.blocking_reason = None;
        request.blocked_at_tick = None;
        request.assigned_unit_id = None;
        request.assigned_task_id = None;
        request.reservation_state = super::types::HaulingReservationState::None;
        request.execution_phase = super::types::HaulExecutionPhase::Pending;
        store.refresh_open_key(blocked_id);
        return Some(blocked_id);
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

pub(crate) fn upsert_hauling_request_for_storage(
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
    upsert_hauling_request(
        world,
        priority,
        item_id,
        quantity,
        source_inventory_id,
        destination_inventory_id,
        owning_building_id,
        generation_reason,
        simulation_tick,
        inventory_ctx,
    )
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
