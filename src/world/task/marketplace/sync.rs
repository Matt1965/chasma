//! Ensure OperateWorkstation marketplace listings exist for enabled buildings (SA7).

use crate::world::task::{
    building_accepts_workstation_use, ensure_building_task, TaskPriority, TaskType,
};
use crate::world::{BuildingCatalog, WorldData};

/// Map building operation policy priority (0..=255) into TaskPriority.
pub fn policy_priority_to_task_priority(policy_priority: u8) -> TaskPriority {
    if policy_priority >= 200 {
        TaskPriority::High
    } else if policy_priority >= 80 {
        TaskPriority::Normal
    } else {
        TaskPriority::Low
    }
}

/// Create/refresh Available OperateWorkstation tasks for buildings that want labor.
///
/// Does not assign workers. Skips constructible / incomplete buildings.
pub fn sync_operate_workstation_tasks(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    simulation_tick: u64,
) {
    let building_ids = world.sorted_building_ids();
    for building_id in building_ids {
        let Some(record) = world.get_building(building_id) else {
            continue;
        };
        if !building_accepts_workstation_use(record) {
            continue;
        }
        let Some(policy) = world.building_production_store().get_policy(building_id) else {
            continue;
        };
        if !policy.enabled || policy.paused || policy.selected_operation.is_none() {
            continue;
        }
        // Interaction profile must advertise OperateWorkstation points.
        let Some(definition) = building_catalog.get(&record.definition_id) else {
            continue;
        };
        let _ = definition;
        let priority = policy_priority_to_task_priority(policy.priority);
        let _ = ensure_building_task(
            world,
            building_id,
            TaskType::OperateWorkstation,
            priority,
            simulation_tick,
        );
        // Refresh priority on existing Available operate tasks (not PlayerAssigned).
        for task_id in world.task_store().building_task_ids(building_id).to_vec() {
            if let Some(task) = world.task_store_mut().get_mut(task_id) {
                if task.task_type == TaskType::OperateWorkstation
                    && task.state == crate::world::task::TaskState::Available
                    && task.priority != TaskPriority::PlayerAssigned
                {
                    task.priority = priority;
                }
            }
        }
    }
}
