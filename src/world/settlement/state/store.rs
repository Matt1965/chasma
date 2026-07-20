//! SettlementState storage on WorldData (SA1).

use std::collections::BTreeMap;

use bevy::prelude::*;

use super::types::{SettlementKind, SettlementState, SettlementStateSaveState};
use crate::world::settlement::SettlementId;

#[derive(Debug, Clone, Default, Reflect)]
pub struct SettlementStateStore {
    states: BTreeMap<SettlementId, SettlementState>,
}

impl SettlementStateStore {
    pub fn clear(&mut self) {
        self.states.clear();
    }

    pub fn get(&self, settlement_id: SettlementId) -> Option<&SettlementState> {
        self.states.get(&settlement_id)
    }

    pub fn get_mut(&mut self, settlement_id: SettlementId) -> Option<&mut SettlementState> {
        self.states.get_mut(&settlement_id)
    }

    pub fn insert(&mut self, state: SettlementState) {
        self.states.insert(state.settlement_id, state);
    }

    /// Ensure a state exists for `settlement_id`. Returns mutable reference.
    pub fn ensure(
        &mut self,
        settlement_id: SettlementId,
        kind: SettlementKind,
        player_controlled: bool,
    ) -> &mut SettlementState {
        self.states.entry(settlement_id).or_insert_with(|| {
            SettlementState::new(settlement_id, kind, player_controlled)
        })
    }

    pub fn remove(&mut self, settlement_id: SettlementId) -> Option<SettlementState> {
        self.states.remove(&settlement_id)
    }

    pub fn settlement_ids(&self) -> impl Iterator<Item = SettlementId> + '_ {
        self.states.keys().copied()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&SettlementId, &SettlementState)> {
        self.states.iter()
    }

    pub fn len(&self) -> usize {
        self.states.len()
    }

    pub fn is_empty(&self) -> bool {
        self.states.is_empty()
    }

    pub fn mark_dirty(&mut self, settlement_id: SettlementId) {
        if let Some(state) = self.states.get_mut(&settlement_id) {
            state.mark_dirty();
        }
    }

    pub fn mark_all_dirty(&mut self) {
        for state in self.states.values_mut() {
            state.mark_dirty();
        }
    }

    /// After load: force dirty so all future planners rebuild from SettlementState.
    pub fn apply_rebuild_principle(&mut self) {
        self.mark_all_dirty();
    }

    pub fn export_save_state(&self) -> SettlementStateSaveState {
        SettlementStateSaveState {
            states: self
                .states
                .iter()
                .map(|(id, state)| (id.raw(), state.clone()))
                .collect(),
        }
    }

    pub fn import_save_state(&mut self, save: SettlementStateSaveState) {
        self.states = save
            .states
            .into_iter()
            .map(|(raw, mut state)| {
                let id = SettlementId::new(raw);
                state.settlement_id = id;
                // Rebuild principle: never trust runtime dirty / derived continuity from disk.
                state.planner.dirty = true;
                (id, state)
            })
            .collect();
    }
}
