//! Authoritative inventory reservations for hauling (EP7).

use std::collections::HashMap;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::world::{InventoryId, ItemDefinitionId};

use super::super::id::HaulingRequestId;

/// Reserved quantity for one item in one inventory (EP7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub struct ItemReservation {
    pub inventory_id: InventoryId,
    pub item_id_hash: u64,
    pub quantity: u32,
}

/// Reserved destination capacity in item units (EP7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub struct CapacityReservation {
    pub inventory_id: InventoryId,
    pub quantity: u32,
}

/// Per-request reservation record (EP7).
#[derive(Debug, Clone, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub struct RequestReservationRecord {
    pub request_id: HaulingRequestId,
    pub source: Option<ItemReservation>,
    pub destination: Option<CapacityReservation>,
}

#[derive(Debug, Clone, Default, PartialEq, Reflect, Serialize, Deserialize)]
pub struct InventoryReservationSaveState {
    pub source_totals: Vec<(InventoryId, String, u32)>,
    pub destination_totals: Vec<(InventoryId, u32)>,
    pub request_records: Vec<RequestReservationRecord>,
}

/// Authoritative hauling reservations on [`WorldData`] (EP7).
#[derive(Debug, Clone, Default, PartialEq, Reflect)]
pub struct InventoryReservationStore {
    source_totals: HashMap<(InventoryId, ItemDefinitionId), u32>,
    destination_totals: HashMap<InventoryId, u32>,
    request_records: HashMap<HaulingRequestId, RequestReservationRecord>,
}

impl InventoryReservationStore {
    pub fn clear(&mut self) {
        self.source_totals.clear();
        self.destination_totals.clear();
        self.request_records.clear();
    }

    pub fn reserved_source_quantity(
        &self,
        inventory_id: InventoryId,
        item_id: &ItemDefinitionId,
    ) -> u32 {
        self.source_totals
            .get(&(inventory_id, item_id.clone()))
            .copied()
            .unwrap_or(0)
    }

    pub fn reserved_destination_capacity(&self, inventory_id: InventoryId) -> u32 {
        self.destination_totals.get(&inventory_id).copied().unwrap_or(0)
    }

    pub fn request_record(&self, request_id: HaulingRequestId) -> Option<&RequestReservationRecord> {
        self.request_records.get(&request_id)
    }

    pub(crate) fn source_totals_mut(
        &mut self,
    ) -> &mut HashMap<(InventoryId, ItemDefinitionId), u32> {
        &mut self.source_totals
    }

    pub(crate) fn destination_totals_mut(&mut self) -> &mut HashMap<InventoryId, u32> {
        &mut self.destination_totals
    }

    pub(crate) fn request_records_mut(
        &mut self,
    ) -> &mut HashMap<HaulingRequestId, RequestReservationRecord> {
        &mut self.request_records
    }

    pub fn export_save_state(&self) -> InventoryReservationSaveState {
        InventoryReservationSaveState {
            source_totals: self
                .source_totals
                .iter()
                .map(|((inventory_id, item_id), quantity)| (*inventory_id, item_id.as_str().to_string(), *quantity))
                .collect(),
            destination_totals: self
                .destination_totals
                .iter()
                .map(|(inventory_id, quantity)| (*inventory_id, *quantity))
                .collect(),
            request_records: self.request_records.values().cloned().collect(),
        }
    }

    pub fn import_save_state(&mut self, state: InventoryReservationSaveState) {
        self.clear();
        for (inventory_id, item_id, quantity) in state.source_totals {
            self.source_totals
                .insert((inventory_id, ItemDefinitionId::new(item_id)), quantity);
        }
        for (inventory_id, quantity) in state.destination_totals {
            self.destination_totals.insert(inventory_id, quantity);
        }
        for record in state.request_records {
            self.request_records.insert(record.request_id, record);
        }
    }
}
