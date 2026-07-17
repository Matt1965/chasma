//! Authoritative terrain field storage on [`crate::world::WorldData`] (ADR-101).

use std::collections::BTreeMap;

use bevy::prelude::*;

use super::contract::TERRAIN_FIELD_BYTES_PER_TILE;
use super::error::TerrainFieldStorageError;
use super::id::TerrainFieldId;
use super::layer::TerrainFieldLayer;
use super::tile::TerrainFieldTile;
use crate::world::ChunkCoord;

/// World-scale terrain field authority keyed by [`TerrainFieldId`].
#[derive(Debug, Clone, PartialEq, Reflect, Default)]
pub struct TerrainFieldStore {
    layers: BTreeMap<TerrainFieldId, TerrainFieldLayer>,
    store_revision: u64,
}

impl TerrainFieldStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn store_revision(&self) -> u64 {
        self.store_revision
    }

    pub fn sorted_field_ids(&self) -> Vec<TerrainFieldId> {
        self.layers.keys().cloned().collect()
    }

    pub fn has_field_data(&self, field_id: &TerrainFieldId) -> bool {
        self.layers
            .get(field_id)
            .is_some_and(|layer| !layer.tiles.is_empty())
    }

    pub fn get_layer(&self, field_id: &TerrainFieldId) -> Option<&TerrainFieldLayer> {
        self.layers.get(field_id)
    }

    pub fn get_layer_mut(&mut self, field_id: &TerrainFieldId) -> Option<&mut TerrainFieldLayer> {
        self.layers.get_mut(field_id)
    }

    pub fn get_tile(
        &self,
        field_id: &TerrainFieldId,
        chunk: ChunkCoord,
    ) -> Option<&TerrainFieldTile> {
        self.layers.get(field_id)?.get_tile(chunk)
    }

    pub fn insert_layer(
        &mut self,
        layer: TerrainFieldLayer,
    ) -> Result<(), TerrainFieldStorageError> {
        let field_id = layer.field_id.clone();
        if self.layers.insert(field_id.clone(), layer).is_some() {
            return Err(TerrainFieldStorageError::DuplicateLayer(field_id));
        }
        self.bump_store_revision();
        Ok(())
    }

    pub fn ensure_layer(
        &mut self,
        field_id: TerrainFieldId,
        source_version: impl Into<String>,
    ) -> &mut TerrainFieldLayer {
        if !self.layers.contains_key(&field_id) {
            self.layers.insert(
                field_id.clone(),
                TerrainFieldLayer::new(field_id.clone(), source_version),
            );
            self.bump_store_revision();
        }
        self.layers.get_mut(&field_id).expect("layer exists")
    }

    pub fn insert_tile(
        &mut self,
        field_id: TerrainFieldId,
        tile: TerrainFieldTile,
        source_version: impl Into<String>,
    ) -> Result<(), TerrainFieldStorageError> {
        let layer = self.ensure_layer(field_id.clone(), source_version);
        layer.insert_tile(tile)?;
        self.bump_store_revision();
        Ok(())
    }

    pub fn replace_tile(
        &mut self,
        field_id: TerrainFieldId,
        tile: TerrainFieldTile,
        source_version: impl Into<String>,
    ) -> Result<(), TerrainFieldStorageError> {
        let layer = self.ensure_layer(field_id, source_version);
        layer.replace_tile(tile)?;
        self.bump_store_revision();
        Ok(())
    }

    pub fn remove_tile(
        &mut self,
        field_id: &TerrainFieldId,
        chunk: ChunkCoord,
    ) -> Option<TerrainFieldTile> {
        let removed = self.layers.get_mut(field_id)?.remove_tile(chunk);
        if removed.is_some() {
            self.bump_store_revision();
        }
        removed
    }

    pub fn remove_layer(&mut self, field_id: &TerrainFieldId) -> Option<TerrainFieldLayer> {
        let removed = self.layers.remove(field_id);
        if removed.is_some() {
            self.bump_store_revision();
        }
        removed
    }

    pub fn clear(&mut self) {
        if !self.layers.is_empty() {
            self.layers.clear();
            self.bump_store_revision();
        }
    }

    pub fn memory_bytes(&self) -> usize {
        self.layers
            .values()
            .map(|layer| layer.tiles.len() * TERRAIN_FIELD_BYTES_PER_TILE)
            .sum()
    }

    pub fn memory_bytes_for_field(&self, field_id: &TerrainFieldId) -> usize {
        self.layers
            .get(field_id)
            .map(TerrainFieldLayer::memory_bytes)
            .unwrap_or(0)
    }

    pub fn validate_shared_edges(&self) -> Result<(), TerrainFieldStorageError> {
        for layer in self.layers.values() {
            layer.validate_shared_edges()?;
        }
        Ok(())
    }

    fn bump_store_revision(&mut self) {
        self.store_revision = self.store_revision.saturating_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::terrain_field::contract::TERRAIN_FIELD_SAMPLES_PER_EDGE;

    #[test]
    fn insert_and_fetch_tile() {
        let mut store = TerrainFieldStore::new();
        let field_id = TerrainFieldId::new("water");
        let chunk = ChunkCoord::new(0, 0);
        let tile = TerrainFieldTile::new_constant(chunk, 42_000, "test");
        store.insert_tile(field_id.clone(), tile, "test").unwrap();
        assert_eq!(
            store.get_tile(&field_id, chunk).unwrap().samples_per_edge,
            TERRAIN_FIELD_SAMPLES_PER_EDGE
        );
    }
}
