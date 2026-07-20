//! Authoritative task system (ADR-085 B8) + Task marketplace assignment (SA7 / ADR-122).

mod assignment;
mod eligibility;
mod error;
mod events;
mod id;
mod labor;
mod marketplace;
mod record;
mod store;
mod sync;
mod types;

#[cfg(test)]
mod tests;

pub use assignment::{
    assign_construct_building_task, assign_operate_workstation_task, cancel_unit_task,
    claim_building_task, ensure_building_task, release_unit_task_to_marketplace,
};
pub use eligibility::{
    building_accepts_workstation_use, building_is_constructible, unit_can_perform_task,
    unit_may_work_on_building, unit_work_capabilities,
};
pub use error::TaskError;
pub use events::{TaskEvent, TaskTickReport};
pub use id::TaskId;
pub use labor::step_all_worker_tasks;
pub use marketplace::{
    may_preempt, may_preempt_with_override, score_marketplace_listing, step_worker_assignment,
    sync_operate_workstation_tasks, validate_worker_assignments, AssignmentDecision,
    AssignmentScore, AssignmentValidationError, MarketplaceCandidate, MarketplaceListing,
    PreemptPolicyOverride, WorkerAssignmentContext, WorkerAssignmentReport, WorkerAssignmentStore,
    WorkerEvaluation, MIN_PREEMPT_PRIORITY_RANKS, MIN_STICK_TICKS, WORKER_ASSIGNMENT_CADENCE_TICKS,
};
pub use record::{StrategicTaskOrigin, TaskRecord};
pub use store::TaskStore;
pub use sync::{prune_invalid_building_tasks, sync_construction_tasks};
pub use types::{
    BuildingInteractionPointId, TaskCancelReason, TaskPriority, TaskReservation, TaskState,
    TaskTarget, TaskType, UnitTaskAssignment,
};
