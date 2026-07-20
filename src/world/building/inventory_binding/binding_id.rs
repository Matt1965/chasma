//! Stable logical channel identity for building inventory bindings (EP4).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Stable authored logical channel for a building inventory (EP4).
///
/// Distinct from [`crate::world::InventoryId`] (runtime instance) and display labels.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect, Serialize, Deserialize)]
pub struct BuildingInventoryBindingId(pub String);

impl BuildingInventoryBindingId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for BuildingInventoryBindingId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for BuildingInventoryBindingId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}
