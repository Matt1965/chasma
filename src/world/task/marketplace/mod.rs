//! Autonomous Worker Assignment — Task marketplace (SA7 / ADR-122).
//!
//! Workers evaluate Available Tasks and claim them via existing reservations.
//! Settlement AI never selects individual workers.

mod candidates;
mod report;
mod score;
mod step;
mod store;
mod sync;
mod validation;

#[cfg(test)]
mod tests;

pub use candidates::{MarketplaceCandidate, MarketplaceListing};
pub use report::{
    AssignmentDecision, WorkerAssignmentReport, WorkerEvaluation,
};
pub use score::{
    may_preempt, may_preempt_with_override, score_marketplace_listing, AssignmentScore,
    PreemptPolicyOverride, MIN_PREEMPT_PRIORITY_RANKS, MIN_STICK_TICKS,
};
pub use step::{
    step_worker_assignment, WorkerAssignmentContext, WORKER_ASSIGNMENT_CADENCE_TICKS,
};
pub use store::WorkerAssignmentStore;
pub use sync::sync_operate_workstation_tasks;
pub use validation::{validate_worker_assignments, AssignmentValidationError};
