use bevy::prelude::*;

use super::id::CorpseId;
use super::record::CorpseRecord;
use crate::world::unit::UnitId;

/// Chunk-local corpse records (ADR-089 I3).
#[derive(Debug, Clone, Default, PartialEq, Reflect)]
pub struct ChunkCorpseStore {
    records: Vec<CorpseRecord>,
}

impl ChunkCorpseStore {
    pub fn records(&self) -> &[CorpseRecord] {
        &self.records
    }

    pub fn get(&self, id: CorpseId) -> Option<&CorpseRecord> {
        self.records
            .binary_search_by_key(&id, |record| record.id)
            .ok()
            .map(|index| &self.records[index])
    }

    pub fn get_mut(&mut self, id: CorpseId) -> Option<&mut CorpseRecord> {
        self.records
            .binary_search_by_key(&id, |record| record.id)
            .ok()
            .map(|index| &mut self.records[index])
    }

    pub fn insert(&mut self, record: CorpseRecord) {
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

    pub fn take(&mut self, id: CorpseId) -> Option<CorpseRecord> {
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

/// Corpse spatial index on [`crate::world::WorldData`] (ADR-089 I3).
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct CorpseStore {
    next_corpse_id: u64,
    corpses: std::collections::HashMap<crate::world::ChunkId, ChunkCorpseStore>,
    corpse_locations: std::collections::HashMap<CorpseId, crate::world::ChunkId>,
    origin_unit_index: std::collections::HashMap<UnitId, CorpseId>,
}

impl Default for CorpseStore {
    fn default() -> Self {
        Self {
            next_corpse_id: 1,
            corpses: std::collections::HashMap::new(),
            corpse_locations: std::collections::HashMap::new(),
            origin_unit_index: std::collections::HashMap::new(),
        }
    }
}

impl CorpseStore {
    pub fn allocate_corpse_id(&mut self) -> CorpseId {
        let id = CorpseId::new(self.next_corpse_id);
        self.next_corpse_id = self.next_corpse_id.saturating_add(1);
        id
    }

    pub fn next_id(&self) -> u64 {
        self.next_corpse_id
    }

    pub fn restore_next_id(&mut self, next: u64) {
        self.next_corpse_id = self.next_corpse_id.max(next);
    }

    pub fn sorted_corpse_ids(&self) -> Vec<CorpseId> {
        let mut ids: Vec<_> = self.corpse_locations.keys().copied().collect();
        ids.sort();
        ids
    }

    pub fn corpse_chunk(&self, id: CorpseId) -> Option<crate::world::ChunkId> {
        self.corpse_locations.get(&id).copied()
    }

    pub fn corpse_by_origin_unit(&self, unit_id: UnitId) -> Option<CorpseId> {
        self.origin_unit_index.get(&unit_id).copied()
    }

    pub fn get(&self, id: CorpseId) -> Option<&CorpseRecord> {
        let chunk = self.corpse_locations.get(&id)?;
        self.corpses.get(chunk)?.get(id)
    }

    pub fn get_mut(&mut self, id: CorpseId) -> Option<&mut CorpseRecord> {
        let chunk = *self.corpse_locations.get(&id)?;
        self.corpses.get_mut(&chunk)?.get_mut(id)
    }

    pub fn insert(
        &mut self,
        chunk: crate::world::ChunkId,
        record: CorpseRecord,
    ) -> Result<(), super::error::CorpseError> {
        if record.placement.position.chunk != chunk.coord() {
            return Err(super::error::CorpseError::ChunkPlacementMismatch {
                corpse_id: record.id,
            });
        }
        if self.corpse_locations.contains_key(&record.id) {
            return Err(super::error::CorpseError::CorpseIdCollision(record.id));
        }
        if self.origin_unit_index.contains_key(&record.origin_unit_id) {
            return Err(super::error::CorpseError::DuplicateOriginUnit(
                record.origin_unit_id,
            ));
        }
        self.corpses
            .entry(chunk)
            .or_default()
            .insert(record.clone());
        self.corpse_locations.insert(record.id, chunk);
        self.origin_unit_index
            .insert(record.origin_unit_id, record.id);
        Ok(())
    }

    pub fn remove(&mut self, id: CorpseId) -> Option<CorpseRecord> {
        let chunk = self.corpse_locations.remove(&id)?;
        let record = self.corpses.get_mut(&chunk)?.take(id)?;
        self.origin_unit_index.remove(&record.origin_unit_id);
        if self
            .corpses
            .get(&chunk)
            .is_some_and(|store| store.is_empty())
        {
            self.corpses.remove(&chunk);
        }
        Some(record)
    }

    pub fn clear(&mut self) {
        self.next_corpse_id = 1;
        self.corpses.clear();
        self.corpse_locations.clear();
        self.origin_unit_index.clear();
    }

    /// Replace all corpse records (scene restore — ADR-094 I8).
    pub fn restore_snapshot(
        &mut self,
        records: Vec<(crate::world::ChunkId, CorpseRecord)>,
        next_id: u64,
    ) -> Result<(), super::error::CorpseError> {
        self.clear();
        self.restore_next_id(next_id);
        for (chunk, record) in records {
            self.insert(chunk, record)?;
        }
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.corpse_locations.len()
    }
}
