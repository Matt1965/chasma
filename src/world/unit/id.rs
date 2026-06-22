use bevy::prelude::*;

/// Stable identifier for a unit instance (ADR-027 U2).
///
/// Unlike [`crate::world::ChunkId`], identity is **not** derived from world
/// position. IDs are assigned monotonically by [`crate::world::WorldData`] so
/// records remain addressable across moves, combat, and persistence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Reflect)]
pub struct UnitId(pub u64);

impl UnitId {
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }
}
