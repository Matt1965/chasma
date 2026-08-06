//! Retained selected-unit path diagnostics for dev overlays (IN-11eO).

use std::collections::HashMap;

use bevy::prelude::*;

use crate::world::{NavigationPath, SpaceId, UnitId, WorldPosition};

/// Lifecycle of a retained path trace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathTraceStatus {
    Active,
    Completed,
    Failed,
    Replaced,
    Cleared,
}

/// One retained path for overlay and status display.
#[derive(Debug, Clone, PartialEq)]
pub struct RetainedUnitPath {
    pub authority_sequence: u64,
    pub sequence: u64,
    pub unit_id: UnitId,
    pub start: WorldPosition,
    pub goal: WorldPosition,
    pub start_space: SpaceId,
    pub goal_space: SpaceId,
    pub path: NavigationPath,
    pub waypoint_index: usize,
    pub status: PathTraceStatus,
    pub failure_reason: Option<String>,
    pub blocked_position: Option<WorldPosition>,
    pub blocked_reason: Option<String>,
}

/// Bounded latest path traces per unit (diagnostic only).
#[derive(Resource, Debug, Clone, Default)]
pub struct UnitPathDiagnosticStore {
    traces: HashMap<UnitId, RetainedUnitPath>,
    next_sequence: u64,
    pub capacity: usize,
}

impl UnitPathDiagnosticStore {
    pub const DEFAULT_CAPACITY: usize = 4;

    pub fn record_committed(
        &mut self,
        unit_id: UnitId,
        start: WorldPosition,
        goal: WorldPosition,
        start_space: SpaceId,
        goal_space: SpaceId,
        path: NavigationPath,
        authority_sequence: u64,
    ) -> u64 {
        let sequence = self.next_sequence;
        self.next_sequence = self.next_sequence.wrapping_add(1);
        let entry = RetainedUnitPath {
            authority_sequence,
            sequence,
            unit_id,
            start,
            goal,
            start_space,
            goal_space,
            path,
            waypoint_index: 0,
            status: PathTraceStatus::Active,
            failure_reason: None,
            blocked_position: None,
            blocked_reason: None,
        };
        if self.traces.len() >= self.capacity {
            if let Some(oldest) = self
                .traces
                .values()
                .min_by_key(|trace| trace.sequence)
                .map(|trace| trace.unit_id)
            {
                self.traces.remove(&oldest);
            }
        }
        self.traces.insert(unit_id, entry);
        sequence
    }

    pub fn sync_live_waypoint(&mut self, unit_id: UnitId, waypoint_index: usize) {
        if let Some(trace) = self.traces.get_mut(&unit_id) {
            if trace.status == PathTraceStatus::Active {
                trace.waypoint_index = waypoint_index;
            }
        }
    }

    pub fn mark_completed(&mut self, unit_id: UnitId) {
        if let Some(trace) = self.traces.get_mut(&unit_id) {
            trace.status = PathTraceStatus::Completed;
        }
    }

    pub fn mark_failed(
        &mut self,
        unit_id: UnitId,
        reason: impl Into<String>,
        blocked_position: Option<WorldPosition>,
        blocked_reason: Option<String>,
    ) {
        if let Some(trace) = self.traces.get_mut(&unit_id) {
            trace.status = PathTraceStatus::Failed;
            trace.failure_reason = Some(reason.into());
            trace.blocked_position = blocked_position;
            trace.blocked_reason = blocked_reason;
        }
    }

    pub fn clear_unit(&mut self, unit_id: UnitId) {
        self.traces.remove(&unit_id);
    }

    pub fn clear_all(&mut self) {
        self.traces.clear();
    }

    pub fn latest_for_unit(&self, unit_id: UnitId) -> Option<&RetainedUnitPath> {
        self.traces.get(&unit_id)
    }
}

#[cfg(feature = "dev")]
pub fn sync_unit_path_diagnostic_store(
    world: Res<crate::world::WorldData>,
    mut store: ResMut<UnitPathDiagnosticStore>,
    selection: Res<crate::units::input::SelectedUnits>,
) {
    for unit_id in selection.iter() {
        let authority = world.movement_authority_trace();
        if let Some(cmd) = authority.latest_command_for_unit(unit_id) {
            let needs_record = store
                .latest_for_unit(unit_id)
                .map(|trace| trace.authority_sequence != cmd.sequence)
                .unwrap_or(true);
            if needs_record {
                if let Some(record) = world.get_unit(unit_id) {
                    if let crate::world::UnitState::Moving {
                        path,
                        target,
                        waypoint_index,
                        ..
                    } = &record.state
                    {
                        store.record_committed(
                            unit_id,
                            record.placement.position,
                            *target,
                            cmd.start_space,
                            cmd.goal_space,
                            path.clone(),
                            cmd.sequence,
                        );
                        store.sync_live_waypoint(unit_id, *waypoint_index);
                    }
                }
            }
        }

        if let Some(record) = world.get_unit(unit_id) {
            match &record.state {
                crate::world::UnitState::Moving { waypoint_index, .. } => {
                    store.sync_live_waypoint(unit_id, *waypoint_index);
                }
                crate::world::UnitState::Idle => {
                    if store
                        .latest_for_unit(unit_id)
                        .is_some_and(|trace| trace.status == PathTraceStatus::Active)
                    {
                        store.mark_completed(unit_id);
                    }
                }
                _ => {}
            }
        }

        if let Some(blocked) = authority.latest_blocked_for_unit(unit_id) {
            store.mark_failed(
                unit_id,
                blocked.blocker_description.clone(),
                Some(blocked.candidate_position),
                Some(blocked.passability_fn.to_string()),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{ChunkCoord, LocalPosition};
    use bevy::prelude::Vec3;

    fn position(x: f32, z: f32) -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(x, 0.0, z)),
        )
    }

    fn empty_path() -> NavigationPath {
        NavigationPath { waypoints: vec![] }
    }

    #[test]
    fn retained_path_survives_completion() {
        let mut store = UnitPathDiagnosticStore {
            capacity: 2,
            ..Default::default()
        };
        let unit = UnitId::new(1);
        store.record_committed(
            unit,
            position(0.0, 0.0),
            position(5.0, 5.0),
            SpaceId::SURFACE,
            SpaceId::SURFACE,
            empty_path(),
            1,
        );
        store.mark_completed(unit);
        let trace = store.latest_for_unit(unit).expect("trace");
        assert_eq!(trace.status, PathTraceStatus::Completed);
    }

    #[test]
    fn clear_unit_removes_trace() {
        let mut store = UnitPathDiagnosticStore::default();
        let unit = UnitId::new(2);
        store.record_committed(
            unit,
            position(0.0, 0.0),
            position(1.0, 1.0),
            SpaceId::SURFACE,
            SpaceId::SURFACE,
            empty_path(),
            2,
        );
        store.clear_unit(unit);
        assert!(store.latest_for_unit(unit).is_none());
    }
}
