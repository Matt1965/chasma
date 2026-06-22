use bevy::prelude::*;

/// Stable string identifier for a unit type definition (ADR-027).
///
/// Distinct from future runtime [`UnitId`]: catalog ids come from Excel `Unit ID`
/// and identify the type, not a world instance.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Reflect)]
pub struct UnitDefinitionId(pub String);

impl UnitDefinitionId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for UnitDefinitionId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}
