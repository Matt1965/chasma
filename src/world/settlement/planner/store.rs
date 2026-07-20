//! Production planner storage on WorldData (EP9).

use std::collections::HashMap;

use bevy::prelude::*;

use super::types::{ProductionPlannerSaveState, SettlementProductionPlanner};
use crate::world::settlement::SettlementId;

#[derive(Debug, Clone, Default, Reflect)]
pub struct ProductionPlannerStore {
    planners: HashMap<SettlementId, SettlementProductionPlanner>,
}

impl ProductionPlannerStore {
    pub fn export_save_state(&self) -> ProductionPlannerSaveState {
        ProductionPlannerSaveState {
            planners: self
                .planners
                .iter()
                .map(|(id, planner)| (id.raw(), planner.clone()))
                .collect(),
        }
    }

    pub fn import_save_state(&mut self, state: ProductionPlannerSaveState) {
        self.planners = state
            .planners
            .into_iter()
            .map(|(raw, planner)| (SettlementId::new(raw), planner))
            .collect();
    }

    pub fn clear(&mut self) {
        self.planners.clear();
    }

    pub fn get(&self, settlement_id: SettlementId) -> Option<&SettlementProductionPlanner> {
        self.planners.get(&settlement_id)
    }

    pub fn get_mut(&mut self, settlement_id: SettlementId) -> &mut SettlementProductionPlanner {
        self.planners.entry(settlement_id).or_default()
    }

    pub fn ensure(&mut self, settlement_id: SettlementId) -> &mut SettlementProductionPlanner {
        self.get_mut(settlement_id)
    }

    pub fn remove(&mut self, settlement_id: SettlementId) {
        self.planners.remove(&settlement_id);
    }

    pub fn settlement_ids(&self) -> impl Iterator<Item = SettlementId> + '_ {
        self.planners.keys().copied()
    }

    pub fn mark_dirty(&mut self, settlement_id: SettlementId) {
        if let Some(planner) = self.planners.get_mut(&settlement_id) {
            planner.mark_dirty();
        }
    }

    pub fn mark_all_dirty(&mut self) {
        for planner in self.planners.values_mut() {
            planner.mark_dirty();
        }
    }
}
