//! Building work priority — player-facing autonomous work competition (ADR-115).

use crate::world::task::TaskPriority;
use crate::world::{BuildingId, WorldData};

/// Player-facing discrete work-priority band stored as `BuildingOperationPolicy.priority` (u8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildingWorkPriorityLevel {
    Low,
    Normal,
    High,
}

/// Neutral default — maps to [`TaskPriority::Normal`] via [`building_work_priority_to_task_priority`].
pub const DEFAULT_BUILDING_WORK_PRIORITY_U8: u8 = 128;

pub const BUILDING_WORK_PRIORITY_LOW_U8: u8 = 40;
pub const BUILDING_WORK_PRIORITY_NORMAL_U8: u8 = 128;
pub const BUILDING_WORK_PRIORITY_HIGH_U8: u8 = 200;

/// Read authoritative building work priority (absent policy = neutral default).
pub fn building_work_priority_u8(world: &WorldData, building_id: BuildingId) -> u8 {
    world
        .building_production_store()
        .get_policy(building_id)
        .map(|policy| policy.priority)
        .unwrap_or(DEFAULT_BUILDING_WORK_PRIORITY_U8)
}

pub fn building_work_priority_level(
    world: &WorldData,
    building_id: BuildingId,
) -> BuildingWorkPriorityLevel {
    building_work_priority_level_from_u8(building_work_priority_u8(world, building_id))
}

pub fn building_work_priority_level_from_u8(priority: u8) -> BuildingWorkPriorityLevel {
    if priority >= BUILDING_WORK_PRIORITY_HIGH_U8 {
        BuildingWorkPriorityLevel::High
    } else if priority >= BUILDING_WORK_PRIORITY_NORMAL_U8 {
        BuildingWorkPriorityLevel::Normal
    } else {
        BuildingWorkPriorityLevel::Low
    }
}

pub fn building_work_priority_u8_for_level(level: BuildingWorkPriorityLevel) -> u8 {
    match level {
        BuildingWorkPriorityLevel::Low => BUILDING_WORK_PRIORITY_LOW_U8,
        BuildingWorkPriorityLevel::Normal => BUILDING_WORK_PRIORITY_NORMAL_U8,
        BuildingWorkPriorityLevel::High => BUILDING_WORK_PRIORITY_HIGH_U8,
    }
}

pub fn building_work_priority_label(level: BuildingWorkPriorityLevel) -> &'static str {
    match level {
        BuildingWorkPriorityLevel::Low => "Low",
        BuildingWorkPriorityLevel::Normal => "Normal",
        BuildingWorkPriorityLevel::High => "High",
    }
}

pub fn step_building_work_priority_level(
    level: BuildingWorkPriorityLevel,
    increase: bool,
) -> BuildingWorkPriorityLevel {
    match (level, increase) {
        (BuildingWorkPriorityLevel::Low, true) => BuildingWorkPriorityLevel::Normal,
        (BuildingWorkPriorityLevel::Normal, true) => BuildingWorkPriorityLevel::High,
        (BuildingWorkPriorityLevel::High, true) => BuildingWorkPriorityLevel::High,
        (BuildingWorkPriorityLevel::Low, false) => BuildingWorkPriorityLevel::Low,
        (BuildingWorkPriorityLevel::Normal, false) => BuildingWorkPriorityLevel::Low,
        (BuildingWorkPriorityLevel::High, false) => BuildingWorkPriorityLevel::Normal,
    }
}

/// Map stored building work priority into marketplace [`TaskPriority`].
pub fn building_work_priority_to_task_priority(policy_priority: u8) -> TaskPriority {
    if policy_priority >= BUILDING_WORK_PRIORITY_HIGH_U8 {
        TaskPriority::High
    } else if policy_priority >= 80 {
        TaskPriority::Normal
    } else {
        TaskPriority::Low
    }
}

pub fn building_work_priority_to_task_priority_for_building(
    world: &WorldData,
    building_id: BuildingId,
) -> TaskPriority {
    building_work_priority_to_task_priority(building_work_priority_u8(world, building_id))
}
