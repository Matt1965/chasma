//! Settlement Response Arbiter (SA4 / ADR-119).
//!
//! Converts CandidateResponses into strategic SettlementIntent.
//! Never executes: no building/policy/task/worker/inventory mutations.

mod arbitrate;
mod intent;
mod scoring;
mod step;
mod store;
mod validation;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod phase6_tests;

pub use arbitrate::{
    ArbitrationContext, HIGH_PRESSURE_THRESHOLD, MAX_INTENTS_PER_NEED_HIGH,
    MAX_INTENTS_PER_NEED_NORMAL, MAX_SETTLEMENT_INTENTS, MIN_ARBITRATION_SCORE,
    arbitrate_settlement_intent, arbitration_score,
};
pub use intent::{
    IntentId, IntentPersistence, IntentRejectionReason, RejectedIntentCandidate, SettlementIntent,
    SettlementIntentPlan,
};
pub use scoring::{
    ArbitrationScoreBreakdown, MAX_WORKLOAD_PENALTY, WORKLOAD_PENALTY_FACTOR,
    authored_weight_for_need, compute_arbitration_score, compute_urgency, policy_component,
    workload_penalty,
};
pub use step::{
    INTENT_ARBITRATION_CADENCE_TICKS, arbitrate_settlement_intent_now,
    step_settlement_response_arbitration,
};
pub use store::SettlementIntentStore;
pub use validation::{IntentValidationError, validate_intent, validate_settlement_intent_plan};
