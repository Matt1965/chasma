//! Authoritative building production runtime storage on WorldData (EP2).

use std::collections::HashMap;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::lifecycle::OperationLifecycle;
use super::operation_id::OperationDefinitionId;
use super::policy::BuildingOperationPolicy;
use super::progress::ProductionProgress;
use crate::world::BuildingId;
use crate::world::building::catalog::BuildingDefinition;
use crate::world::building::operational_efficiency::OperationalLimitingFactor;

/// Simulation truth for one building's active production operation (EP2).
#[derive(Debug, Clone, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub struct BuildingOperationState {
    pub lifecycle: OperationLifecycle,
    pub progress: ProductionProgress,
    #[serde(skip)]
    pub blocked_reason: Option<OperationalLimitingFactor>,
    pub completion_count: u32,
    pub last_efficiency_revision: u64,
    /// Workers currently assigned to operate this building (EP2).
    pub active_worker_count: u32,
}

impl Default for BuildingOperationState {
    fn default() -> Self {
        Self {
            lifecycle: OperationLifecycle::Idle,
            progress: ProductionProgress::ZERO,
            blocked_reason: None,
            completion_count: 0,
            last_efficiency_revision: 0,
            active_worker_count: 0,
        }
    }
}

/// Serializable production runtime for save/load (EP2).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildingProductionSaveState {
    pub states: HashMap<u64, BuildingOperationState>,
    pub policies: HashMap<u64, BuildingOperationPolicy>,
}

/// Authoritative per-building production state and policy (EP2).
#[derive(Debug, Clone, Default, Reflect)]
pub struct BuildingProductionStore {
    states: HashMap<BuildingId, BuildingOperationState>,
    policies: HashMap<BuildingId, BuildingOperationPolicy>,
}

impl BuildingProductionStore {
    pub fn export_save_state(&self) -> BuildingProductionSaveState {
        BuildingProductionSaveState {
            states: self
                .states
                .iter()
                .map(|(id, state)| (id.raw(), state.clone()))
                .collect(),
            policies: self
                .policies
                .iter()
                .map(|(id, policy)| (id.raw(), policy.clone()))
                .collect(),
        }
    }

    pub fn import_save_state(&mut self, state: BuildingProductionSaveState) {
        self.states = state
            .states
            .into_iter()
            .map(|(raw, state)| (BuildingId::new(raw), state))
            .collect();
        self.policies = state
            .policies
            .into_iter()
            .map(|(raw, policy)| (BuildingId::new(raw), policy))
            .collect();
    }

    pub fn clear(&mut self) {
        self.states.clear();
        self.policies.clear();
    }

    pub fn remove(&mut self, building_id: BuildingId) {
        self.states.remove(&building_id);
        self.policies.remove(&building_id);
    }

    pub fn get(&self, building_id: BuildingId) -> Option<&BuildingOperationState> {
        self.get_state(building_id)
    }

    pub fn get_or_default_mut(&mut self, building_id: BuildingId) -> &mut BuildingOperationState {
        self.get_state_mut(building_id)
    }

    pub fn reset_progress(&mut self, building_id: BuildingId) {
        if let Some(state) = self.states.get_mut(&building_id) {
            state.progress = ProductionProgress::ZERO;
            state.completion_count = 0;
            state.lifecycle = OperationLifecycle::Idle;
            state.blocked_reason = None;
            state.active_worker_count = 0;
        }
    }

    pub fn get_state(&self, building_id: BuildingId) -> Option<&BuildingOperationState> {
        self.states.get(&building_id)
    }

    pub fn get_state_mut(&mut self, building_id: BuildingId) -> &mut BuildingOperationState {
        self.states.entry(building_id).or_default()
    }

    pub fn get_policy(&self, building_id: BuildingId) -> Option<&BuildingOperationPolicy> {
        self.policies.get(&building_id)
    }

    pub fn get_policy_mut(&mut self, building_id: BuildingId) -> &mut BuildingOperationPolicy {
        self.policies.entry(building_id).or_default()
    }

    /// Replace policy and reset runtime progress (used when changing operation configuration).
    pub fn set_policy(&mut self, building_id: BuildingId, policy: BuildingOperationPolicy) {
        if let Some(state) = self.states.get_mut(&building_id) {
            state.completion_count = 0;
            state.progress = ProductionProgress::ZERO;
            state.lifecycle = OperationLifecycle::Idle;
            state.blocked_reason = None;
            state.active_worker_count = 0;
        }
        self.policies.insert(building_id, policy);
    }

    pub fn ensure_policy_for_building(
        &mut self,
        building_id: BuildingId,
        definition: &BuildingDefinition,
        operation_catalog: &crate::world::operation::OperationCatalog,
    ) -> &BuildingOperationPolicy {
        self.policies
            .entry(building_id)
            .or_insert_with(|| {
                BuildingOperationPolicy::default_for_building(definition, operation_catalog)
            })
    }

    pub fn building_ids(&self) -> impl Iterator<Item = BuildingId> + '_ {
        self.states
            .keys()
            .chain(self.policies.keys())
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
    }

    pub fn len(&self) -> usize {
        self.building_ids().count()
    }

    pub fn selected_operation(
        &self,
        building_id: BuildingId,
    ) -> Option<&OperationDefinitionId> {
        self.policies
            .get(&building_id)
            .and_then(|policy| policy.selected_operation.as_ref())
    }
}

/// Backward-compatible alias for state-only access during migration (EP1).
pub type BuildingOperationStore = BuildingProductionStore;

/// Backward-compatible save alias (EP1).
pub type BuildingOperationSaveState = BuildingProductionSaveState;
