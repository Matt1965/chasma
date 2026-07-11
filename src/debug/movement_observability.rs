//! Read-only last-known movement block reasons (ADR-066).
//!
//! Updated from simulation tick reports; not authoritative world state.

use std::collections::HashMap;

use bevy::prelude::*;

use crate::world::{UnitId, UnitMovementTrace};

/// Last reported movement block per unit (inspector / debug only).
#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub struct MovementBlockObservability {
    pub last_by_unit: HashMap<UnitId, UnitMovementTrace>,
}

impl MovementBlockObservability {
    pub fn apply_batch_traces(&mut self, traces: &[UnitMovementTrace]) {
        for trace in traces {
            self.last_by_unit.insert(trace.unit_id, trace.clone());
        }
    }

    /// Returns only traces whose block reason changed since the last observation.
    pub fn filter_new_block_traces(
        &mut self,
        traces: &[UnitMovementTrace],
    ) -> Vec<UnitMovementTrace> {
        let mut fresh = Vec::new();
        for trace in traces {
            let changed = self
                .last_by_unit
                .get(&trace.unit_id)
                .map(|prior| prior.reason)
                != Some(trace.reason);
            if changed {
                fresh.push(trace.clone());
            }
            self.last_by_unit.insert(trace.unit_id, trace.clone());
        }
        fresh
    }

    pub fn last_for_unit(&self, unit_id: UnitId) -> Option<&UnitMovementTrace> {
        self.last_by_unit.get(&unit_id)
    }
}

pub fn blocked_reason_label(reason: crate::world::BlockedMovementReason) -> &'static str {
    use crate::world::BlockedMovementReason;
    match reason {
        BlockedMovementReason::TerrainUnavailable => "Terrain unavailable",
        BlockedMovementReason::SlopeUnavailable => "Slope unavailable",
        BlockedMovementReason::SlopeTooSteep => "Slope too steep",
        BlockedMovementReason::BlockedByDoodad => "Blocked by doodad",
        BlockedMovementReason::DestinationBlocked => "Destination blocked",
        BlockedMovementReason::PathUnavailable => "Path unavailable",
        BlockedMovementReason::InvalidWaypoint => "Invalid waypoint",
        BlockedMovementReason::TargetUnavailable => "Target unavailable",
    }
}
