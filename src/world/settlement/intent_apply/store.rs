//! Transient Building Intent Propagation store (SA5). Never persisted.

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;

use super::report::BuildingIntentPropagationReport;
use crate::world::settlement::SettlementId;
use crate::world::BuildingId;

#[derive(Debug, Clone, Default, Reflect)]
pub struct BuildingIntentPropagationStore {
    reports: HashMap<SettlementId, BuildingIntentPropagationReport>,
    dirty: HashMap<SettlementId, bool>,
    /// Cross-settlement index of buildings currently owned by SA5 propagation.
    assigned_buildings: HashSet<BuildingId>,
}

impl BuildingIntentPropagationStore {
    pub fn clear(&mut self) {
        self.reports.clear();
        self.dirty.clear();
        self.assigned_buildings.clear();
    }

    pub fn get(&self, settlement_id: SettlementId) -> Option<&BuildingIntentPropagationReport> {
        self.reports.get(&settlement_id)
    }

    pub fn insert(&mut self, report: BuildingIntentPropagationReport) {
        let id = report.settlement_id;
        if let Some(prev) = self.reports.get(&id) {
            for building_id in prev.assigned_building_ids() {
                self.assigned_buildings.remove(&building_id);
            }
        }
        for building_id in report.assigned_building_ids() {
            self.assigned_buildings.insert(building_id);
        }
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

    /// True when SA5 currently owns this building's policy assignment.
    pub fn is_building_assigned(&self, building_id: BuildingId) -> bool {
        self.assigned_buildings.contains(&building_id)
    }

    pub fn len(&self) -> usize {
        self.reports.len()
    }

    pub fn is_empty(&self) -> bool {
        self.reports.is_empty()
    }
}
