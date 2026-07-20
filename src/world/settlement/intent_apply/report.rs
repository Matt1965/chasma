//! Transient Building Intent Propagation report (SA5). Never persisted.

use bevy::prelude::*;

use crate::world::operation::OperationDefinitionId;
use crate::world::settlement::arbiter::IntentId;
use crate::world::settlement::needs::NeedId;
use crate::world::settlement::response::ResponseId;
use crate::world::settlement::SettlementId;
use crate::world::BuildingId;

/// One building selected to carry out a SettlementIntent via policy.
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct BuildingPolicyAssignment {
    pub building_id: BuildingId,
    pub intent_id: IntentId,
    pub response_id: ResponseId,
    pub need_id: NeedId,
    pub selected_operation: Option<OperationDefinitionId>,
    pub enabled: bool,
    pub priority: u8,
    pub reason: String,
}

/// Building considered but not assigned.
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct IgnoredBuilding {
    pub building_id: BuildingId,
    pub response_id: ResponseId,
    pub reason: String,
}

/// Per-settlement propagation result (diagnostics + assignments).
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct BuildingIntentPropagationReport {
    pub settlement_id: SettlementId,
    pub propagated_tick: u64,
    pub source_intent_tick: u64,
    pub assignments: Vec<BuildingPolicyAssignment>,
    pub ignored_buildings: Vec<IgnoredBuilding>,
    pub deferred_intents: Vec<String>,
    pub diagnostics: Vec<String>,
}

impl Default for BuildingIntentPropagationReport {
    fn default() -> Self {
        Self {
            settlement_id: SettlementId::new(0),
            propagated_tick: 0,
            source_intent_tick: 0,
            assignments: Vec::new(),
            ignored_buildings: Vec::new(),
            deferred_intents: Vec::new(),
            diagnostics: Vec::new(),
        }
    }
}

impl BuildingIntentPropagationReport {
    pub fn assigned_building_ids(&self) -> impl Iterator<Item = BuildingId> + '_ {
        self.assignments.iter().map(|a| a.building_id)
    }

    pub fn assignment_for(&self, building_id: BuildingId) -> Option<&BuildingPolicyAssignment> {
        self.assignments
            .iter()
            .find(|a| a.building_id == building_id)
    }
}
