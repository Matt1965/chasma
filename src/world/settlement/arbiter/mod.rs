//! Settlement Response Arbiter (SA4 / ADR-119).
//!
//! Converts CandidateResponses into strategic SettlementIntent.
//! Never executes: no building/policy/task/worker/inventory mutations.

mod arbitrate;
mod intent;
mod step;
mod store;
mod validation;

#[cfg(test)]
mod tests;

pub use arbitrate::{
    arbitrate_settlement_intent, arbitration_score, ArbitrationContext, HIGH_PRESSURE_THRESHOLD,
    MAX_INTENTS_PER_NEED_HIGH, MAX_INTENTS_PER_NEED_NORMAL, MAX_SETTLEMENT_INTENTS,
    MIN_ARBITRATION_SCORE,
};
pub use intent::{
    IntentId, IntentPersistence, IntentRejectionReason, RejectedIntentCandidate, SettlementIntent,
    SettlementIntentPlan,
};
pub use step::{
    arbitrate_settlement_intent_now, step_settlement_response_arbitration,
    INTENT_ARBITRATION_CADENCE_TICKS,
};
pub use store::SettlementIntentStore;
pub use validation::{
    validate_intent, validate_settlement_intent_plan, IntentValidationError,
};
