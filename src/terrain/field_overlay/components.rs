//! Terrain field overlay render markers (ADR-103).

use bevy::prelude::*;

use crate::world::{ChunkId, TerrainFieldId};

/// Derived field overlay mesh for one resident terrain chunk.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct TerrainFieldOverlayMesh {
    pub chunk: ChunkId,
    pub field_id: TerrainFieldId,
    pub request_revision: u64,
    pub tile_revision: u64,
}
