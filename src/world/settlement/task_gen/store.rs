//! Transient strategic task generation store (SA6). Never persisted.

use std::collections::HashMap;

use bevy::prelude::*;

use super::report::StrategicTaskGenerationReport;
use crate::world::settlement::SettlementId;

#[derive(Debug, Clone, Default, Reflect)]
pub struct StrategicTaskGenerationStore {
    reports: HashMap<SettlementId, StrategicTaskGenerationReport>,
    dirty: HashMap<SettlementId, bool>,
}

impl StrategicTaskGenerationStore {
    pub fn clear(&mut self) {
        self.reports.clear();
        self.dirty.clear();
    }

    pub fn get(&self, settlement_id: SettlementId) -> Option<&StrategicTaskGenerationReport> {
        self.reports.get(&settlement_id)
    }

    pub fn insert(&mut self, report: StrategicTaskGenerationReport) {
        let id = report.settlement_id;
        self.reports.insert(id, report);
        self.dirty.insert(id, false);
    }

    pub fn mark_dirty(&mut self, settlement_id: SettlementId) {
        self.dirty.insert(settlement_id, true);
    }

    pub fn mark_all_dirty(&mut self) {
        for id in self.reports.keys().copied().collect::<Vec<_>>() {
            self.dirty.insert(id, true);
        }
    }

    pub fn is_dirty(&self, settlement_id: SettlementId) -> bool {
        match self.dirty.get(&settlement_id) {
            Some(flag) => *flag,
            None => true,
        }
    }
}
