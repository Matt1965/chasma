use bevy::prelude::*;

/// Stable identifier for a doodad instance (ADR-015).
///
/// Unlike [`crate::world::ChunkId`], identity is **not** derived from world
/// position. IDs are assigned monotonically by [`crate::world::WorldData`] so
/// records remain addressable across moves, harvesting, and persistence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Reflect)]
pub struct DoodadId(pub u64);

impl DoodadId {
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }
}
