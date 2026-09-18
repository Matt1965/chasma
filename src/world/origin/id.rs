use bevy::prelude::*;

/// Stable identifier for a starting-origin definition (CG7).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect)]
pub struct OriginId(pub String);

impl OriginId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
