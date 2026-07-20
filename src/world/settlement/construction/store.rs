//! Construction plan store (authoritative) and transient planning reports (SA9).

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;

use crate::world::settlement::SettlementId;

use super::plan::{ConstructionPlan, ConstructionPlanId, ConstructionPlanSaveState};
use super::report::ConstructionPlanningReport;

#[derive(Debug, Clone, Default, Reflect)]
pub struct ConstructionPlanStore {
    plans: HashMap<ConstructionPlanId, ConstructionPlan>,
    by_settlement: HashMap<SettlementId, HashSet<ConstructionPlanId>>,
    by_fulfillment: HashMap<String, ConstructionPlanId>,
    next_plan_id: u64,
}

impl ConstructionPlanStore {
    pub fn export_save_state(&self) -> ConstructionPlanSaveState {
        let mut plans: Vec<_> = self.plans.values().cloned().collect();
        plans.sort_by_key(|p| p.id.raw());
        ConstructionPlanSaveState {
            plans,
            next_plan_id: self.next_plan_id.max(1),
        }
    }

    pub fn import_save_state(&mut self, state: ConstructionPlanSaveState) {
        self.clear();
        self.next_plan_id = state.next_plan_id.max(1);
        for plan in state.plans {
            self.insert(plan);
        }
    }

    pub fn clear(&mut self) {
        self.plans.clear();
        self.by_settlement.clear();
        self.by_fulfillment.clear();
        self.next_plan_id = 1;
    }

    pub fn allocate_id(&mut self) -> ConstructionPlanId {
        if self.next_plan_id == 0 {
            self.next_plan_id = 1;
        }
        let id = ConstructionPlanId::new(self.next_plan_id);
        self.next_plan_id = self.next_plan_id.saturating_add(1);
        id
    }

    pub fn insert(&mut self, plan: ConstructionPlan) {
        let id = plan.id;
        let settlement_id = plan.settlement_id;
        if let Some(prev) = self.plans.get(&id) {
            if prev.fulfillment_key != plan.fulfillment_key {
                self.by_fulfillment.remove(&prev.fulfillment_key);
            }
            if let Some(set) = self.by_settlement.get_mut(&prev.settlement_id) {
                set.remove(&id);
            }
        }
        if plan.status.is_active() {
            self.by_fulfillment
                .insert(plan.fulfillment_key.clone(), id);
        } else {
            self.by_fulfillment.remove(&plan.fulfillment_key);
        }
        self.by_settlement
            .entry(settlement_id)
            .or_default()
            .insert(id);
        self.plans.insert(id, plan);
    }

    pub fn get(&self, id: ConstructionPlanId) -> Option<&ConstructionPlan> {
        self.plans.get(&id)
    }

    pub fn get_mut(&mut self, id: ConstructionPlanId) -> Option<&mut ConstructionPlan> {
        self.plans.get_mut(&id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &ConstructionPlan> {
        self.plans.values()
    }

    pub fn plans_for_settlement(&self, settlement_id: SettlementId) -> Vec<&ConstructionPlan> {
        self.by_settlement
            .get(&settlement_id)
            .into_iter()
            .flat_map(|ids| ids.iter())
            .filter_map(|id| self.plans.get(id))
            .collect()
    }

    pub fn active_for_fulfillment(&self, key: &str) -> Option<&ConstructionPlan> {
        self.by_fulfillment
            .get(key)
            .and_then(|id| self.plans.get(id))
            .filter(|p| p.status.is_active())
    }

    pub fn active_count(&self, settlement_id: SettlementId) -> usize {
        self.plans_for_settlement(settlement_id)
            .into_iter()
            .filter(|p| p.status.is_active())
            .count()
    }
}

/// Transient construction planning diagnostics (never persisted).
#[derive(Debug, Clone, Default, Reflect)]
pub struct ConstructionPlanningReportStore {
    reports: HashMap<SettlementId, ConstructionPlanningReport>,
    dirty: HashSet<SettlementId>,
}

impl ConstructionPlanningReportStore {
    pub fn insert(&mut self, report: ConstructionPlanningReport) {
        let id = report.settlement_id;
        self.reports.insert(id, report);
        self.dirty.remove(&id);
    }

    pub fn get(&self, settlement_id: SettlementId) -> Option<&ConstructionPlanningReport> {
        self.reports.get(&settlement_id)
    }

    pub fn mark_dirty(&mut self, settlement_id: SettlementId) {
        self.dirty.insert(settlement_id);
    }

    pub fn mark_all_dirty(&mut self) {
        for id in self.reports.keys().copied().collect::<Vec<_>>() {
            self.dirty.insert(id);
        }
    }

    pub fn is_dirty(&self, settlement_id: SettlementId) -> bool {
        self.dirty.contains(&settlement_id) || !self.reports.contains_key(&settlement_id)
    }

    pub fn clear(&mut self) {
        self.reports.clear();
        self.dirty.clear();
    }
}
