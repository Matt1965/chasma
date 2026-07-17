use std::collections::HashMap;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::progress::ProductionProgress;
use crate::world::BuildingId;

/// Authoritative fractional operation progress for one Building (ADR-105 TF5).
#[derive(Debug, Clone, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub struct BuildingOperationState {
    pub progress: ProductionProgress,
    pub completion_count: u32,
    pub last_efficiency_revision: u64,
}

impl Default for BuildingOperationState {
    fn default() -> Self {
        Self {
            progress: ProductionProgress::ZERO,
            completion_count: 0,
            last_efficiency_revision: 0,
        }
    }
}

/// Serializable operation progress for save/load (ADR-106 TF6).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildingOperationSaveState {
    pub states: HashMap<u64, BuildingOperationState>,
}

/// Side store for generic Building operation progress (ADR-105 TF5).
#[derive(Debug, Clone, Default, Resource, Reflect)]
#[reflect(Resource)]
pub struct BuildingOperationStore {
    #[reflect(ignore)]
    states: HashMap<BuildingId, BuildingOperationState>,
}

impl BuildingOperationStore {
    pub fn export_save_state(&self) -> BuildingOperationSaveState {
        BuildingOperationSaveState {
            states: self
                .states
                .iter()
                .map(|(id, state)| (id.raw(), state.clone()))
                .collect(),
        }
    }

    pub fn import_save_state(&mut self, state: BuildingOperationSaveState) {
        self.states = state
            .states
            .into_iter()
            .map(|(raw, state)| (BuildingId::new(raw), state))
            .collect();
    }
    pub fn get(&self, building_id: BuildingId) -> Option<&BuildingOperationState> {
        self.states.get(&building_id)
    }

    pub fn get_or_default_mut(&mut self, building_id: BuildingId) -> &mut BuildingOperationState {
        self.states.entry(building_id).or_default()
    }

    pub fn remove(&mut self, building_id: BuildingId) {
        self.states.remove(&building_id);
    }

    pub fn reset(&mut self, building_id: BuildingId) {
        self.states
            .insert(building_id, BuildingOperationState::default());
    }

    pub fn len(&self) -> usize {
        self.states.len()
    }
}
