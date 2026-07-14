use bevy::prelude::*;

/// Stable identifier for a world item pile (ADR-090 I4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Reflect)]
pub struct ItemPileId(pub u64);

impl ItemPileId {
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

impl std::fmt::Display for ItemPileId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ItemPile({})", self.0)
    }
}
