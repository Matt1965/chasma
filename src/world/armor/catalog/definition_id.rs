use bevy::prelude::*;
use std::fmt;

/// Stable authored identifier for an armor profile definition.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect)]
pub struct ArmorProfileId(pub String);

impl ArmorProfileId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ArmorProfileId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
