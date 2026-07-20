//! Building Intent Propagation (SA5 / ADR-120).
//!
//! Converts SettlementIntent into BuildingOperationPolicy changes.
//! Buildings execute intent; workers remain unaware of settlement strategy.

mod discover;
mod propagate;
mod report;
mod step;
mod store;
mod validation;

#[cfg(test)]
mod tests;

pub use discover::{discover_capable_buildings, primary_operation_requirement, CapableBuilding};
pub use propagate::{
    building_owned_by_intent_propagation, propagate_settlement_intent_to_buildings,
    PropagationContext, HIGH_INTENT_PRIORITY, MAX_BUILDINGS_PER_INTENT_HIGH,
    MAX_BUILDINGS_PER_INTENT_NORMAL,
};
pub use report::{
    BuildingIntentPropagationReport, BuildingPolicyAssignment, IgnoredBuilding,
};
pub use step::{
    propagate_building_intent_now, step_building_intent_propagation,
    INTENT_PROPAGATION_CADENCE_TICKS,
};
pub use store::BuildingIntentPropagationStore;
pub use validation::{validate_propagation_report, PropagationValidationError};
