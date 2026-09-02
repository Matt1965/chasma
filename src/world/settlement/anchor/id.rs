//! Settlement anchor identifiers (ADR-133).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Authoritative settlement anchor instance id.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Reflect, Serialize, Deserialize,
)]
pub struct SettlementAnchorId(pub u64);

impl SettlementAnchorId {
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }
}
