//! OperateWorkstation marketplace listings — farms only when ready to harvest.

use crate::world::building::operation::{farm_needs_harvest_worker, is_prispod_farm_definition};
use crate::world::task::{
    TaskPriority, TaskState, TaskType, building_accepts_workstation_use, ensure_building_task,
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
        let Some(definition) = building_catalog.get(&record.definition_id) else {
            continue;
        };
        let store = world.building_production_store();
        let Some(policy) = store.get_policy(building_id) else {
            cancel_available_operate_tasks(world, building_id);
            continue;
        };
        let effective_operation = policy
            .selected_operation
            .clone()
            .or_else(|| definition.resolved_default_operation());
        if !policy.enabled || policy.paused || effective_operation.is_none() {
            cancel_available_operate_tasks(world, building_id);
            continue;
        }
        if is_prispod_farm_definition(definition)
            && !farm_needs_harvest_worker(store, building_id, definition)
        {
            cancel_available_operate_tasks(world, building_id);
            continue;
        }
        let priority = policy_priority_to_task_priority(policy.priority);
        let _ = ensure_building_task(
            world,
            building_id,
            TaskType::OperateWorkstation,
            priority,
            simulation_tick,
        );
        for task_id in world.task_store().building_task_ids(building_id).to_vec() {
            if let Some(task) = world.task_store_mut().get_mut(task_id) {
                if task.task_type == TaskType::OperateWorkstation
                    && task.state == TaskState::Available
                    && task.priority != TaskPriority::PlayerAssigned
                {
                    task.priority = priority;
                }
            }
        }
    }
}

fn cancel_available_operate_tasks(world: &mut WorldData, building_id: crate::world::BuildingId) {
    for task_id in world.task_store().building_task_ids(building_id).to_vec() {
        if let Some(task) = world.task_store_mut().get_mut(task_id) {
            if task.task_type == TaskType::OperateWorkstation && task.state == TaskState::Available
            {
                task.state = TaskState::Canceled;
            }
        }
    }
}
