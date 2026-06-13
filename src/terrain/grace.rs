//! Same-frame apply grace guard (ADR-012 Phase 2B.5 step 4).

use std::collections::HashSet;

use bevy::prelude::*;

use crate::world::ChunkId;

/// Chunks applied this frame that must not be unloaded until the next tick.
#[derive(Resource, Default)]
pub struct JustAppliedGrace {
    chunks: HashSet<ChunkId>,
}

impl JustAppliedGrace {
    pub fn grant(&mut self, chunk_id: ChunkId) {
        self.chunks.insert(chunk_id);
    }

    pub fn as_set(&self) -> &HashSet<ChunkId> {
        &self.chunks
    }

    pub fn clear(&mut self) {
        self.chunks.clear();
    }
}
