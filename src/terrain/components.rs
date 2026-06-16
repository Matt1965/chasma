//! Terrain Runtime Layer ECS components (ADR-010).

use bevy::prelude::*;

use crate::world::ChunkId;

use super::mesh::ChunkLod;

/// Marks a derived terrain render entity and links it back to the authoritative
/// chunk it was built from.
///
/// The entity owns only rendering data (`Mesh3d`, material, `Transform`); the
/// authoritative terrain lives in [`crate::world::WorldData`]. This marker
/// carries the [`ChunkId`] and the currently displayed mesh LOD.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub struct TerrainChunkMesh {
    pub chunk: ChunkId,
    pub active_lod: ChunkLod,
}

impl TerrainChunkMesh {
    pub const fn new(chunk: ChunkId, active_lod: ChunkLod) -> Self {
        Self { chunk, active_lod }
    }
}
