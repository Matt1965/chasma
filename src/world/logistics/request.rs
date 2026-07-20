//! Authoritative hauling request record (EP7).

use bevy::prelude::*;

use super::id::HaulingRequestId;
use super::types::{
    HaulExecutionPhase, HaulingBlockingReason, HaulingGenerationReason, HaulingRequestPriority,
    HaulingRequestStatus, HaulingReservationState,
};
use crate::world::{BuildingId, InventoryId, ItemDefinitionId, TaskId, UnitId};

/// One authoritative hauling request on [`WorldData`] (EP7).
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct HaulingRequest {
    pub id: HaulingRequestId,
    pub priority: HaulingRequestPriority,
    pub item_id: ItemDefinitionId,
    pub quantity: u32,
    pub remaining_quantity: u32,
    pub source_inventory_id: InventoryId,
    pub destination_inventory_id: InventoryId,
    pub owning_building_id: BuildingId,
    pub generation_reason: HaulingGenerationReason,
    pub status: HaulingRequestStatus,
    pub reservation_state: HaulingReservationState,
    pub assigned_unit_id: Option<UnitId>,
    pub assigned_task_id: Option<TaskId>,
    pub blocking_reason: Option<HaulingBlockingReason>,
    pub execution_phase: HaulExecutionPhase,
    pub picked_up_quantity: u32,
    pub created_tick: u64,
}

impl HaulingRequest {
    pub fn new(
        id: HaulingRequestId,
        priority: HaulingRequestPriority,
        item_id: ItemDefinitionId,
        quantity: u32,
        source_inventory_id: InventoryId,
        destination_inventory_id: InventoryId,
        owning_building_id: BuildingId,
        generation_reason: HaulingGenerationReason,
        created_tick: u64,
    ) -> Self {
        Self {
            id,
            priority,
            item_id,
            quantity,
            remaining_quantity: quantity,
            source_inventory_id,
            destination_inventory_id,
            owning_building_id,
            generation_reason,
            status: HaulingRequestStatus::Pending,
            reservation_state: HaulingReservationState::None,
            assigned_unit_id: None,
            assigned_task_id: None,
            blocking_reason: None,
            execution_phase: HaulExecutionPhase::Pending,
            picked_up_quantity: 0,
            created_tick,
        }
    }

    pub fn consolidation_key(&self) -> (InventoryId, InventoryId, ItemDefinitionId) {
        (
            self.source_inventory_id,
            self.destination_inventory_id,
            self.item_id.clone(),
        )
    }
}
