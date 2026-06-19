use bevy::prelude::*;

/// Stable string identifier for a doodad type definition (ADR-016).
///
/// Distinct from [`super::super::kind::DoodadKind`]: multiple definitions may share
/// a kind (e.g. `tree_oak` and `tree_dead` are both [`DoodadKind::Tree`]).
/// Procedural generation and persistence should reference this id, not the kind enum.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Reflect)]
pub struct DoodadDefinitionId(pub String);

impl DoodadDefinitionId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for DoodadDefinitionId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}
