//! Transient worker assignment state (SA7). Never persisted.

use std::collections::HashMap;

use bevy::prelude::*;

use super::report::WorkerAssignmentReport;
use crate::world::UnitId;

#[derive(Debug, Clone, Default, Reflect, PartialEq)]
pub struct WorkerStickState {
    pub task_assigned_tick: u64,
    pub last_preempt_tick: Option<u64>,
}

/// Runtime marketplace / hysteresis state on [`crate::world::WorldData`].
#[derive(Debug, Clone, Default, Reflect)]
pub struct WorkerAssignmentStore {
    pub last_report: WorkerAssignmentReport,
    /// Per-worker stick/preempt hysteresis (rebuild-safe; not authoritative).
    stick: HashMap<UnitId, WorkerStickState>,
    dirty: bool,
}

impl WorkerAssignmentStore {
    pub fn clear(&mut self) {
        self.last_report = WorkerAssignmentReport::default();
        self.stick.clear();
        self.dirty = true;
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }

    pub fn set_report(&mut self, report: WorkerAssignmentReport) {
        self.last_report = report;
        self.dirty = false;
    }

    pub fn stick(&self, unit_id: UnitId) -> Option<&WorkerStickState> {
        self.stick.get(&unit_id)
    }

    pub fn note_assignment(&mut self, unit_id: UnitId, tick: u64, preempted: bool) {
        let entry = self.stick.entry(unit_id).or_default();
        entry.task_assigned_tick = tick;
        if preempted {
            entry.last_preempt_tick = Some(tick);
        }
    }

    pub fn clear_stick(&mut self, unit_id: UnitId) {
        self.stick.remove(&unit_id);
    }
}
