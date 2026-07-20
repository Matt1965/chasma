//! Data-driven logistics route definitions (EP7).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::world::building::inventory_binding::BuildingInventoryBindingId;
use crate::world::{BuildingDefinitionId, ItemDefinitionId};

use crate::world::logistics::{HaulingRequestPriority, LogisticsRouteTrigger};

/// Authored logistics route on a building definition (EP7).
#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub struct BuildingLogisticsRouteDefinition {
    pub trigger: LogisticsRouteTrigger,
    pub local_binding_id: BuildingInventoryBindingId,
    pub item_id: ItemDefinitionId,
    pub remote_building_definition_id: BuildingDefinitionId,
    pub remote_binding_id: BuildingInventoryBindingId,
    pub priority: HaulingRequestPriority,
}

impl BuildingLogisticsRouteDefinition {
    pub fn output_surplus(
        local_binding_id: impl Into<BuildingInventoryBindingId>,
        item_id: impl Into<ItemDefinitionId>,
        remote_building_definition_id: impl Into<BuildingDefinitionId>,
        remote_binding_id: impl Into<BuildingInventoryBindingId>,
    ) -> Self {
        Self {
            trigger: LogisticsRouteTrigger::OutputSurplus,
            local_binding_id: local_binding_id.into(),
            item_id: item_id.into(),
            remote_building_definition_id: remote_building_definition_id.into(),
            remote_binding_id: remote_binding_id.into(),
            priority: HaulingRequestPriority::Normal,
        }
    }

    pub fn input_deficit(
        local_binding_id: impl Into<BuildingInventoryBindingId>,
        item_id: impl Into<ItemDefinitionId>,
        remote_building_definition_id: impl Into<BuildingDefinitionId>,
        remote_binding_id: impl Into<BuildingInventoryBindingId>,
    ) -> Self {
        Self {
            trigger: LogisticsRouteTrigger::InputDeficit,
            local_binding_id: local_binding_id.into(),
            item_id: item_id.into(),
            remote_building_definition_id: remote_building_definition_id.into(),
            remote_binding_id: remote_binding_id.into(),
            priority: HaulingRequestPriority::High,
        }
    }

    pub fn with_priority(mut self, priority: HaulingRequestPriority) -> Self {
        self.priority = priority;
        self
    }
}
