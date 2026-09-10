//! Blocked hauling request recovery (EP7).

use crate::world::WorldData;
use crate::world::building::catalog::BuildingCatalog;
use crate::world::inventory::InventoryCatalogCtx;

use super::reservation::release_request_reservations;
use super::types::{
    BLOCKED_HAUL_RETRY_COOLDOWN_TICKS, HaulExecutionPhase, HaulingBlockingReason,
    HaulingRequestStatus, HaulingReservationState,
};

/// Re-list recoverable blocked haul requests after cooldown.
pub fn retry_blocked_hauling_requests(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    simulation_tick: u64,
    inventory_ctx: &InventoryCatalogCtx<'_>,
) {
    let ids = world.hauling_request_store().sorted_request_ids();
    for request_id in ids {
        let Some(request) = world.hauling_request_store().get(request_id).cloned() else {
            continue;
        };
        if request.status != HaulingRequestStatus::Blocked {
            continue;
        }
        let Some(blocked_at) = request.blocked_at_tick else {
            continue;
        };
        if simulation_tick < blocked_at + BLOCKED_HAUL_RETRY_COOLDOWN_TICKS {
            continue;
        }
        let recoverable = request.blocking_reason.as_ref().is_some_and(|reason| {
            matches!(
                reason,
                HaulingBlockingReason::DestinationFull
                    | HaulingBlockingReason::ReservationFailed
                    | HaulingBlockingReason::NoAvailableItems
            )
        });
        if !recoverable || request.remaining_quantity == 0 {
            continue;
        }
        if world
            .inventory_store()
            .get(request.source_inventory_id)
            .is_none()
            || world
                .inventory_store()
                .get(request.destination_inventory_id)
                .is_none()
        {
            continue;
        }
        let _ = building_catalog;
        let _ = inventory_ctx;
        release_request_reservations(
            world.inventory_reservation_store_mut(),
            request_id,
            &request.item_id,
        );
        if let Some(request) = world.hauling_request_store_mut().get_mut(request_id) {
            request.status = HaulingRequestStatus::Pending;
            request.blocking_reason = None;
            request.blocked_at_tick = None;
            request.assigned_unit_id = None;
            request.assigned_task_id = None;
            request.reservation_state = HaulingReservationState::None;
            request.execution_phase = HaulExecutionPhase::Pending;
            world
                .hauling_request_store_mut()
                .refresh_open_key(request_id);
        }
    }
}
