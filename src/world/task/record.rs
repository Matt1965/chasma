use bevy::prelude::*;

use super::id::TaskId;
use super::types::{TaskPriority, TaskState, TaskTarget, TaskType};
use crate::world::{BuildingId, UnitId};

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
        }
    }

    pub fn target_building_id(&self) -> BuildingId {
        match &self.target {
            TaskTarget::Building(id) => *id,
            TaskTarget::InteractionPoint { building_id, .. } => *building_id,
        }
    }
}
