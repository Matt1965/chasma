//! Transient response candidate store on WorldData (SA3). Never persisted.

use std::collections::HashMap;

use bevy::prelude::*;

use super::candidate::SettlementResponseCandidates;
use crate::world::settlement::SettlementId;

/// Runtime-only cache of latest response candidates. Discarded on load; rebuilt by discovery.
#[derive(Debug, Clone, Default, Reflect)]
pub struct ResponseCandidateStore {
    candidates: HashMap<SettlementId, SettlementResponseCandidates>,
    dirty: HashMap<SettlementId, bool>,
}

impl ResponseCandidateStore {
    pub fn clear(&mut self) {
        self.candidates.clear();
        self.dirty.clear();
    }

    pub fn get(&self, settlement_id: SettlementId) -> Option<&SettlementResponseCandidates> {
        self.candidates.get(&settlement_id)
    }

    pub fn insert(&mut self, result: SettlementResponseCandidates) {
        let id = result.settlement_id;
        self.candidates.insert(id, result);
        self.dirty.insert(id, false);
    }

    pub fn remove(&mut self, settlement_id: SettlementId) {
        self.candidates.remove(&settlement_id);
        self.dirty.remove(&settlement_id);
    }

    pub fn mark_dirty(&mut self, settlement_id: SettlementId) {
        self.dirty.insert(settlement_id, true);
    }

    pub fn mark_all_dirty(&mut self) {
        for id in self.candidates.keys().copied().collect::<Vec<_>>() {
            self.dirty.insert(id, true);
        }
    }

    pub fn is_dirty(&self, settlement_id: SettlementId) -> bool {
        match self.dirty.get(&settlement_id) {
            Some(flag) => *flag,
            None => true,
        }
    }

    pub fn len(&self) -> usize {
        self.candidates.len()
    }

    pub fn is_empty(&self) -> bool {
        self.candidates.is_empty()
    }
}
