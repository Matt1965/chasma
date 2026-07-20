//! Worker assignment validation (SA7).

use std::collections::HashMap;

use crate::world::task::{TaskState, TaskType};
use crate::world::{UnitId, UnitState, WorldData};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssignmentValidationError {
    DoubleAssignment {
        unit_id: u64,
        task_a: u32,
        task_b: u32,
    },
    BrokenReservation {
        building_id: u64,
        point_key: String,
        detail: String,
    },
    InvalidCapability {
        unit_id: u64,
        task_id: u32,
        task_type: String,
    },
    DeadWorkerHoldingTask {
        unit_id: u64,
        task_id: u32,
    },
}

impl std::fmt::Display for AssignmentValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DoubleAssignment {
                unit_id,
                task_a,
                task_b,
            } => write!(
                f,
                "unit #{unit_id} double-assigned task #{task_a} and #{task_b}"
            ),
            Self::BrokenReservation {
                building_id,
                point_key,
                detail,
            } => write!(
                f,
                "broken reservation building#{building_id} point={point_key}: {detail}"
            ),
            Self::InvalidCapability {
                unit_id,
                task_id,
                task_type,
            } => write!(
                f,
                "unit #{unit_id} lacks capability for task #{task_id} ({task_type})"
            ),
            Self::DeadWorkerHoldingTask { unit_id, task_id } => {
                write!(f, "dead unit #{unit_id} still holds task #{task_id}")
            }
        }
    }
}

pub fn validate_worker_assignments(
    world: &WorldData,
    unit_catalog: &crate::world::UnitCatalog,
) -> Vec<AssignmentValidationError> {
    let mut errors = Vec::new();
    let mut unit_seen: HashMap<UnitId, crate::world::task::TaskId> = HashMap::new();

    for task_id in world.task_store().sorted_task_ids() {
        let Some(task) = world.task_store().get(task_id) else {
            continue;
        };
        if let Some(unit_id) = task.assigned_unit_id {
            if let Some(prev) = unit_seen.insert(unit_id, task_id) {
                if prev != task_id {
                    errors.push(AssignmentValidationError::DoubleAssignment {
                        unit_id: unit_id.raw(),
                        task_a: prev.raw(),
                        task_b: task_id.raw(),
                    });
                }
            }
            if let Some(unit) = world.get_unit(unit_id) {
                if matches!(unit.state, UnitState::Dead) {
                    errors.push(AssignmentValidationError::DeadWorkerHoldingTask {
                        unit_id: unit_id.raw(),
                        task_id: task_id.raw(),
                    });
                }
                if matches!(
                    task.state,
                    TaskState::Assigned | TaskState::InProgress | TaskState::BlockedWaiting
                ) && !crate::world::task::unit_can_perform_task(
                    unit_catalog,
                    world,
                    unit_id,
                    task.task_type,
                ) && !task.task_type.is_strategic()
                {
                    // Strategic kinds are marketplace stubs; skip capability until execution exists.
                    errors.push(AssignmentValidationError::InvalidCapability {
                        unit_id: unit_id.raw(),
                        task_id: task_id.raw(),
                        task_type: task.task_type.label().to_string(),
                    });
                }
            }
            if let Some(point_key) = task.reserved_point_key.as_deref() {
                match world
                    .task_store()
                    .reservation_for_point(task.target_building_id(), point_key)
                {
                    None => errors.push(AssignmentValidationError::BrokenReservation {
                        building_id: task.target_building_id().raw(),
                        point_key: point_key.to_string(),
                        detail: "missing reservation entry".into(),
                    }),
                    Some(reserved_unit) if reserved_unit != unit_id => {
                        errors.push(AssignmentValidationError::BrokenReservation {
                            building_id: task.target_building_id().raw(),
                            point_key: point_key.to_string(),
                            detail: format!(
                                "reserved by unit #{} but task assigned to #{}",
                                reserved_unit.raw(),
                                unit_id.raw()
                            ),
                        });
                    }
                    _ => {}
                }
            }
        }
        // Haul tasks must have matching request assignment.
        if task.task_type == TaskType::Haul {
            if let Some(request_id) = task.hauling_request_id() {
                if world.hauling_request_store().get(request_id).is_none() {
                    errors.push(AssignmentValidationError::BrokenReservation {
                        building_id: task.target_building_id().raw(),
                        point_key: "haul".into(),
                        detail: format!("missing haul request #{}", request_id.raw()),
                    });
                }
            }
        }
    }

    // Index consistency: unit_task → task.assigned or multi-worker map.
    for unit_id in world.sorted_unit_ids() {
        let Some(task_id) = world.task_store().unit_task_id(unit_id) else {
            continue;
        };
        let Some(task) = world.task_store().get(task_id) else {
            errors.push(AssignmentValidationError::BrokenReservation {
                building_id: 0,
                point_key: "unit_task".into(),
                detail: format!("unit #{} maps to missing task #{}", unit_id.raw(), task_id.raw()),
            });
            continue;
        };
        if task.assigned_unit_id.is_none() {
            // Multi-worker construction may only store first assignee on the record.
            // unit_task index is authoritative for the worker.
        }
        if let Some(unit) = world.get_unit(unit_id) {
            if matches!(unit.state, UnitState::Dead) {
                errors.push(AssignmentValidationError::DeadWorkerHoldingTask {
                    unit_id: unit_id.raw(),
                    task_id: task_id.raw(),
                });
            }
        }
    }

    errors
}
