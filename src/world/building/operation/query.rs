//! Production runtime queries (EP1).

use crate::world::{BuildingId, TaskTarget, TaskType, UnitId, WorldData};

/// Units currently assigned to operate the given building workstation.
pub fn workstation_workers_for_building(world: &WorldData, building_id: BuildingId) -> Vec<UnitId> {
    let mut workers = Vec::new();
    for unit_id in world.sorted_unit_ids() {
        let Some(task_id) = world.task_store().unit_task_id(unit_id) else {
            continue;
        };
        let Some(task) = world.task_store().get(task_id) else {
            continue;
        };
        if task.task_type != TaskType::OperateWorkstation {
            continue;
        }
        let target = match &task.target {
            TaskTarget::Building(id) => *id,
            TaskTarget::InteractionPoint { building_id, .. } => *building_id,
            TaskTarget::HaulRequest { .. } => continue,
        };
        if target == building_id {
            workers.push(unit_id);
        }
    }
    workers
}
