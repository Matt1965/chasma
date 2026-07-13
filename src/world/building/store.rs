use bevy::prelude::*;

use super::id::BuildingId;
use super::record::BuildingRecord;

/// Chunk-local collection of building records (ADR-079 B2).
#[derive(Debug, Clone, Default, PartialEq, Reflect)]
pub struct ChunkBuildingStore {
    records: Vec<BuildingRecord>,
}

impl ChunkBuildingStore {
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

    pub fn records(&self) -> &[BuildingRecord] {
        &self.records
    }

    pub fn get(&self, id: BuildingId) -> Option<&BuildingRecord> {
        self.records
            .binary_search_by_key(&id, |record| record.id)
            .ok()
            .map(|index| &self.records[index])
    }

    pub fn get_mut(&mut self, id: BuildingId) -> Option<&mut BuildingRecord> {
        self.records
            .binary_search_by_key(&id, |entry| entry.id)
            .ok()
            .map(|index| &mut self.records[index])
    }

    pub fn insert(&mut self, record: BuildingRecord) {
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

    pub fn take(&mut self, id: BuildingId) -> Option<BuildingRecord> {
        if let Ok(index) = self.records.binary_search_by_key(&id, |entry| entry.id) {
            Some(self.records.remove(index))
        } else {
            None
        }
    }

    pub fn remove(&mut self, id: BuildingId) -> bool {
        self.take(id).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        BuildingDefinitionId, BuildingOwnership, BuildingPlacement, BuildingSource, ChunkCoord,
        LocalPosition, WorldPosition,
    };

    fn sample_record(id: u64) -> BuildingRecord {
        BuildingRecord::new(
            BuildingId::new(id),
            BuildingDefinitionId::new("hut"),
            BuildingPlacement::new(
                WorldPosition::new(
                    ChunkCoord::new(0, 0),
                    LocalPosition::new(Vec3::new(10.0, 0.0, 20.0)),
                ),
                Quat::IDENTITY,
            ),
            BuildingOwnership::neutral(),
            250,
            BuildingSource::Authored,
        )
    }

    #[test]
    fn insert_maintains_sorted_order_by_id() {
        let mut store = ChunkBuildingStore::new();
        store.insert(sample_record(3));
        store.insert(sample_record(1));
        store.insert(sample_record(2));

        let ids: Vec<_> = store.records().iter().map(|r| r.id.raw()).collect();
        assert_eq!(ids, vec![1, 2, 3]);
    }

    #[test]
    fn remove_by_id() {
        let mut store = ChunkBuildingStore::new();
        store.insert(sample_record(1));
        store.insert(sample_record(2));

        assert!(store.remove(BuildingId::new(1)));
        assert_eq!(store.len(), 1);
        assert!(store.get(BuildingId::new(1)).is_none());
        assert!(!store.remove(BuildingId::new(99)));
    }
}
