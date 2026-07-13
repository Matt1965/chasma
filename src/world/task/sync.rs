use super::assignment::ensure_building_task;
use super::eligibility::building_is_constructible;
use super::types::{TaskPriority, TaskType};
use crate::world::{BuildingCatalog, BuildingId, WorldData};

/// Ensure construction tasks exist for incomplete buildings (ADR-085 B8).
pub fn sync_construction_tasks(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    simulation_tick: u64,
) {
    let building_ids = world.sorted_building_ids();
    for building_id in building_ids {
        let Some(record) = world.get_building(building_id) else {
            continue;
        };
        if !building_is_constructible(record) {
            continue;
        }
        let _ = ensure_building_task(
            world,
            building_id,
            TaskType::ConstructBuilding,
            TaskPriority::Normal,
            simulation_tick,
        );
        let _ = building_catalog;
    }
}

/// Remove tasks for buildings that no longer need them.
pub fn prune_invalid_building_tasks(world: &mut WorldData) {
    let task_ids = world.task_store().sorted_task_ids();
    for task_id in task_ids {
        let Some(task) = world.task_store().get(task_id).cloned() else {
            continue;
        };
        let building_id = match task.target {
            super::types::TaskTarget::Building(id) => id,
            super::types::TaskTarget::InteractionPoint { building_id, .. } => building_id,
        };
        let should_remove =
            world
                .get_building(building_id)
                .is_none_or(|record| match task.task_type {
                    TaskType::ConstructBuilding => !building_is_constructible(record),
                    TaskType::OperateWorkstation => {
                        !super::eligibility::building_accepts_workstation_use(record)
                    }
                });
        if should_remove {
            if let Some(unit_id) = task.assigned_unit_id {
                world.task_store_mut().clear_unit_assignment(unit_id);
            }
            world.task_store_mut().remove_task(task_id);
        }
    }
}
