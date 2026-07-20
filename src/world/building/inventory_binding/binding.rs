//! Runtime building inventory binding (EP4).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::world::InventoryId;

use super::binding_id::BuildingInventoryBindingId;
use super::role::BuildingInventoryRole;

/// One runtime inventory channel owned by a building (EP4).
#[derive(Debug, Clone, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub struct BuildingInventoryBinding {
    pub binding_id: BuildingInventoryBindingId,
    pub role: BuildingInventoryRole,
    pub inventory_id: InventoryId,
    pub label: Option<String>,
    pub is_default: bool,
}

impl BuildingInventoryBinding {
    pub fn new(
        binding_id: impl Into<BuildingInventoryBindingId>,
        role: BuildingInventoryRole,
        inventory_id: InventoryId,
    ) -> Self {
        Self {
            binding_id: binding_id.into(),
            role,
            inventory_id,
            label: None,
            is_default: false,
        }
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn with_default(mut self, is_default: bool) -> Self {
        self.is_default = is_default;
        self
    }
}
