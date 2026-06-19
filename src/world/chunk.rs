use bevy::prelude::*;

use super::coordinates::ChunkCoord;
use super::terrain::{Heightfield, TerrainMask, TerrainMetadata};

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

/// The authoritative definition of a single chunk's geography (ADR-002,
/// ADR-008).
///
/// A `ChunkData` owns its terrain tile, the metadata derived from it, and any
/// imported mask layers. It owns terrain geography only: per ADR-002 and
/// ADR-008 it must not own doodads, occupancy, units, settlements, factions,
/// or rendering/LOD state. Doodads are parallel world data in
/// [`crate::world::WorldData`] keyed by [`ChunkId`] (ADR-015), not fields on
/// this struct.
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct ChunkData {
    pub heightfield: Heightfield,
    pub metadata: TerrainMetadata,
    pub masks: Vec<TerrainMask>,
}

impl ChunkData {
    /// Build chunk data from a heightfield and its mask layers, deriving the
    /// terrain metadata so it always matches the heightfield.
    pub fn new(heightfield: Heightfield, masks: Vec<TerrainMask>) -> Self {
        let metadata = TerrainMetadata::from_heightfield(&heightfield);
        Self {
            heightfield,
            metadata,
            masks,
        }
    }
}
