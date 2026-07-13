use bevy::prelude::*;

/// Stable string identifier for a building type (B1).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect)]
pub struct BuildingDefinitionId(pub String);

impl BuildingDefinitionId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for BuildingDefinitionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
