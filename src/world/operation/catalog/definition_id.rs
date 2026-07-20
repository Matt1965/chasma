//! Stable catalog identifier for operation definitions (EP3).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Stable catalog id for a production operation definition (EP3).
///
/// Runtime [`BuildingOperationPolicy`] stores only this id; definitions live in
/// [`super::registry::OperationCatalog`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect, Serialize, Deserialize)]
pub struct OperationDefinitionId(pub String);

/// Authoritative operation reference alias (EP3).
pub type OperationId = OperationDefinitionId;

impl OperationDefinitionId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for OperationDefinitionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
