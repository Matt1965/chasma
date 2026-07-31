//! Emergency Pressure & Priority Reweighting (SA8 / ADR-123).
//!
//! Emergencies are priority inputs into the existing Need → Response → Intent → Task → Worker
//! pipeline. They never assign workers or command buildings directly.

mod catalog;
mod definition;
mod evaluate;
mod modifiers;
mod report;
mod starter;
mod step;
mod store;
mod validation;

#[cfg(test)]
mod tests;

pub use catalog::{EmergencyCatalog, EmergencyCatalogError};
pub use definition::{
    EmergencyDefinition, EmergencyEvaluatorKind, EmergencyId, EmergencyInterruptionPolicy,
    NeedPressureModifier, ResponseScoreModifier, TaskPriorityModifier,
};
pub use evaluate::{EmergencyEvalContext, evaluate_settlement_emergencies};
pub use modifiers::{
    EmergencyPreemptRelaxation, active_definitions, emergency_blocks_response,
    emergency_bump_task_priority, emergency_need_pressure_delta, emergency_only_gate,
    emergency_preempt_relaxation, emergency_response_score_delta, emergency_unlocks_response,
};
pub use report::{EmergencyEvaluationReport, EmergencySignalDiagnostic};
pub use starter::starter_emergency_definitions;
pub use step::{
    EMERGENCY_EVAL_CADENCE_TICKS, evaluate_settlement_emergencies_now,
    step_settlement_emergency_evaluation,
};
pub use store::EmergencyEvaluationStore;
pub use validation::{
    EmergencyValidationError, validate_emergency_catalog, validate_emergency_definition,
};
