//! Atomic hauling pickup and deposit (EP7).

use crate::world::inventory::{
    InventoryCatalogCtx, InventoryEntryContents, TransferPlacementPolicy, count_stack_item,
    transfer_stack_quantity,
};
use crate::world::{InventoryId, ItemDefinitionId, WorldData};

use super::id::HaulingRequestId;
use super::reservation::{
    available_stack_quantity, release_request_reservations, reserve_destination_capacity,
    reserve_source_items,
};
use super::types::{
    HaulExecutionPhase, HaulingBlockingReason, HaulingRequestStatus, HaulingReservationState,
};

/// Reserve source and destination for a hauling request (EP7).
pub fn reserve_hauling_request(
    world: &mut WorldData,
    request_id: HaulingRequestId,
    quantity: u32,
    inventory_ctx: &InventoryCatalogCtx<'_>,
) -> Result<(), HaulingBlockingReason> {
    let (source, destination, item_id) = {
        let request = world
            .hauling_request_store()
            .get(request_id)
            .ok_or(HaulingBlockingReason::ReservationFailed)?;
        (
            request.source_inventory_id,
            request.destination_inventory_id,
            request.item_id.clone(),
        )
    };
    if source == destination {
        return Err(HaulingBlockingReason::SourceEqualsDestination);
    }
    {
        let (inventory_store, reservations) = world.hauling_reserve_borrow_split();
        reserve_destination_capacity(
            reservations,
            request_id,
            destination,
            quantity,
            inventory_store,
            inventory_ctx,
            &item_id,
        )?;
    }
    {
        let (inventory_store, reservations) = world.hauling_reserve_borrow_split();
        reserve_source_items(
            reservations,
            request_id,
            source,
            &item_id,
            quantity,
            inventory_store,
        )?;
    }
    if let Some(request) = world.hauling_request_store_mut().get_mut(request_id) {
        request.reservation_state = HaulingReservationState::FullyReserved;
    }
    Ok(())
}

/// Pick up items from source into worker inventory (EP7).
pub fn pickup_haul_cargo(
    world: &mut WorldData,
    request_id: HaulingRequestId,
    worker_inventory_id: InventoryId,
    quantity: u32,
    inventory_ctx: &InventoryCatalogCtx<'_>,
) -> Result<u32, HaulingBlockingReason> {
    let (source, item_id) = {
        let request = world
            .hauling_request_store()
            .get(request_id)
            .ok_or(HaulingBlockingReason::MissingSource)?;
        (request.source_inventory_id, request.item_id.clone())
    };
    let source_entry = find_transferable_stack(world, source, &item_id, quantity)?;
    let (inventory_store, instance_store) = world.inventory_runtime_mut();
    let report = transfer_stack_quantity(
        inventory_store,
        instance_store,
        inventory_ctx,
        source,
        source_entry,
        worker_inventory_id,
        quantity,
        TransferPlacementPolicy::MergeThenFirstFit,
        false,
    )
    .map_err(|_| HaulingBlockingReason::NoAvailableItems)?;
    if report.moved == 0 {
        return Err(HaulingBlockingReason::NoAvailableItems);
    }
    if let Some(request) = world.hauling_request_store_mut().get_mut(request_id) {
        request.picked_up_quantity = request.picked_up_quantity.saturating_add(report.moved);
        request.execution_phase = HaulExecutionPhase::TravelingToDestination;
    }
    Ok(report.moved)
}

/// Deposit worker cargo into destination inventory (EP7).
pub fn deposit_haul_cargo(
    world: &mut WorldData,
    request_id: HaulingRequestId,
    worker_inventory_id: InventoryId,
    quantity: u32,
    inventory_ctx: &InventoryCatalogCtx<'_>,
) -> Result<u32, HaulingBlockingReason> {
    let (destination, item_id) = {
        let request = world
            .hauling_request_store()
            .get(request_id)
            .ok_or(HaulingBlockingReason::DestinationFull)?;
        (request.destination_inventory_id, request.item_id.clone())
    };
    let worker_entry = find_transferable_stack(world, worker_inventory_id, &item_id, quantity)?;
    let (inventory_store, instance_store) = world.inventory_runtime_mut();
    let report = transfer_stack_quantity(
        inventory_store,
        instance_store,
        inventory_ctx,
        worker_inventory_id,
        worker_entry,
        destination,
        quantity,
        TransferPlacementPolicy::MergeThenFirstFit,
        false,
    )
    .map_err(|_| HaulingBlockingReason::DestinationFull)?;
    if report.moved == 0 {
        return Err(HaulingBlockingReason::DestinationFull);
    }
    if let Some(request) = world.hauling_request_store_mut().get_mut(request_id) {
        request.remaining_quantity = request.remaining_quantity.saturating_sub(report.moved);
        request.picked_up_quantity = request.picked_up_quantity.saturating_sub(report.moved);
        if request.remaining_quantity == 0 {
            request.status = HaulingRequestStatus::Completed;
            request.execution_phase = HaulExecutionPhase::Completed;
        } else {
            request.status = HaulingRequestStatus::PartiallyFulfilled;
            request.execution_phase = HaulExecutionPhase::TravelingToSource;
        }
    }
    release_request_reservations(
        world.inventory_reservation_store_mut(),
        request_id,
        &item_id,
    );
    if let Some(request) = world.hauling_request_store_mut().get_mut(request_id) {
        request.reservation_state = HaulingReservationState::None;
    }
    Ok(report.moved)
}

