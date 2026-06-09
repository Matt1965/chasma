use bevy::prelude::*;

use super::coordinates::ChunkCoord;

/// The authoritative identity of a chunk.
///
/// `ChunkId` is a 1:1, deterministic newtype over [`ChunkCoord`]: a chunk's
/// identity is fully derived from its position on the grid, with no separately
/// generated id and no shared registry (ADR-001 addendum). This keeps identity
/// stable across save/load and across machines, which is the
/// persistence- and multiplayer-friendly choice.
///
/// [`ChunkCoord`] is the type used for grid arithmetic; `ChunkId` is the type
/// used to name a chunk (for example as a lookup or persistence key).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub struct ChunkId(pub ChunkCoord);

impl ChunkId {
    pub const fn new(coord: ChunkCoord) -> Self {
        Self(coord)
    }

    pub const fn coord(self) -> ChunkCoord {
        self.0
    }
}

impl From<ChunkCoord> for ChunkId {
    fn from(coord: ChunkCoord) -> Self {
        Self(coord)
    }
}

impl From<ChunkId> for ChunkCoord {
    fn from(id: ChunkId) -> Self {
        id.0
    }
}
