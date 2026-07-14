use bevy::prelude::*;

use super::id::ItemPileId;
use super::record::WorldItemPileRecord;

/// Chunk-local world item piles (ADR-090 I4).
#[derive(Debug, Clone, Default, PartialEq, Reflect)]
pub struct ChunkItemPileStore {
    records: Vec<WorldItemPileRecord>,
}

impl ChunkItemPileStore {
    pub fn records(&self) -> &[WorldItemPileRecord] {
        &self.records
    }

    pub fn get(&self, id: ItemPileId) -> Option<&WorldItemPileRecord> {
        self.records
            .binary_search_by_key(&id, |record| record.id)
            .ok()
            .map(|index| &self.records[index])
    }

    pub fn get_mut(&mut self, id: ItemPileId) -> Option<&mut WorldItemPileRecord> {
        self.records
            .binary_search_by_key(&id, |record| record.id)
            .ok()
            .map(|index| &mut self.records[index])
    }

    pub fn insert(&mut self, record: WorldItemPileRecord) {
        if let Ok(index) = self
            .records
            .binary_search_by_key(&record.id, |entry| entry.id)
        {
            self.records[index] = record;
        } else {
            self.records.push(record);
            self.records.sort_by_key(|entry| entry.id);
        }
    }

    pub fn take(&mut self, id: ItemPileId) -> Option<WorldItemPileRecord> {
        if let Ok(index) = self.records.binary_search_by_key(&id, |entry| entry.id) {
            Some(self.records.remove(index))
        } else {
            None
        }
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

/// Spatial index for world item piles on [`crate::world::WorldData`] (ADR-090 I4).
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct ItemPileStore {
    next_item_pile_id: u64,
    piles: std::collections::HashMap<crate::world::ChunkId, ChunkItemPileStore>,
    pile_locations: std::collections::HashMap<ItemPileId, crate::world::ChunkId>,
}

impl Default for ItemPileStore {
    fn default() -> Self {
        Self {
            next_item_pile_id: 1,
            piles: std::collections::HashMap::new(),
            pile_locations: std::collections::HashMap::new(),
        }
    }
}

impl ItemPileStore {
    pub fn allocate_item_pile_id(&mut self) -> ItemPileId {
        let id = ItemPileId::new(self.next_item_pile_id);
        self.next_item_pile_id = self.next_item_pile_id.saturating_add(1);
        id
    }

    pub fn next_id(&self) -> u64 {
        self.next_item_pile_id
    }

    pub fn restore_next_id(&mut self, next: u64) {
        self.next_item_pile_id = self.next_item_pile_id.max(next);
    }

    pub fn sorted_item_pile_ids(&self) -> Vec<ItemPileId> {
        let mut ids: Vec<_> = self.pile_locations.keys().copied().collect();
        ids.sort();
        ids
    }

    pub fn pile_chunk(&self, id: ItemPileId) -> Option<crate::world::ChunkId> {
        self.pile_locations.get(&id).copied()
    }

    pub fn get(&self, id: ItemPileId) -> Option<&WorldItemPileRecord> {
        let chunk = self.pile_locations.get(&id)?;
        self.piles.get(chunk)?.get(id)
    }

    pub fn get_mut(&mut self, id: ItemPileId) -> Option<&mut WorldItemPileRecord> {
        let chunk = *self.pile_locations.get(&id)?;
        self.piles.get_mut(&chunk)?.get_mut(id)
    }

    pub fn piles_in_chunk(&self, chunk: crate::world::ChunkId) -> &[WorldItemPileRecord] {
        self.piles
            .get(&chunk)
            .map(|store| store.records())
            .unwrap_or(&[])
    }

    pub fn insert(
        &mut self,
        chunk: crate::world::ChunkId,
        record: WorldItemPileRecord,
    ) -> Result<(), super::error::ItemPileError> {
        if record.placement.chunk != chunk.coord() {
            return Err(super::error::ItemPileError::ChunkPlacementMismatch { pile_id: record.id });
        }
        if self.pile_locations.contains_key(&record.id) {
            return Err(super::error::ItemPileError::ItemPileIdCollision(record.id));
        }
        self.piles.entry(chunk).or_default().insert(record.clone());
        self.pile_locations.insert(record.id, chunk);
        Ok(())
    }

    pub fn remove(&mut self, id: ItemPileId) -> Option<WorldItemPileRecord> {
        let chunk = self.pile_locations.remove(&id)?;
        let record = self.piles.get_mut(&chunk)?.take(id)?;
        if self.piles.get(&chunk).is_some_and(|store| store.is_empty()) {
            self.piles.remove(&chunk);
        }
        Some(record)
    }

    pub fn clear(&mut self) {
        self.next_item_pile_id = 1;
        self.piles.clear();
        self.pile_locations.clear();
    }

    /// Replace all world item piles (scene restore — ADR-094 I8).
    pub fn restore_snapshot(
        &mut self,
        records: Vec<(crate::world::ChunkId, WorldItemPileRecord)>,
        next_id: u64,
    ) -> Result<(), super::error::ItemPileError> {
        self.clear();
        self.restore_next_id(next_id);
        for (chunk, record) in records {
            self.insert(chunk, record)?;
        }
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.pile_locations.len()
    }
}
