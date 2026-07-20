use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Authoritative inventory container instance identifier (ADR-088 I2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Reflect, Serialize, Deserialize)]
pub struct InventoryId(pub u32);

impl InventoryId {
    pub const INVALID: Self = Self(0);

    pub const fn new(id: u32) -> Self {
        Self(id)
    }

    pub const fn raw(self) -> u32 {
        self.0
    }

    pub const fn is_valid(self) -> bool {
        self.0 != 0
    }
}

/// Authoritative unique item instance identifier (ADR-088 I2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Reflect)]
pub struct ItemInstanceId(pub u32);

impl ItemInstanceId {
    pub const INVALID: Self = Self(0);

    pub const fn new(id: u32) -> Self {
        Self(id)
    }

    pub const fn raw(self) -> u32 {
        self.0
    }

    pub const fn is_valid(self) -> bool {
        self.0 != 0
    }
}
