use bevy::prelude::*;

use super::assignment::cancel_unit_task;
use super::eligibility::unit_work_capabilities;
use super::events::{TaskEvent, TaskTickReport};
use super::types::{TaskCancelReason, TaskState, TaskType};
use crate::world::{
    BuildingCatalog, BuildingDefinition, DoodadCatalog, InteriorProfileCatalog, OccupancyCatalogs,
    UnitCatalog, UnitId, UnitState, WorldData, add_building_construction_progress,
};
use crate::world::{
    BuildingInteractionProfileCatalog, INTERACTION_WORK_RANGE_METERS,
    interaction_point_world_position,
};

/// Apply worker labor on fixed simulation ticks (ADR-085 B8).
pub fn step_all_worker_tasks(
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
    building_catalog: &BuildingCatalog,
    interaction_catalog: &BuildingInteractionProfileCatalog,
    interior_catalog: &InteriorProfileCatalog,
    doodad_catalog: &DoodadCatalog,
    occupancy: OccupancyCatalogs<'_>,
    delta_seconds: f32,
) -> TaskTickReport {
    let mut report = TaskTickReport::default();
    if delta_seconds <= 0.0 {
        return report;
    }

    let unit_ids = world.sorted_unit_ids();
    for unit_id in unit_ids {
        let Some(task_id) = world.task_store().unit_task_id(unit_id) else {
            continue;
        };
        let Some(task) = world.task_store().get(task_id).cloned() else {
            continue;
        };
        let building_id = task.target_building_id();
        let Some(building) = world.get_building(building_id).cloned() else {
            cancel_unit_task(
                world,
                unit_id,
                TaskCancelReason::BuildingDestroyed,
                &mut report.events,
            );
            report.tasks_canceled += 1;
            continue;
        };
        let Some(definition) = building_catalog.get(&building.definition_id) else {
            cancel_unit_task(
                world,
                unit_id,
                TaskCancelReason::Invalidated,
                &mut report.events,
            );
            report.tasks_canceled += 1;
            continue;
        };
        let Some(profile) = interaction_catalog.profile_for_definition(definition) else {
            cancel_unit_task(
                world,
                unit_id,
                TaskCancelReason::Invalidated,
                &mut report.events,
            );
            report.tasks_canceled += 1;
            continue;
        };
        let point_key = task
            .reserved_point_key
            .as_deref()
            .or_else(|| profile.points.first().map(|point| point.key));
        let Some(point_key) = point_key else {
            cancel_unit_task(
                world,
                unit_id,
                TaskCancelReason::InteractionPointRemoved,
                &mut report.events,
            );
            report.tasks_canceled += 1;
            continue;
        };
        let point = profile.points.iter().find(|point| point.key == point_key);
        let Some(point) = point else {
            cancel_unit_task(
                world,
                unit_id,
                TaskCancelReason::InteractionPointRemoved,
                &mut report.events,
            );
            report.tasks_canceled += 1;
            continue;
        };

        let layout = world.layout();
        let work_pos = interaction_point_world_position(&building, layout, point);
        let unit = match world.get_unit(unit_id) {
            Some(record) => record.clone(),
            None => {
                cancel_unit_task(
                    world,
                    unit_id,
                    TaskCancelReason::WorkerDied,
                    &mut report.events,
                );
                report.tasks_canceled += 1;
                continue;
            }
        };
        if matches!(unit.state, UnitState::Dead) {
            cancel_unit_task(
                world,
                unit_id,
                TaskCancelReason::WorkerDied,
                &mut report.events,
            );
            report.tasks_canceled += 1;
            continue;
        }

        let unit_global = unit.placement.position.to_global(layout);
        let work_global = work_pos.to_global(layout);
        let dx = unit_global.x - work_global.x;
        let dz = unit_global.z - work_global.z;
        let distance = (dx * dx + dz * dz).sqrt();

        if distance > INTERACTION_WORK_RANGE_METERS {
            if matches!(unit.state, UnitState::Working { .. }) {
                let _ = world.set_unit_state(
                    unit_id,
                    UnitState::Moving {
                        target: work_pos,
                        path: unit.state.path_if_moving().cloned().unwrap_or_default(),
                        waypoint_index: 0,
                    },
                );
            }
            continue;
        }

        if !matches!(unit.state, UnitState::Working { .. }) {
            let _ = world.set_unit_state(unit_id, UnitState::Working { task_id });
            report
                .events
                .push(TaskEvent::WorkerArrived { task_id, unit_id });
            if task.task_type == TaskType::OperateWorkstation {
                report.events.push(TaskEvent::WorkstationOperationStarted {
                    task_id,
                    building_id,
                    unit_id,
                });
            }
        }
        if let Some(task) = world.task_store_mut().get_mut(task_id) {
            task.state = TaskState::InProgress;
        }

        match task.task_type {
            TaskType::ConstructBuilding => {
                let Some(caps) = unit_work_capabilities(unit_catalog, world, unit_id) else {
                    continue;
                };
                let required_labor = definition.build_time_seconds.max(0.01);
                let progress_delta = caps.construction_speed * delta_seconds / required_labor;
                if progress_delta <= 0.0 {
                    continue;
                }
                if add_building_construction_progress(
                    world,
                    building_catalog,
                    interior_catalog,
                    doodad_catalog,
                    occupancy,
                    building_id,
                    progress_delta,
                )
                .is_ok()
                {
                    report.labor_applied += 1;
                    report.events.push(TaskEvent::BuildingLaborApplied {
                        building_id,
                        unit_id,
                        progress_delta,
                    });
                }
                if world.get_building(building_id).is_some_and(|record| {
                    record.lifecycle_state == crate::world::BuildingLifecycleState::Complete
                }) {
                    report
                        .events
                        .push(TaskEvent::BuildingConstructionCompleted { building_id });
                    cancel_unit_task(
                        world,
                        unit_id,
                        TaskCancelReason::BuildingCompleted,
                        &mut report.events,
                    );
                    if let Some(task) = world.task_store_mut().get_mut(task_id) {
                        task.state = TaskState::Completed;
                    }
                    report.tasks_completed += 1;
                }
            }
            TaskType::OperateWorkstation => {
                // Foundation only — no production output in B8.
            }
        }
    }
    report
}

trait TaskLaborExt {
    fn target_building_id(&self) -> crate::world::BuildingId;
}

impl TaskLaborExt for super::record::TaskRecord {
    fn target_building_id(&self) -> crate::world::BuildingId {
        match &self.target {
            super::types::TaskTarget::Building(id) => *id,
            super::types::TaskTarget::InteractionPoint { building_id, .. } => *building_id,
        }
    }
}

trait UnitStatePath {
    fn path_if_moving(&self) -> Option<&crate::world::NavigationPath>;
}

impl UnitStatePath for UnitState {
    fn path_if_moving(&self) -> Option<&crate::world::NavigationPath> {
        match self {
            UnitState::Moving { path, .. } => Some(path),
            _ => None,
        }
    }
}
