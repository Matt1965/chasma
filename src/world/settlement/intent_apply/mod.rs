//! Building Intent Propagation (SA5 / ADR-120).
//!
//! Converts SettlementIntent into BuildingOperationPolicy changes.
//! Buildings execute intent; workers remain unaware of settlement strategy.

mod discover;
mod policy;
mod propagate;
mod report;
mod step;
mod store;
mod validation;

#[cfg(test)]
mod phase8_tests;
mod tests;

pub use discover::{CapableBuilding, discover_capable_buildings, primary_operation_requirement};
pub use policy::{
    can_sa5_mutate_policy, sa5_apply_policy_decision, sa5_disable_unselected_ai_buildings,
};
pub use propagate::{
    HIGH_INTENT_PRIORITY, MAX_BUILDINGS_PER_INTENT_HIGH, MAX_BUILDINGS_PER_INTENT_NORMAL,
    PropagationContext, propagate_settlement_intent_to_buildings,
};
pub use report::{BuildingIntentPropagationReport, BuildingPolicyAssignment, IgnoredBuilding};
pub use step::{
    INTENT_PROPAGATION_CADENCE_TICKS, propagate_building_intent_now,
    step_building_intent_propagation,
};
pub use store::BuildingIntentPropagationStore;
pub use validation::{PropagationValidationError, validate_propagation_report};
