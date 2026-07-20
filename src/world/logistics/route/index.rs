//! Logistics endpoint index for O(1) building lookup (EP7).

use std::collections::HashMap;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::world::building::inventory_binding::BuildingInventoryBindingId;
use crate::world::{BuildingDefinitionId, BuildingId};

/// Key for a logistics endpoint (building type + binding) (EP7).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect, Serialize, Deserialize)]
pub struct LogisticsEndpointKey {
    pub building_definition_id: BuildingDefinitionId,
    pub binding_id: BuildingInventoryBindingId,
}

impl LogisticsEndpointKey {
    pub fn new(
        building_definition_id: BuildingDefinitionId,
        binding_id: BuildingInventoryBindingId,
    ) -> Self {
        Self {
            building_definition_id,
            binding_id,
        }
    }
}

/// Index of logistics endpoints by authored building+binding (EP7).
#[derive(Debug, Clone, Default, PartialEq, Reflect)]
pub struct LogisticsEndpointIndex {
    endpoints: HashMap<LogisticsEndpointKey, Vec<BuildingId>>,
}

impl LogisticsEndpointIndex {
    pub fn clear(&mut self) {
        self.endpoints.clear();
    }

    pub fn register(
        &mut self,
        building_definition_id: &BuildingDefinitionId,
        binding_id: &BuildingInventoryBindingId,
        building_id: BuildingId,
    ) {
        let key = LogisticsEndpointKey::new(building_definition_id.clone(), binding_id.clone());
        let entries = self.endpoints.entry(key).or_default();
        if !entries.contains(&building_id) {
            entries.push(building_id);
        }
    }

    pub fn unregister_building(
        &mut self,
        building_definition_id: &BuildingDefinitionId,
        binding_ids: &[BuildingInventoryBindingId],
        building_id: BuildingId,
    ) {
        for binding_id in binding_ids {
            let key = LogisticsEndpointKey::new(building_definition_id.clone(), binding_id.clone());
            if let Some(entries) = self.endpoints.get_mut(&key) {
                entries.retain(|id| *id != building_id);
                if entries.is_empty() {
                    self.endpoints.remove(&key);
                }
            }
        }
    }

    pub fn resolve(
        &self,
        building_definition_id: &BuildingDefinitionId,
        binding_id: &BuildingInventoryBindingId,
    ) -> Option<&[BuildingId]> {
        self.endpoints
            .get(&LogisticsEndpointKey::new(
                building_definition_id.clone(),
                binding_id.clone(),
            ))
            .map(Vec::as_slice)
    }
}
