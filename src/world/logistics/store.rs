//! Authoritative hauling request storage on WorldData (EP7).

use std::collections::{BTreeMap, HashMap};

use bevy::prelude::*;

use super::id::HaulingRequestId;
use super::request::HaulingRequest;
use super::types::HaulingRequestStatus;
use crate::world::{BuildingId, InventoryId, ItemDefinitionId, UnitId};

#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct HaulingRequestStore {
    next_request_id: u32,
    requests: BTreeMap<HaulingRequestId, HaulingRequest>,
    building_requests: HashMap<BuildingId, Vec<HaulingRequestId>>,
    open_by_key: HashMap<(InventoryId, InventoryId, ItemDefinitionId), HaulingRequestId>,
}

impl Default for HaulingRequestStore {
    fn default() -> Self {
        Self {
            next_request_id: 1,
            requests: BTreeMap::new(),
            building_requests: HashMap::new(),
            open_by_key: HashMap::new(),
        }
    }
}

impl HaulingRequestStore {
    pub fn allocate_id(&mut self) -> HaulingRequestId {
        if self.next_request_id == 0 {
            self.next_request_id = 1;
        }
        let id = HaulingRequestId::new(self.next_request_id);
        self.next_request_id += 1;
        id
    }

    pub fn get(&self, id: HaulingRequestId) -> Option<&HaulingRequest> {
        self.requests.get(&id)
    }

    pub fn get_mut(&mut self, id: HaulingRequestId) -> Option<&mut HaulingRequest> {
        self.requests.get_mut(&id)
    }

    pub fn requests_for_building(&self, building_id: BuildingId) -> &[HaulingRequestId] {
        self.building_requests
            .get(&building_id)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub fn open_request_for_key(
        &self,
        source: InventoryId,
        destination: InventoryId,
        item_id: &ItemDefinitionId,
    ) -> Option<HaulingRequestId> {
        self.open_by_key
            .get(&(source, destination, item_id.clone()))
            .copied()
            .and_then(|id| {
                self.get(id)
                    .filter(|request| request.status.is_consolidatable())
                    .map(|_| id)
            })
    }

    pub fn blocked_request_for_key(
        &self,
        source: InventoryId,
        destination: InventoryId,
        item_id: &ItemDefinitionId,
    ) -> Option<HaulingRequestId> {
        self.sorted_request_ids().into_iter().find(|id| {
            self.get(*id).is_some_and(|request| {
                request.status == HaulingRequestStatus::Blocked
                    && request.source_inventory_id == source
                    && request.destination_inventory_id == destination
                    && request.item_id == *item_id
            })
        })
    }

    pub fn refresh_open_key(&mut self, id: HaulingRequestId) {
        let Some(request) = self.requests.get(&id) else {
            return;
        };
        let key = request.consolidation_key();
        if request.status.is_consolidatable() {
            self.open_by_key.insert(key, id);
        } else {
            self.open_by_key.remove(&key);
        }
    }

    pub fn sorted_request_ids(&self) -> Vec<HaulingRequestId> {
        self.requests.keys().copied().collect()
    }

    pub fn insert(&mut self, request: HaulingRequest) {
        let building_id = request.owning_building_id;
        let key = request.consolidation_key();
        let id = request.id;
        self.building_requests
            .entry(building_id)
            .or_default()
            .push(id);
        if request.status.is_consolidatable() {
            self.open_by_key.insert(key, id);
        }
        let next = id.raw().saturating_add(1);
        self.next_request_id = self.next_request_id.max(next);
        self.requests.insert(id, request);
    }

    pub fn remove(&mut self, id: HaulingRequestId) -> Option<HaulingRequest> {
        let request = self.requests.remove(&id)?;
        self.open_by_key.remove(&request.consolidation_key());
        if let Some(ids) = self.building_requests.get_mut(&request.owning_building_id) {
            ids.retain(|entry| *entry != id);
        }
        Some(request)
    }

    pub fn cancel_requests_for_building(
        &mut self,
        building_id: BuildingId,
    ) -> Vec<HaulingRequestId> {
        let ids: Vec<_> = self
            .requests_for_building(building_id)
            .iter()
            .copied()
            .collect();
        for id in &ids {
            if let Some(request) = self.requests.get_mut(id) {
                request.status = HaulingRequestStatus::Cancelled;
                self.open_by_key.remove(&request.consolidation_key());
            }
        }
        ids
    }

    pub fn cancel_requests_for_inventory(
        &mut self,
        inventory_id: InventoryId,
    ) -> Vec<HaulingRequestId> {
        let mut cancelled = Vec::new();
        for request in self.requests.values_mut() {
            if request.status.is_consolidatable()
                && (request.source_inventory_id == inventory_id
                    || request.destination_inventory_id == inventory_id)
            {
                request.status = HaulingRequestStatus::Cancelled;
                self.open_by_key.remove(&request.consolidation_key());
                cancelled.push(request.id);
            }
        }
        cancelled
    }

    pub fn clear(&mut self) {
        self.next_request_id = 1;
        self.requests.clear();
        self.building_requests.clear();
        self.open_by_key.clear();
    }

    pub fn next_request_id_value(&self) -> u32 {
        self.next_request_id
    }

    pub fn restore_next_request_id(&mut self, next: u32) {
        self.next_request_id = self.next_request_id.max(next.max(1));
    }

    pub fn assigned_unit(&self, id: HaulingRequestId) -> Option<UnitId> {
        self.get(id).and_then(|request| request.assigned_unit_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hauling_request_ids_start_at_one() {
        let mut store = HaulingRequestStore::default();
        let first = store.allocate_id();
        assert_eq!(first, HaulingRequestId::new(1));
        assert!(first.is_valid());
        assert_ne!(first, HaulingRequestId::INVALID);

        let second = store.allocate_id();
        let third = store.allocate_id();
        assert_eq!(second, HaulingRequestId::new(2));
        assert_eq!(third, HaulingRequestId::new(3));
        for id in [first, second, third] {
            assert_ne!(id, HaulingRequestId::INVALID);
        }
    }
}
