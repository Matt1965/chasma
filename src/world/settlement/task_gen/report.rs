//! Transient strategic task generation report (SA6).

use bevy::prelude::*;

use crate::world::settlement::SettlementId;
use crate::world::task::{TaskId, TaskPriority, TaskType};
use crate::world::BuildingId;

#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct StrategicTaskEmission {
    pub task_id: TaskId,
    pub intent_id: String,
    pub response_id: String,
    pub template_id: String,
    pub task_type: TaskType,
    pub building_id: BuildingId,
    pub priority: TaskPriority,
    pub merged: bool,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct StrategicTaskGenerationReport {
    pub settlement_id: SettlementId,
    pub generated_tick: u64,
    pub source_intent_tick: u64,
    pub emissions: Vec<StrategicTaskEmission>,
    pub cancelled_task_ids: Vec<TaskId>,
    pub diagnostics: Vec<String>,
}

impl Default for StrategicTaskGenerationReport {
    fn default() -> Self {
        Self {
            settlement_id: SettlementId::new(0),
            generated_tick: 0,
            source_intent_tick: 0,
            emissions: Vec::new(),
            cancelled_task_ids: Vec::new(),
            diagnostics: Vec::new(),
        }
    }
}
