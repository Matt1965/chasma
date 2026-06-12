//! Terrain Runtime Layer ECS components (ADR-010).

use bevy::prelude::*;

use crate::world::ChunkId;

/// Marks a derived terrain render entity and links it back to the authoritative
/// chunk it was built from.
///
/// The entity owns only rendering data (`Mesh3d`, material, `Transform`); the
/// authoritative terrain lives in [`crate::world::WorldData`]. This marker
/// carries the [`ChunkId`] only, so the render entity can be reconciled with or
/// disposed alongside its source chunk in later phases (streaming, ADR-012).
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub struct TerrainChunkMesh {
    pub chunk: ChunkId,
}

impl TerrainChunkMesh {
    pub const fn new(chunk: ChunkId) -> Self {
        Self { chunk }
    }
}
