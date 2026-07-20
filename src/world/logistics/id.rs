//! Typed identifiers for logistics runtime (EP7).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Authoritative hauling request identity (EP7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Reflect, Serialize, Deserialize)]
pub struct HaulingRequestId(pub u32);

impl HaulingRequestId {
    pub const INVALID: Self = Self(0);

    pub fn new(raw: u32) -> Self {
        Self(raw)
    }

    pub fn raw(self) -> u32 {
        self.0
    }

    pub fn is_valid(self) -> bool {
        self.0 != 0
    }
}

impl std::fmt::Display for HaulingRequestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HaulingRequestId({})", self.0)
    }
}
