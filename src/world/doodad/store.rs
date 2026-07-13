use bevy::prelude::*;

use super::id::DoodadId;
use super::record::DoodadRecord;

/// Chunk-local collection of doodad records (ADR-015).
///
/// Records are kept sorted by [`DoodadId`] for deterministic iteration.
#[derive(Debug, Clone, Default, PartialEq, Reflect)]
pub struct ChunkDoodadStore {
    records: Vec<DoodadRecord>,
}

impl ChunkDoodadStore {
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

    pub fn records(&self) -> &[DoodadRecord] {
        &self.records
    }

    pub fn get(&self, id: DoodadId) -> Option<&DoodadRecord> {
        self.records
            .binary_search_by_key(&id, |record| record.id)
            .ok()
            .map(|index| &self.records[index])
    }

    pub fn get_mut(&mut self, id: DoodadId) -> Option<&mut DoodadRecord> {
        self.records
            .binary_search_by_key(&id, |record| record.id)
            .ok()
            .map(|index| &mut self.records[index])
    }

    /// Insert a record, replacing any existing entry with the same [`DoodadId`].
    pub fn insert(&mut self, record: DoodadRecord) {
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
    pub fn take(&mut self, id: DoodadId) -> Option<DoodadRecord> {
        if let Ok(index) = self.records.binary_search_by_key(&id, |entry| entry.id) {
            Some(self.records.remove(index))
        } else {
            None
        }
    }

    /// Remove a record by id. Returns `true` when a record was removed.
    pub fn remove(&mut self, id: DoodadId) -> bool {
        self.take(id).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        ChunkCoord, DoodadDefinitionId, DoodadKind, DoodadPlacement, DoodadSource, LocalPosition,
        WorldPosition,
    };

    fn sample_record(id: u64, kind: DoodadKind) -> DoodadRecord {
        DoodadRecord::new(
            DoodadId::new(id),
            DoodadDefinitionId::new("tree_oak"),
            kind,
            DoodadPlacement::new(
                WorldPosition::new(
                    ChunkCoord::new(0, 0),
                    LocalPosition::new(Vec3::new(10.0, 0.0, 20.0)),
                ),
                Quat::IDENTITY,
                Vec3::ONE,
            ),
            DoodadSource::Authored,
        )
    }

    #[test]
    fn insert_maintains_sorted_order_by_id() {
        let mut store = ChunkDoodadStore::new();
        store.insert(sample_record(3, DoodadKind::Rock));
        store.insert(sample_record(1, DoodadKind::Tree));
        store.insert(sample_record(2, DoodadKind::Bush));

        let ids: Vec<_> = store.records().iter().map(|r| r.id.raw()).collect();
        assert_eq!(ids, vec![1, 2, 3]);
    }

    #[test]
    fn remove_by_id() {
        let mut store = ChunkDoodadStore::new();
        store.insert(sample_record(1, DoodadKind::Tree));
        store.insert(sample_record(2, DoodadKind::Rock));

        assert!(store.remove(DoodadId::new(1)));
        assert_eq!(store.len(), 1);
        assert!(store.get(DoodadId::new(1)).is_none());
        assert!(!store.remove(DoodadId::new(99)));
    }
}