/// Dev-only: force-complete a hauling request by transferring remaining cargo (EP7).
pub fn force_complete_hauling_request(
    world: &mut WorldData,
    request_id: HaulingRequestId,
    inventory_ctx: &InventoryCatalogCtx<'_>,
) -> Result<u32, HaulingBlockingReason> {
    let (source, destination, item_id, remaining) = {
        let request = world
            .hauling_request_store()
            .get(request_id)
            .ok_or(HaulingBlockingReason::MissingSource)?;
        (
            request.source_inventory_id,
            request.destination_inventory_id,
            request.item_id.clone(),
            request.remaining_quantity,
        )
    };
    if remaining == 0 {
        if let Some(request) = world.hauling_request_store_mut().get_mut(request_id) {
            request.status = HaulingRequestStatus::Completed;
            request.execution_phase = HaulExecutionPhase::Completed;
        }
        return Ok(0);
    }
    let available = super::reservation::available_stack_quantity(
        world.inventory_store(),
        world.inventory_reservation_store(),
        source,
        &item_id,
    );
    let quantity = available.min(remaining);
    if quantity == 0 {
        return Err(HaulingBlockingReason::NoAvailableItems);
    }
    let source_entry = find_transferable_stack(world, source, &item_id, quantity)?;
    let (inventory_store, instance_store) = world.inventory_runtime_mut();
    let report = transfer_stack_quantity(
        inventory_store,
        instance_store,
        inventory_ctx,
        source,
        source_entry,
        destination,
        quantity,
        TransferPlacementPolicy::MergeThenFirstFit,
        false,
    )
    .map_err(|_| HaulingBlockingReason::DestinationFull)?;
    if let Some(request) = world.hauling_request_store_mut().get_mut(request_id) {
        request.remaining_quantity = request.remaining_quantity.saturating_sub(report.moved);
        if request.remaining_quantity == 0 {
            request.status = HaulingRequestStatus::Completed;
            request.execution_phase = HaulExecutionPhase::Completed;
        } else {
            request.status = HaulingRequestStatus::PartiallyFulfilled;
        }
    }
    release_request_reservations(
        world.inventory_reservation_store_mut(),
        request_id,
        &item_id,
    );
    if let Some(request) = world.hauling_request_store_mut().get_mut(request_id) {
        request.reservation_state = HaulingReservationState::None;
    }
    Ok(report.moved)
}

/// Cancel hauling request and release reservations (EP7).
pub fn cancel_hauling_request(world: &mut WorldData, request_id: HaulingRequestId) {
    let item_id = world
        .hauling_request_store()
        .get(request_id)
        .map(|request| request.item_id.clone());
    if let Some(item_id) = item_id {
        release_request_reservations(
            world.inventory_reservation_store_mut(),
            request_id,
            &item_id,
        );
    }
    if let Some(request) = world.hauling_request_store_mut().get_mut(request_id) {
        request.status = HaulingRequestStatus::Cancelled;
        request.execution_phase = HaulExecutionPhase::Failed;
        request.assigned_unit_id = None;
        request.assigned_task_id = None;
        request.reservation_state = HaulingReservationState::None;
    }
}

fn find_transferable_stack(
    world: &WorldData,
    inventory_id: InventoryId,
    item_id: &ItemDefinitionId,
    quantity: u32,
) -> Result<crate::world::inventory::EntryIndex, HaulingBlockingReason> {
    let record = world
        .inventory_store()
        .get(inventory_id)
        .ok_or(HaulingBlockingReason::InventoryRemoved)?;
    let available = count_stack_item(record, item_id);
    if available < quantity {
        return Err(HaulingBlockingReason::NoAvailableItems);
    }
    for (index, entry) in record.placed_entries().iter().enumerate() {
        if let InventoryEntryContents::Stack {
            item_definition_id,
            quantity: qty,
        } = &entry.contents
        {
            if item_definition_id == item_id && *qty >= quantity {
                return Ok(index);
            }
        }
    }
    Err(HaulingBlockingReason::NoAvailableItems)
}
