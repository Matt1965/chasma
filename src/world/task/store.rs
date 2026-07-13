use std::collections::{BTreeMap, HashMap};

use bevy::prelude::Reflect;

use super::error::TaskError;
use super::id::TaskId;
use super::record::TaskRecord;
use super::types::{TaskReservation, TaskState};
use crate::world::{BuildingId, UnitId};

/// Runtime task index on [`WorldData`] (ADR-085 B8).
#[derive(Debug, Clone, Default, PartialEq, Reflect)]
pub struct TaskStore {
    next_task_id: u32,
    tasks: BTreeMap<TaskId, TaskRecord>,
    building_tasks: HashMap<BuildingId, Vec<TaskId>>,
    unit_task: HashMap<UnitId, TaskId>,
    reservations: HashMap<(BuildingId, String), UnitId>,
}

impl TaskStore {
    pub fn allocate_task_id(&mut self) -> TaskId {
        let id = TaskId::new(self.next_task_id);
        self.next_task_id += 1;
        id
    }

    pub fn get(&self, id: TaskId) -> Option<&TaskRecord> {
        self.tasks.get(&id)
    }

    pub fn get_mut(&mut self, id: TaskId) -> Option<&mut TaskRecord> {
        self.tasks.get_mut(&id)
    }

    pub fn building_task_ids(&self, building_id: BuildingId) -> &[TaskId] {
        self.building_tasks
            .get(&building_id)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub fn unit_task_id(&self, unit_id: UnitId) -> Option<TaskId> {
        self.unit_task.get(&unit_id).copied()
    }

    pub fn reservation_for_point(
        &self,
        building_id: BuildingId,
        point_key: &str,
    ) -> Option<UnitId> {
        self.reservations
            .get(&(building_id, point_key.to_string()))
            .copied()
    }

    pub fn insert_task(&mut self, record: TaskRecord) -> Result<(), TaskError> {
        if self.tasks.contains_key(&record.id) {
            return Err(TaskError::TaskInvalidated(record.id));
        }
        let building_id = record.target_building_id();
        self.building_tasks
            .entry(building_id)
            .or_default()
            .push(record.id);
        self.tasks.insert(record.id, record);
        Ok(())
    }

    pub fn remove_task(&mut self, id: TaskId) -> Option<TaskRecord> {
        let record = self.tasks.remove(&id)?;
        if let Some(unit_id) = record.assigned_unit_id {
            self.unit_task.remove(&unit_id);
            if let Some(point_key) = record.reserved_point_key.as_deref() {
                self.release_reservation(record.target_building_id(), point_key, unit_id);
            }
        }
        if let Some(ids) = self.building_tasks.get_mut(&record.target_building_id()) {
            ids.retain(|task_id| *task_id != id);
        }
        Some(record)
    }

    pub fn reserve_point(
        &mut self,
        building_id: BuildingId,
        point_key: &str,
        unit_id: UnitId,
    ) -> Result<(), TaskError> {
        let key = (building_id, point_key.to_string());
        if let Some(existing) = self.reservations.get(&key) {
            if *existing != unit_id {
                return Err(TaskError::InteractionPointOccupied {
                    building_id,
                    point_key: point_key.to_string(),
                });
            }
            return Ok(());
        }
        self.reservations.insert(key, unit_id);
        Ok(())
    }

    pub fn release_reservation(
        &mut self,
        building_id: BuildingId,
        point_key: &str,
        unit_id: UnitId,
    ) {
        let key = (building_id, point_key.to_string());
        if self.reservations.get(&key) == Some(&unit_id) {
            self.reservations.remove(&key);
        }
    }

    pub fn assign_unit(&mut self, task_id: TaskId, unit_id: UnitId) -> Result<(), TaskError> {
        let task = self
            .tasks
            .get_mut(&task_id)
            .ok_or(TaskError::TaskNotFound(task_id))?;
        if let Some(existing_task) = self.unit_task.get(&unit_id) {
            if *existing_task != task_id {
                return Err(TaskError::TaskAlreadyAssigned(*existing_task));
            }
            return Ok(());
        }
        if task.assigned_unit_id.is_none() {
            task.assigned_unit_id = Some(unit_id);
        }
        task.state = TaskState::Assigned;
        self.unit_task.insert(unit_id, task_id);
        Ok(())
    }

    pub fn clear_unit_assignment(&mut self, unit_id: UnitId) {
        let Some(task_id) = self.unit_task.remove(&unit_id) else {
            return;
        };
        let release = self
            .tasks
            .get(&task_id)
            .map(|task| (task.target_building_id(), task.reserved_point_key.clone()));
        if let Some((building_id, Some(point_key))) = release {
            self.release_reservation(building_id, &point_key, unit_id);
        }
        if let Some(task) = self.tasks.get_mut(&task_id) {
            task.assigned_unit_id = None;
            task.reserved_point_key = None;
            if matches!(task.state, TaskState::Assigned | TaskState::InProgress) {
                task.state = TaskState::Available;
            }
        }
    }

    pub fn reservations(&self) -> impl Iterator<Item = TaskReservation> + '_ {
        self.reservations
            .iter()
            .map(|((building_id, point_key), unit_id)| TaskReservation {
                building_id: *building_id,
                point_key: point_key.clone(),
                unit_id: *unit_id,
            })
    }

    pub fn sorted_task_ids(&self) -> Vec<TaskId> {
        self.tasks.keys().copied().collect()
    }

    pub fn next_id(&self) -> u32 {
        self.next_task_id
    }

    pub fn restore_next_id(&mut self, next: u32) {
        self.next_task_id = self.next_task_id.max(next);
    }

    /// Clear all tasks and reservations (ADR-086 B9 scene load).
    pub fn clear(&mut self) {
        self.next_task_id = 1;
        self.tasks.clear();
        self.building_tasks.clear();
        self.unit_task.clear();
        self.reservations.clear();
    }

    /// Replace task state from a validated snapshot (ADR-086 B9).
    pub fn restore_snapshot(&mut self, records: Vec<TaskRecord>) -> Result<(), TaskError> {
        self.clear();
        for record in records {
            let building_id = record.target_building_id();
            if self.tasks.contains_key(&record.id) {
                return Err(TaskError::TaskInvalidated(record.id));
            }
            self.building_tasks
                .entry(building_id)
                .or_default()
                .push(record.id);
            if let Some(unit_id) = record.assigned_unit_id {
                if self.unit_task.insert(unit_id, record.id).is_some() {
                    return Err(TaskError::TaskAlreadyAssigned(record.id));
                }
            }
            if let Some(point_key) = record.reserved_point_key.as_deref() {
                if let Some(existing) = self.reservations.get(&(building_id, point_key.to_string()))
                {
                    if *existing != record.assigned_unit_id.unwrap_or(*existing) {
                        return Err(TaskError::ReservationConflict {
                            building_id,
                            point_key: point_key.to_string(),
                        });
                    }
                } else if let Some(unit_id) = record.assigned_unit_id {
                    self.reservations
                        .insert((building_id, point_key.to_string()), unit_id);
                }
            }
            let next = record.id.raw().saturating_add(1);
            self.next_task_id = self.next_task_id.max(next);
            self.tasks.insert(record.id, record);
        }
        Ok(())
    }
}
