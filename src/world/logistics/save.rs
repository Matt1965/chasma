//! Logistics persistence (EP7).

use serde::{Deserialize, Serialize};

use crate::world::{BuildingId, InventoryId, ItemDefinitionId, TaskId, UnitId, WorldData};

use super::id::HaulingRequestId;
use super::request::HaulingRequest;
use super::reservation::InventoryReservationSaveState;
use super::types::{
    HaulExecutionPhase, HaulingBlockingReason, HaulingGenerationReason, HaulingRequestPriority,
    HaulingRequestStatus, HaulingReservationState,
};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct HaulingRequestSaveState {
    pub next_request_id: u32,
    pub requests: Vec<HaulingRequestRecord>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HaulingRequestRecord {
    pub id: u32,
    pub priority: HaulingRequestPriority,
    pub item_id: String,
    pub quantity: u32,
    pub remaining_quantity: u32,
    pub source_inventory_id: u32,
    pub destination_inventory_id: u32,
    pub owning_building_id: u64,
    pub generation_reason: HaulingGenerationReason,
    pub status: HaulingRequestStatus,
    pub reservation_state: HaulingReservationState,
    pub assigned_unit_id: Option<u64>,
    pub assigned_task_id: Option<u32>,
    pub blocking_reason: Option<HaulingBlockingReason>,
    pub execution_phase: HaulExecutionPhase,
    pub picked_up_quantity: u32,
    pub created_tick: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LogisticsSaveState {
    pub hauling_requests: HaulingRequestSaveState,
    pub reservations: InventoryReservationSaveState,
}

pub fn export_logistics_save_state(world: &WorldData) -> LogisticsSaveState {
    LogisticsSaveState {
        hauling_requests: export_hauling_requests(world),
        reservations: world.inventory_reservation_store().export_save_state(),
    }
}

pub fn import_logistics_save_state(world: &mut WorldData, state: LogisticsSaveState) {
    import_hauling_requests(world, state.hauling_requests);
    world
        .inventory_reservation_store_mut()
        .import_save_state(state.reservations);
}

fn export_hauling_requests(world: &WorldData) -> HaulingRequestSaveState {
    let store = world.hauling_request_store();
    HaulingRequestSaveState {
        next_request_id: world.hauling_request_store().next_request_id_value(),
        requests: store
            .sorted_request_ids()
            .into_iter()
            .filter_map(|id| store.get(id).map(to_record))
            .collect(),
    }
}

fn import_hauling_requests(world: &mut WorldData, state: HaulingRequestSaveState) {
    world.hauling_request_store_mut().clear();
    world.hauling_request_store_mut().import_records(state);
}

impl super::store::HaulingRequestStore {
    pub(crate) fn import_records(&mut self, state: HaulingRequestSaveState) {
        self.clear();
        self.restore_next_request_id(state.next_request_id.max(1));
        for record in state.requests {
            self.insert(from_record(record));
        }
    }
}

fn to_record(request: &HaulingRequest) -> HaulingRequestRecord {
    HaulingRequestRecord {
        id: request.id.raw(),
        priority: request.priority,
        item_id: request.item_id.as_str().to_string(),
        quantity: request.quantity,
        remaining_quantity: request.remaining_quantity,
        source_inventory_id: request.source_inventory_id.raw(),
        destination_inventory_id: request.destination_inventory_id.raw(),
        owning_building_id: request.owning_building_id.raw(),
        generation_reason: request.generation_reason.clone(),
        status: request.status,
        reservation_state: request.reservation_state,
        assigned_unit_id: request.assigned_unit_id.map(|id| id.raw()),
        assigned_task_id: request.assigned_task_id.map(|id| id.raw()),
        blocking_reason: request.blocking_reason.clone(),
        execution_phase: request.execution_phase,
        picked_up_quantity: request.picked_up_quantity,
        created_tick: request.created_tick,
    }
}

fn from_record(record: HaulingRequestRecord) -> HaulingRequest {
    HaulingRequest {
        id: HaulingRequestId::new(record.id),
        priority: record.priority,
        item_id: ItemDefinitionId::new(record.item_id),
        quantity: record.quantity,
        remaining_quantity: record.remaining_quantity,
        source_inventory_id: InventoryId::new(record.source_inventory_id),
        destination_inventory_id: InventoryId::new(record.destination_inventory_id),
        owning_building_id: BuildingId::new(record.owning_building_id),
        generation_reason: record.generation_reason,
        status: record.status,
        reservation_state: record.reservation_state,
        assigned_unit_id: record.assigned_unit_id.map(UnitId::new),
        assigned_task_id: record.assigned_task_id.map(TaskId::new),
        blocking_reason: record.blocking_reason,
        execution_phase: record.execution_phase,
        picked_up_quantity: record.picked_up_quantity,
        created_tick: record.created_tick,
    }
}
