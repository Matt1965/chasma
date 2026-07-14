use bevy::prelude::*;

/// Authoritative corpse instance identifier (ADR-089 I3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Reflect)]
pub struct CorpseId(pub u64);

impl CorpseId {
    pub const INVALID: Self = Self(0);

    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }

    pub const fn is_valid(self) -> bool {
        self.0 != 0
    }
}
