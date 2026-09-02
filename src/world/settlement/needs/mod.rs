//! Need Evaluation Runtime (SA2 / ADR-117).
//!
//! Computes settlement need pressures from SettlementState + world readouts.
//! Never generates tasks, mutates production, or persists snapshots.

mod catalog;
mod definition;
mod evaluate;
mod id;
mod pressure;
mod snapshot;
mod starter;
mod step;
mod store;
mod validation;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod phase5_tests;

pub use catalog::NeedCatalog;
pub use definition::{
    DEFAULT_FOOD_PLANNING_HORIZON_TICKS, NeedDefinition, NeedEvaluationMethod, NeedMeasurementType,
    NeedResponseCategory, NeedTargetSource,
};
pub use evaluate::{
    NeedEvalContext, evaluate_settlement_needs, resolve_desired, resolve_desired_from_state,
};
pub use id::NeedId;
pub use pressure::{apply_pressure_modifiers, normalize_pressure};
pub use snapshot::{NeedBlockingReason, NeedSnapshot, NeedTrend, SettlementNeedEvaluation};
pub use starter::starter_need_definitions;
pub use step::{
    NEED_EVAL_CADENCE_TICKS, evaluate_settlement_needs_now, step_settlement_need_evaluation,
};
pub use store::NeedEvaluationStore;
pub use validation::{
    NeedCatalogError, NeedEvaluationValidationError, validate_need_catalog, validate_need_snapshot,
    validate_settlement_need_evaluation,
};
