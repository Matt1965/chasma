use bevy::prelude::*;

use super::id::TaskId;
use crate::world::{BuildingId, UnitId};

/// Active task kinds for B8 (ADR-085).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum TaskType {
    ConstructBuilding,
    OperateWorkstation,
}

impl TaskType {
    pub fn label(self) -> &'static str {
        match self {
            Self::ConstructBuilding => "ConstructBuilding",
            Self::OperateWorkstation => "OperateWorkstation",
        }
    }
}

/// Task lifecycle state (ADR-085 B8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, Default)]
pub enum TaskState {
    #[default]
    Available,
    Assigned,
    InProgress,
    /// Waiting for operational conditions (terrain, output, etc.) (ADR-105 TF5).
    BlockedWaiting,
    Completed,
    Canceled,
}

/// Deterministic priority ordering (lower = higher priority).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum TaskPriority {
    PlayerAssigned = 0,
    High = 1,
    Normal = 2,
    Low = 3,
}

impl Default for TaskPriority {
    fn default() -> Self {
        Self::Normal
    }
}

impl TaskPriority {
    pub fn rank(self) -> u8 {
        self as u8
    }
}

/// What a task acts upon (ADR-085 B8).
#[derive(Debug, Clone, PartialEq, Eq, Reflect)]
pub enum TaskTarget {
    Building(BuildingId),
    InteractionPoint {
        building_id: BuildingId,
        point_key: String,
    },
}

/// Reservation of one interaction point slot.
#[derive(Debug, Clone, PartialEq, Eq, Reflect)]
pub struct TaskReservation {
    pub building_id: BuildingId,
    pub point_key: String,
    pub unit_id: UnitId,
}

/// Why a task was canceled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum TaskCancelReason {
    PlayerOrder,
    BuildingCompleted,
    BuildingDestroyed,
    WorkerDied,
    InteractionPointRemoved,
    AccessRevoked,
    PathUnavailable,
    OutOfRange,
    Invalidated,
    DevCancel,
}

impl TaskCancelReason {
    pub fn label(self) -> &'static str {
        match self {
            Self::PlayerOrder => "PlayerOrder",
            Self::BuildingCompleted => "BuildingCompleted",
            Self::BuildingDestroyed => "BuildingDestroyed",
            Self::WorkerDied => "WorkerDied",
            Self::InteractionPointRemoved => "InteractionPointRemoved",
            Self::AccessRevoked => "AccessRevoked",
            Self::PathUnavailable => "PathUnavailable",
            Self::OutOfRange => "OutOfRange",
            Self::Invalidated => "Invalidated",
            Self::DevCancel => "DevCancel",
        }
    }
}

/// Interaction point identity (ADR-085 B8).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BuildingInteractionPointId {
    pub building_id: BuildingId,
    pub point_key: String,
}

impl BuildingInteractionPointId {
    pub fn new(building_id: BuildingId, point_key: impl Into<String>) -> Self {
        Self {
            building_id,
            point_key: point_key.into(),
        }
    }
}

/// Per-unit task assignment mirror for quick lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Default)]
pub struct UnitTaskAssignment {
    pub task_id: Option<TaskId>,
}
