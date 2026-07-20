//! Transient need evaluation store on WorldData (SA2). Never persisted.

use std::collections::HashMap;

use bevy::prelude::*;

use super::snapshot::SettlementNeedEvaluation;
use crate::world::settlement::SettlementId;

/// Runtime-only cache of latest need evaluations. Discarded on load; rebuilt by evaluation.
///
/// Dirty flags live here (not on SettlementState) so need evaluation never clears EP9 planner dirty.
#[derive(Debug, Clone, Default, Reflect)]
pub struct NeedEvaluationStore {
    evaluations: HashMap<SettlementId, SettlementNeedEvaluation>,
    /// Settlements that need re-evaluation. Missing key is treated as dirty.
    dirty: HashMap<SettlementId, bool>,
}

impl NeedEvaluationStore {
    pub fn clear(&mut self) {
        self.evaluations.clear();
        self.dirty.clear();
    }

    pub fn get(&self, settlement_id: SettlementId) -> Option<&SettlementNeedEvaluation> {
        self.evaluations.get(&settlement_id)
    }

    pub fn insert(&mut self, evaluation: SettlementNeedEvaluation) {
        let id = evaluation.settlement_id;
        self.evaluations.insert(id, evaluation);
        self.dirty.insert(id, false);
    }

    pub fn remove(&mut self, settlement_id: SettlementId) {
        self.evaluations.remove(&settlement_id);
        self.dirty.remove(&settlement_id);
    }

    pub fn mark_dirty(&mut self, settlement_id: SettlementId) {
        self.dirty.insert(settlement_id, true);
    }

    pub fn mark_all_dirty(&mut self) {
        for id in self.evaluations.keys().copied().collect::<Vec<_>>() {
            self.dirty.insert(id, true);
        }
    }

    /// True when never evaluated or explicitly dirtied.
    pub fn is_dirty(&self, settlement_id: SettlementId) -> bool {
        match self.dirty.get(&settlement_id) {
            Some(flag) => *flag,
            None => true,
        }
    }

    pub fn len(&self) -> usize {
        self.evaluations.len()
    }

    pub fn is_empty(&self) -> bool {
        self.evaluations.is_empty()
    }
}
