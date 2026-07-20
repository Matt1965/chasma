use bevy::prelude::*;

use super::id::TaskId;
use super::types::{TaskPriority, TaskState, TaskTarget, TaskType};
use crate::world::{BuildingId, UnitId};

/// Origin tag when Settlement AI emitted this task (SA6). Persists with the task.
#[derive(Debug, Clone, PartialEq, Eq, Reflect)]
pub struct StrategicTaskOrigin {
    pub settlement_id: u64,
    pub intent_id: String,
    pub response_id: String,
    pub template_id: String,
}

/// One authoritative work task (ADR-085 B8).
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct TaskRecord {
    pub id: TaskId,
    pub task_type: TaskType,
    pub target: TaskTarget,
    pub state: TaskState,
    pub priority: TaskPriority,
    pub assigned_unit_id: Option<UnitId>,
    pub reserved_point_key: Option<String>,
    pub created_tick: u64,
    /// Present when emitted/owned by Settlement strategic task generation (SA6).
    pub strategic: Option<StrategicTaskOrigin>,
}

impl TaskRecord {
    pub fn new(
        id: TaskId,
        task_type: TaskType,
        target: TaskTarget,
        priority: TaskPriority,
        created_tick: u64,
    ) -> Self {
        Self {
            id,
            task_type,
            target,
            state: TaskState::Available,
            priority,
            assigned_unit_id: None,
            reserved_point_key: None,
            created_tick,
            strategic: None,
        }
    }

    pub fn with_strategic(mut self, origin: StrategicTaskOrigin) -> Self {
        self.strategic = Some(origin);
        self
    }

    pub fn target_building_id(&self) -> BuildingId {
        match &self.target {
            TaskTarget::Building(id) => *id,
            TaskTarget::InteractionPoint { building_id, .. } => *building_id,
            TaskTarget::HaulRequest {
                owning_building_id, ..
            } => *owning_building_id,
        }
    }

    pub fn hauling_request_id(&self) -> Option<crate::world::HaulingRequestId> {
        match &self.target {
            TaskTarget::HaulRequest { request_id, .. } => Some(*request_id),
            _ => None,
        }
    }
}
