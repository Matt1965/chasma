use bevy::prelude::*;

use super::id::TaskId;
use super::types::{TaskCancelReason, TaskType};
use crate::world::{BuildingId, UnitId};

/// Bounded task lifecycle events (ADR-085 B8).
#[derive(Debug, Clone, PartialEq)]
pub enum TaskEvent {
    BuildingTaskCreated {
        task_id: TaskId,
        building_id: BuildingId,
        task_type: TaskType,
    },
    TaskAssigned {
        task_id: TaskId,
        unit_id: UnitId,
    },
    InteractionPointReserved {
        building_id: BuildingId,
        point_key: String,
        unit_id: UnitId,
    },
    WorkerArrived {
        task_id: TaskId,
        unit_id: UnitId,
    },
    BuildingLaborApplied {
        building_id: BuildingId,
        unit_id: UnitId,
        progress_delta: f32,
    },
    BuildingConstructionCompleted {
        building_id: BuildingId,
    },
    TaskCanceled {
        task_id: TaskId,
        reason: TaskCancelReason,
    },
    ReservationReleased {
        building_id: BuildingId,
        point_key: String,
        unit_id: UnitId,
    },
    WorkstationOperationStarted {
        task_id: TaskId,
        building_id: BuildingId,
        unit_id: UnitId,
    },
    WorkstationOperationStopped {
        task_id: TaskId,
        building_id: BuildingId,
        unit_id: UnitId,
        reason: TaskCancelReason,
    },
}

/// Aggregated task tick report.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TaskTickReport {
    pub labor_applied: u32,
    pub tasks_completed: u32,
    pub tasks_canceled: u32,
    pub events: Vec<TaskEvent>,
}
