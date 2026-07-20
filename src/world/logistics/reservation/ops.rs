//! Inventory reservation operations (EP7).

use crate::world::inventory::{InventoryCatalogCtx, count_stack_item};
use crate::world::{InventoryId, ItemDefinitionId};

use super::store::{InventoryReservationStore, RequestReservationRecord};
use super::super::id::HaulingRequestId;
use super::super::types::HaulingBlockingReason;

/// Available stack quantity after reservations (EP7).
pub fn available_stack_quantity(
    inventory_store: &crate::world::InventoryStore,
    reservations: &InventoryReservationStore,
    inventory_id: InventoryId,
    item_id: &ItemDefinitionId,
) -> u32 {
    let physical = inventory_store
        .get(inventory_id)
        .map(|record| count_stack_item(record, item_id))
        .unwrap_or(0);
    let reserved = reservations.reserved_source_quantity(inventory_id, item_id);
    physical.saturating_sub(reserved)
}

/// Reserve source items for a hauling request (EP7).
pub fn reserve_source_items(
    reservations: &mut InventoryReservationStore,
    request_id: HaulingRequestId,
    inventory_id: InventoryId,
    item_id: &ItemDefinitionId,
    quantity: u32,
    inventory_store: &crate::world::InventoryStore,
) -> Result<(), HaulingBlockingReason> {
    if quantity == 0 {
        return Err(HaulingBlockingReason::NoAvailableItems);
    }
    let available = available_stack_quantity(inventory_store, reservations, inventory_id, item_id);
    if available < quantity {
        return Err(HaulingBlockingReason::NoAvailableItems);
    }
    let key = (inventory_id, item_id.clone());
    *reservations.source_totals_mut().entry(key).or_insert(0) += quantity;
    let record = reservations
        .request_records_mut()
        .entry(request_id)
        .or_insert(RequestReservationRecord {
            request_id,
            source: None,
            destination: None,
        });
    record.source = Some(super::store::ItemReservation {
        inventory_id,
        item_id_hash: 0,
        quantity,
    });
    Ok(())
}

/// Reserve destination capacity for a hauling request (EP7).
pub fn reserve_destination_capacity(
    reservations: &mut InventoryReservationStore,
    request_id: HaulingRequestId,
    inventory_id: InventoryId,
    quantity: u32,
    inventory_store: &crate::world::InventoryStore,
    inventory_ctx: &InventoryCatalogCtx<'_>,
    item_id: &ItemDefinitionId,
) -> Result<(), HaulingBlockingReason> {
    if quantity == 0 {
        return Err(HaulingBlockingReason::DestinationFull);
    }
    if !can_accept_quantity(
        inventory_store,
        reservations,
        inventory_ctx,
        inventory_id,
        item_id,
        quantity,
    ) {
        return Err(HaulingBlockingReason::DestinationFull);
    }
    *reservations
        .destination_totals_mut()
        .entry(inventory_id)
        .or_insert(0) += quantity;
    let record = reservations
        .request_records_mut()
        .entry(request_id)
        .or_insert(RequestReservationRecord {
            request_id,
            source: None,
            destination: None,
        });
    record.destination = Some(super::store::CapacityReservation {
        inventory_id,
        quantity,
    });
    Ok(())
}

/// Release all reservations held by one request (EP7).
pub fn release_request_reservations(
    reservations: &mut InventoryReservationStore,
    request_id: HaulingRequestId,
    item_id: &ItemDefinitionId,
) {
    let Some(record) = reservations.request_records_mut().remove(&request_id) else {
        return;
    };
    if let Some(source) = record.source {
        let key = (source.inventory_id, item_id.clone());
        if let Some(total) = reservations.source_totals_mut().get_mut(&key) {
            *total = total.saturating_sub(source.quantity);
            if *total == 0 {
                reservations.source_totals_mut().remove(&key);
            }
        }
    }
    if let Some(destination) = record.destination {
        if let Some(total) = reservations
            .destination_totals_mut()
            .get_mut(&destination.inventory_id)
        {
            *total = total.saturating_sub(destination.quantity);
            if *total == 0 {
                reservations.destination_totals_mut().remove(&destination.inventory_id);
            }
        }
    }
}

fn can_accept_quantity(
    inventory_store: &crate::world::InventoryStore,
    _reservations: &InventoryReservationStore,
    inventory_ctx: &InventoryCatalogCtx<'_>,
    inventory_id: InventoryId,
    item_id: &ItemDefinitionId,
    quantity: u32,
) -> bool {
    let Some(record) = inventory_store.get(inventory_id) else {
        return false;
    };
    let mut sim = record.clone();
    simulate_place_stack_quantity(&mut sim, inventory_ctx, item_id, quantity)
}

fn simulate_place_stack_quantity(
    record: &mut crate::world::inventory::InventoryRecord,
    ctx: &InventoryCatalogCtx<'_>,
    item_id: &ItemDefinitionId,
    mut quantity: u32,
) -> bool {
    use crate::world::inventory::{
        InventoryError, PlacedInventoryEntry, can_place_entry, first_fit_position,
    };
    if quantity == 0 {
        return true;
    }
    let Ok(item) = ctx.require_item(item_id) else {
        return false;
    };
    if item.unique_instance_required || !item.stackable {
        return false;
    }
    let Ok(limit) = ctx.stack_limit_for(item, record.profile_id()) else {
        return false;
    };
    while quantity > 0 {
        let chunk = quantity.min(limit);
        let Ok((anchor_x, anchor_y)) =
            first_fit_position(record, item.grid_width, item.grid_height)
        else {
            return false;
        };
        let entry = PlacedInventoryEntry::stack(anchor_x, anchor_y, item_id.clone(), chunk);
        if can_place_entry(record, &entry, item_id, None, ctx).is_err() {
            return false;
        }
        record.placed_entries_mut().push(entry);
        if record
            .rebuild_derived(ctx, |id| Err(InventoryError::ItemInstanceNotFound(id)))
            .is_err()
        {
            return false;
        }
        quantity = quantity.saturating_sub(chunk);
    }
    true
}
