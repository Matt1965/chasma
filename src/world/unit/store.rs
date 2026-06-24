use bevy::prelude::*;

use super::id::UnitId;
use super::record::UnitRecord;

/// Chunk-local collection of unit records (ADR-027 U2).
///
/// Records are kept sorted by [`UnitId`] for deterministic iteration.
#[derive(Debug, Clone, Default, PartialEq, Reflect)]
pub struct ChunkUnitStore {
    records: Vec<UnitRecord>,
}

impl ChunkUnitStore {
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn records(&self) -> &[UnitRecord] {
        &self.records
    }

    pub fn get(&self, id: UnitId) -> Option<&UnitRecord> {
        self.records
            .binary_search_by_key(&id, |record| record.id)
            .ok()
            .map(|index| &self.records[index])
    }

    pub fn get_mut(&mut self, id: UnitId) -> Option<&mut UnitRecord> {
        self.records
            .binary_search_by_key(&id, |record| record.id)
            .ok()
            .map(|index| &mut self.records[index])
    }

    /// Insert a record, replacing any existing entry with the same [`UnitId`].
    pub fn insert(&mut self, record: UnitRecord) {
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

    /// Remove a record by id, returning it when present.
    pub fn take(&mut self, id: UnitId) -> Option<UnitRecord> {
        if let Ok(index) = self.records.binary_search_by_key(&id, |entry| entry.id) {
            Some(self.records.remove(index))
        } else {
            None
        }
    }

    /// Remove a record by id. Returns `true` when a record was removed.
    pub fn remove(&mut self, id: UnitId) -> bool {
        self.take(id).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        ChunkCoord, LocalPosition, UnitDefinitionId, UnitPlacement, UnitSource, WorldPosition,
    };

    fn sample_record(id: u64) -> UnitRecord {
        UnitRecord::new(
            UnitId::new(id),
            UnitDefinitionId::new("wolf"),
            UnitPlacement::new(
                WorldPosition::new(
                    ChunkCoord::new(0, 0),
                    LocalPosition::new(Vec3::new(10.0, 0.0, 20.0)),
                ),
                Quat::IDENTITY,
            ),
            UnitSource::Authored,
            crate::world::UnitOwnership::neutral(),
        )
    }

    #[test]
    fn insert_maintains_sorted_order_by_id() {
        let mut store = ChunkUnitStore::new();
        store.insert(sample_record(3));
        store.insert(sample_record(1));
        store.insert(sample_record(2));

        let ids: Vec<_> = store.records().iter().map(|r| r.id.raw()).collect();
        assert_eq!(ids, vec![1, 2, 3]);
    }

    #[test]
    fn remove_by_id() {
        let mut store = ChunkUnitStore::new();
        store.insert(sample_record(1));
        store.insert(sample_record(2));

        assert!(store.remove(UnitId::new(1)));
        assert_eq!(store.len(), 1);
        assert!(store.get(UnitId::new(1)).is_none());
        assert!(!store.remove(UnitId::new(99)));
    }
}
