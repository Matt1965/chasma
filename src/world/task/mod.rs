//! Authoritative task system (ADR-085 B8).

mod assignment;
mod eligibility;
mod error;
mod events;
mod id;
mod labor;
mod record;
mod store;
mod sync;
mod types;

#[cfg(test)]
mod tests;

pub use assignment::{
    assign_construct_building_task, assign_operate_workstation_task, cancel_unit_task,
    ensure_building_task,
};
pub use eligibility::{
    building_accepts_workstation_use, building_is_constructible, unit_can_perform_task,
    unit_may_work_on_building, unit_work_capabilities,
};
pub use error::TaskError;
pub use events::{TaskEvent, TaskTickReport};
pub use id::TaskId;
pub use labor::step_all_worker_tasks;
pub use record::TaskRecord;
pub use store::TaskStore;
pub use sync::{prune_invalid_building_tasks, sync_construction_tasks};
pub use types::{
    BuildingInteractionPointId, TaskCancelReason, TaskPriority, TaskReservation, TaskState,
    TaskTarget, TaskType, UnitTaskAssignment,
};
