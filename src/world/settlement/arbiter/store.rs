//! Transient SettlementIntent store on WorldData (SA4). Never persisted.

use std::collections::HashMap;

use bevy::prelude::*;

use super::intent::SettlementIntentPlan;
use crate::world::settlement::SettlementId;

/// Runtime-only cache of latest intent plans. Discarded on load; rebuilt by arbitration.
#[derive(Debug, Clone, Default, Reflect)]
pub struct SettlementIntentStore {
    plans: HashMap<SettlementId, SettlementIntentPlan>,
    dirty: HashMap<SettlementId, bool>,
}

impl SettlementIntentStore {
    pub fn clear(&mut self) {
        self.plans.clear();
        self.dirty.clear();
    }

    pub fn get(&self, settlement_id: SettlementId) -> Option<&SettlementIntentPlan> {
        self.plans.get(&settlement_id)
    }

    pub fn insert(&mut self, plan: SettlementIntentPlan) {
        let id = plan.settlement_id;
        self.plans.insert(id, plan);
        self.dirty.insert(id, false);
    }

    pub fn remove(&mut self, settlement_id: SettlementId) {
        self.plans.remove(&settlement_id);
        self.dirty.remove(&settlement_id);
    }

    pub fn mark_dirty(&mut self, settlement_id: SettlementId) {
        self.dirty.insert(settlement_id, true);
    }

    pub fn mark_all_dirty(&mut self) {
        for id in self.plans.keys().copied().collect::<Vec<_>>() {
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
        self.plans.len()
    }

    pub fn is_empty(&self) -> bool {
        self.plans.is_empty()
    }
}
