//! Authored inventory layout on building definitions (EP4).

use bevy::prelude::*;

use crate::world::InventoryProfileId;

use super::binding_id::BuildingInventoryBindingId;
use super::role::BuildingInventoryRole;

/// Authored inventory channel on a [`crate::world::BuildingDefinition`] (EP4).
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct BuildingInventoryBindingDefinition {
    pub binding_id: BuildingInventoryBindingId,
    pub role: BuildingInventoryRole,
    pub profile_id: InventoryProfileId,
    pub label: Option<String>,
    pub is_default: bool,
}

impl BuildingInventoryBindingDefinition {
    pub fn new(
        binding_id: impl Into<BuildingInventoryBindingId>,
        role: BuildingInventoryRole,
        profile_id: InventoryProfileId,
    ) -> Self {
        Self {
            binding_id: binding_id.into(),
            role,
            profile_id,
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

    /// Legacy I5 single-container migration seam.
    pub fn legacy_primary(profile_id: InventoryProfileId) -> Self {
        Self {
            binding_id: BuildingInventoryBindingId::new("primary"),
            role: BuildingInventoryRole::General,
            profile_id,
            label: Some("Primary".into()),
            is_default: true,
        }
    }
}

impl From<BuildingInventoryBindingDefinition> for BuildingInventoryBindingId {
    fn from(value: BuildingInventoryBindingDefinition) -> Self {
        value.binding_id
    }
}

impl From<&BuildingInventoryBindingDefinition> for BuildingInventoryBindingId {
    fn from(value: &BuildingInventoryBindingDefinition) -> Self {
        value.binding_id.clone()
    }
}
