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

pub use catalog::NeedCatalog;
pub use definition::{
    NeedDefinition, NeedEvaluationMethod, NeedMeasurementType, NeedResponseCategory,
    NeedTargetSource,
};
pub use evaluate::{evaluate_settlement_needs, resolve_desired, NeedEvalContext};
pub use id::NeedId;
pub use pressure::{apply_pressure_modifiers, normalize_pressure};
pub use snapshot::{
    NeedBlockingReason, NeedSnapshot, NeedTrend, SettlementNeedEvaluation,
};
pub use starter::starter_need_definitions;
pub use step::{
    evaluate_settlement_needs_now, step_settlement_need_evaluation, NEED_EVAL_CADENCE_TICKS,
};
pub use store::NeedEvaluationStore;
pub use validation::{
    validate_need_catalog, validate_need_snapshot, validate_settlement_need_evaluation,
    NeedCatalogError, NeedEvaluationValidationError,
};
