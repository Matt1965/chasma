//! SettlementState runtime — persistent SA foundation (SA1 / ADR-116).
//!
//! Storage and lifecycle bookkeeping only. No need evaluation, planning, or task generation.

mod dirty;
mod store;
mod types;
mod validation;

#[cfg(test)]
mod tests;

pub use dirty::{
    mark_all_settlement_states_dirty, mark_settlement_state_dirty,
    mark_settlement_state_dirty_for_building,
};
pub use store::SettlementStateStore;
pub use types::{
    ActiveEmergencyInstance, NeedCategory, NeedTarget, SettlementEmergencyState, SettlementKind,
    SettlementModifier, SettlementModifierSource, SettlementPlannerLifecycle, SettlementPolicies,
    SettlementState, SettlementStateSaveState, default_need_targets_for_kind,
};
pub use validation::{
    SettlementStateValidationError, ensure_settlement_states_for_world, validate_settlement_state,
    validate_settlement_states, validate_world_settlement_states,
};
