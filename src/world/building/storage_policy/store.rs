//! Authoritative storage policy storage on [`WorldData`].

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::policy::BuildingStoragePolicy;
use crate::world::BuildingId;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildingStoragePolicySaveState {
    pub policies: HashMap<u64, BuildingStoragePolicy>,
}

#[derive(Debug, Clone, Default, Reflect)]
pub struct BuildingStoragePolicyStore {
    policies: HashMap<BuildingId, BuildingStoragePolicy>,
    /// Storage buildings needing misfiled-item logistics reevaluation.
    #[reflect(ignore)]
    logistics_dirty: HashSet<BuildingId>,
}

impl BuildingStoragePolicyStore {
    pub fn export_save_state(&self) -> BuildingStoragePolicySaveState {
        BuildingStoragePolicySaveState {
            policies: self
                .policies
                .iter()
                .map(|(id, policy)| (id.raw(), policy.clone()))
                .collect(),
        }
    }

    pub fn import_save_state(&mut self, state: BuildingStoragePolicySaveState) {
        self.policies.clear();
        for (raw, policy) in state.policies {
            self.policies.insert(BuildingId::new(raw), policy);
        }
    }

    pub fn clear(&mut self) {
        self.policies.clear();
        self.logistics_dirty.clear();
    }

    pub fn mark_logistics_dirty(&mut self, building_id: BuildingId) {
        self.logistics_dirty.insert(building_id);
    }

    pub fn drain_logistics_dirty(&mut self) -> Vec<BuildingId> {
        let dirty: Vec<_> = self.logistics_dirty.iter().copied().collect();
        self.logistics_dirty.clear();
        dirty
    }

    pub fn policy(&self, building_id: BuildingId) -> Option<&BuildingStoragePolicy> {
        self.policies.get(&building_id)
    }

    pub fn policy_mut(&mut self, building_id: BuildingId) -> &mut BuildingStoragePolicy {
        self.policies
            .entry(building_id)
            .or_insert_with(BuildingStoragePolicy::default)
    }

    pub fn remove_building(&mut self, building_id: BuildingId) {
        self.policies.remove(&building_id);
    }
}
