//! Settlement anchor storage on [`crate::world::WorldData`] (ADR-133).

use std::collections::BTreeMap;

use bevy::prelude::*;

use super::super::id::SettlementId;
use super::error::SettlementCreationError;
use super::id::SettlementAnchorId;
use super::record::SettlementAnchorRecord;

#[derive(Debug, Clone, Default, PartialEq, Reflect)]
pub struct SettlementAnchorStore {
    next_anchor_id: u64,
    anchors: BTreeMap<SettlementAnchorId, SettlementAnchorRecord>,
    anchor_by_settlement: BTreeMap<SettlementId, SettlementAnchorId>,
}

impl SettlementAnchorStore {
    pub fn allocate_anchor_id(&mut self) -> SettlementAnchorId {
        let id = SettlementAnchorId::new(self.next_anchor_id);
        self.next_anchor_id = self.next_anchor_id.saturating_add(1);
        id
    }

    pub fn next_anchor_id(&self) -> u64 {
        self.next_anchor_id
    }

    pub fn restore_next_id(&mut self, next: u64) {
        self.next_anchor_id = self.next_anchor_id.max(next);
    }

    pub fn get(&self, id: SettlementAnchorId) -> Option<&SettlementAnchorRecord> {
        self.anchors.get(&id)
    }

    pub fn get_mut(&mut self, id: SettlementAnchorId) -> Option<&mut SettlementAnchorRecord> {
        self.anchors.get_mut(&id)
    }

    pub fn anchor_for_settlement(
        &self,
        settlement_id: SettlementId,
    ) -> Option<&SettlementAnchorRecord> {
        let anchor_id = self.anchor_by_settlement.get(&settlement_id)?;
        self.anchors.get(anchor_id)
    }

    pub fn settlement_for_anchor(&self, anchor_id: SettlementAnchorId) -> Option<SettlementId> {
        self.anchors
            .get(&anchor_id)
            .map(|record| record.settlement_id)
    }

    pub fn sorted_anchor_ids(&self) -> Vec<SettlementAnchorId> {
        self.anchors.keys().copied().collect()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn insert(
        &mut self,
        record: SettlementAnchorRecord,
    ) -> Result<(), SettlementCreationError> {
        if self.anchors.contains_key(&record.id) {
            return Err(SettlementCreationError::DuplicateAnchorId(record.id));
        }
        if self
            .anchor_by_settlement
            .contains_key(&record.settlement_id)
        {
            return Err(SettlementCreationError::DuplicateSettlementId(
                record.settlement_id,
            ));
        }
        self.anchor_by_settlement
            .insert(record.settlement_id, record.id);
        self.anchors.insert(record.id, record);
        Ok(())
    }

    pub fn remove(&mut self, id: SettlementAnchorId) -> Option<SettlementAnchorRecord> {
        let record = self.anchors.remove(&id)?;
        self.anchor_by_settlement.remove(&record.settlement_id);
        Some(record)
    }

    pub fn restore_snapshot(
        &mut self,
        records: Vec<SettlementAnchorRecord>,
        next_id: u64,
    ) -> Result<(), SettlementCreationError> {
        self.clear();
        self.restore_next_id(next_id);
        for record in records {
            self.insert(record)?;
        }
        Ok(())
    }
}
