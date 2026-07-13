use bevy::prelude::*;

/// Stable interior profile identifier (ADR-084 B7).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect)]
pub struct InteriorProfileId(pub String);

impl InteriorProfileId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Authoritative door instance identifier (ADR-084 B7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Reflect)]
pub struct DoorId(pub u32);

impl DoorId {
    pub const fn new(id: u32) -> Self {
        Self(id)
    }

    pub const fn raw(self) -> u32 {
        self.0
    }
}
