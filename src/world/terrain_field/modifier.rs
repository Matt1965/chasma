//! Future runtime field modifier storage (ADR-106 TF6 seam).

use std::collections::BTreeMap;

use bevy::prelude::*;

use super::id::TerrainFieldId;
use crate::world::ChunkCoord;

/// Modifier application mode for future gameplay systems.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum TerrainFieldModifierKind {
    AdditiveDelta,
    MultiplicativeFactor,
    Override,
    Clamp,
}

/// One sparse runtime modifier sample (not active in TF6).
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct TerrainFieldModifierEntry {
    pub kind: TerrainFieldModifierKind,
    pub value: u16,
}

/// Sparse per-chunk modifier tiles for future runtime state (ADR-106 TF6).
///
/// Empty by default. Saves may persist this store later; base tiles remain in the world package.
#[derive(Debug, Clone, Default, PartialEq, Reflect)]
pub struct TerrainFieldModifierStore {
    layers: BTreeMap<TerrainFieldId, BTreeMap<ChunkCoord, TerrainFieldModifierEntry>>,
}

impl TerrainFieldModifierStore {
    pub fn is_empty(&self) -> bool {
        self.layers.is_empty()
    }

    pub fn get(
        &self,
        field_id: &TerrainFieldId,
        chunk: ChunkCoord,
    ) -> Option<&TerrainFieldModifierEntry> {
        self.layers.get(field_id)?.get(&chunk)
    }

    pub fn set(
        &mut self,
        field_id: TerrainFieldId,
        chunk: ChunkCoord,
        entry: TerrainFieldModifierEntry,
    ) {
        self.layers
            .entry(field_id)
            .or_default()
            .insert(chunk, entry);
    }

    pub fn clear_field(&mut self, field_id: &TerrainFieldId) {
        self.layers.remove(field_id);
    }

    pub fn clear(&mut self) {
        self.layers.clear();
    }
}
