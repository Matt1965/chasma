//! Settlement and treasury identifiers (ADR-093 I7).

use bevy::prelude::*;

/// Authoritative settlement instance id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Reflect)]
pub struct SettlementId(pub u64);

impl SettlementId {
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }
}

/// Authoritative abstract treasury id (settlement wealth only — no inventory).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Reflect)]
pub struct TreasuryId(pub u64);

impl TreasuryId {
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }
}
