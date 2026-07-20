//! Transient construction planning report (SA9).

use bevy::prelude::*;

use crate::world::building::catalog::BuildingDefinitionId;
use crate::world::settlement::SettlementId;

use super::plan::ConstructionPlanId;

#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct BuildingCandidateScore {
    pub building_definition_id: BuildingDefinitionId,
    pub score: i32,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct RejectedSiteDiagnostic {
    pub offset_x: f32,
    pub offset_z: f32,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct ConstructionPlanningReport {
    pub settlement_id: SettlementId,
    pub planned_tick: u64,
    pub source_intent_tick: Option<u64>,
    pub created_plan_ids: Vec<ConstructionPlanId>,
    pub refreshed_plan_ids: Vec<ConstructionPlanId>,
    pub cancelled_plan_ids: Vec<ConstructionPlanId>,
    pub considered_buildings: Vec<BuildingCandidateScore>,
    pub capacity_notes: Vec<String>,
    pub rejected_sites: Vec<RejectedSiteDiagnostic>,
    pub diagnostics: Vec<String>,
}

impl ConstructionPlanningReport {
    pub fn new(settlement_id: SettlementId, planned_tick: u64) -> Self {
        Self {
            settlement_id,
            planned_tick,
            source_intent_tick: None,
            created_plan_ids: Vec::new(),
            refreshed_plan_ids: Vec::new(),
            cancelled_plan_ids: Vec::new(),
            considered_buildings: Vec::new(),
            capacity_notes: Vec::new(),
            rejected_sites: Vec::new(),
            diagnostics: Vec::new(),
        }
    }
}
