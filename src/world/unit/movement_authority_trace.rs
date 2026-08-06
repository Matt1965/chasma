//! Bounded movement-authority trace for IN-11eR dev diagnostics.
//!
//! Records command resolution and blocked movement frames so live player-command
//! paths can be compared against passability call sites without per-frame spam.

use std::collections::VecDeque;

use crate::world::{NavigationWaypoint, SpaceId, UnitId, WorldPosition};

/// One resolved player move command (path committed).
#[derive(Debug, Clone, PartialEq)]
pub struct MovementCommandAuthorityRecord {
    pub sequence: u64,
    pub unit_id: UnitId,
    pub unit_space_before: SpaceId,
    pub click_target: WorldPosition,
    pub start_space: SpaceId,
    pub goal_space: SpaceId,
    pub grounded_goal: WorldPosition,
    pub waypoint_spaces: Vec<SpaceId>,
}

/// One blocked movement step with passability authority detail.
#[derive(Debug, Clone, PartialEq)]
pub struct MovementBlockedAuthorityRecord {
    pub sequence: u64,
    pub unit_id: UnitId,
    pub unit_space_id: SpaceId,
    pub waypoint_space_id: SpaceId,
    pub validation_space_id: SpaceId,
    pub waypoint_index: usize,
    pub candidate_position: WorldPosition,
    pub passability_fn: &'static str,
    pub blocker_description: String,
    pub segment_boundary_checked: bool,
    pub segment_boundary_ok: bool,
}

/// Dev-only authority violation (fail-loud, bounded).
#[derive(Debug, Clone, PartialEq)]
pub struct MovementAuthorityViolation {
    pub sequence: u64,
    pub unit_id: UnitId,
    pub description: String,
}

/// Latest command + blocked frames + violations for the selected unit.
#[derive(Debug, Clone, Default)]
pub struct MovementAuthorityTrace {
    commands: VecDeque<MovementCommandAuthorityRecord>,
    blocked: VecDeque<MovementBlockedAuthorityRecord>,
    violations: VecDeque<MovementAuthorityViolation>,
    next_sequence: u64,
    /// Unit id filters diagnostics to one selection in dev UI.
    pub focus_unit_id: Option<UnitId>,
}

impl MovementAuthorityTrace {
    pub const COMMAND_CAPACITY: usize = 4;
    pub const BLOCKED_CAPACITY: usize = 8;
    pub const VIOLATION_CAPACITY: usize = 8;

    pub fn record_command(&mut self, mut record: MovementCommandAuthorityRecord) -> u64 {
        let sequence = self.next_sequence;
        record.sequence = sequence;
        self.next_sequence = self.next_sequence.wrapping_add(1);
        if self.commands.len() == Self::COMMAND_CAPACITY {
            self.commands.pop_front();
        }
        self.commands.push_back(record);
        sequence
    }

    pub fn record_blocked(&mut self, mut record: MovementBlockedAuthorityRecord) -> u64 {
        let sequence = self.next_sequence;
        record.sequence = sequence;
        self.next_sequence = self.next_sequence.wrapping_add(1);
        if self.blocked.len() == Self::BLOCKED_CAPACITY {
            self.blocked.pop_front();
        }
        self.blocked.push_back(record);
        sequence
    }

    pub fn record_violation(&mut self, unit_id: UnitId, description: impl Into<String>) -> u64 {
        let sequence = self.next_sequence;
        self.next_sequence = self.next_sequence.wrapping_add(1);
        if self.violations.len() == Self::VIOLATION_CAPACITY {
            self.violations.pop_front();
        }
        self.violations.push_back(MovementAuthorityViolation {
            sequence,
            unit_id,
            description: description.into(),
        });
        sequence
    }

    pub fn latest_command_for_unit(
        &self,
        unit_id: UnitId,
    ) -> Option<&MovementCommandAuthorityRecord> {
        self.commands
            .iter()
            .rev()
            .find(|entry| entry.unit_id == unit_id)
    }

    pub fn latest_blocked_for_unit(
        &self,
        unit_id: UnitId,
    ) -> Option<&MovementBlockedAuthorityRecord> {
        self.blocked
            .iter()
            .rev()
            .find(|entry| entry.unit_id == unit_id)
    }

    pub fn latest_violation_for_unit(
        &self,
        unit_id: UnitId,
    ) -> Option<&MovementAuthorityViolation> {
        self.violations
            .iter()
            .rev()
            .find(|entry| entry.unit_id == unit_id)
    }

    pub fn commands_for_unit(
        &self,
        unit_id: UnitId,
    ) -> impl Iterator<Item = &MovementCommandAuthorityRecord> {
        self.commands
            .iter()
            .filter(move |entry| entry.unit_id == unit_id)
    }

    pub fn blocked_for_unit(
        &self,
        unit_id: UnitId,
    ) -> impl Iterator<Item = &MovementBlockedAuthorityRecord> {
        self.blocked
            .iter()
            .filter(move |entry| entry.unit_id == unit_id)
    }

    pub fn diagnostic_line_for_unit(&self, unit_id: UnitId) -> String {
        let mut parts = Vec::new();
        if let Some(cmd) = self.latest_command_for_unit(unit_id) {
            parts.push(format!(
                "cmd start={} goal={} waypoints={}",
                cmd.start_space.raw(),
                cmd.goal_space.raw(),
                format_waypoint_spaces(&cmd.waypoint_spaces)
            ));
        }
        if let Some(blocked) = self.latest_blocked_for_unit(unit_id) {
            parts.push(format!(
                "blocked unit_space={} wp_space={} validate={} fn={} | {}",
                blocked.unit_space_id.raw(),
                blocked.waypoint_space_id.raw(),
                blocked.validation_space_id.raw(),
                blocked.passability_fn,
                blocked.blocker_description
            ));
            if blocked.segment_boundary_checked {
                parts.push(format!(
                    "segment_boundary_ok={}",
                    blocked.segment_boundary_ok
                ));
            }
        }
        if let Some(v) = self.latest_violation_for_unit(unit_id) {
            parts.push(format!("violation: {}", v.description));
        }
        if parts.is_empty() {
            "no movement authority trace for unit".to_string()
        } else {
            parts.join(" | ")
        }
    }

    pub fn clear(&mut self) {
        self.commands.clear();
        self.blocked.clear();
        self.violations.clear();
    }
}

pub fn format_waypoint_spaces(spaces: &[SpaceId]) -> String {
    if spaces.is_empty() {
        return "[]".to_string();
    }
    let mut out = String::from("[");
    for (index, space) in spaces.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str(&space.raw().to_string());
    }
    out.push(']');
    out
}

pub fn waypoint_space_ids(waypoints: &[NavigationWaypoint]) -> Vec<SpaceId> {
    waypoints.iter().map(|wp| wp.space_id).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{ChunkCoord, LocalPosition};
    use bevy::prelude::Vec3;

    fn position() -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(1.0, 2.0, 3.0)),
        )
    }

    #[test]
    fn bounded_capacity_drops_oldest_entries() {
        let mut trace = MovementAuthorityTrace::default();
        let unit = UnitId::new(1);
        for _ in 0..(MovementAuthorityTrace::BLOCKED_CAPACITY + 2) {
            trace.record_blocked(MovementBlockedAuthorityRecord {
                sequence: 0,
                unit_id: unit,
                unit_space_id: SpaceId::SURFACE,
                waypoint_space_id: SpaceId::SURFACE,
                validation_space_id: SpaceId::SURFACE,
                waypoint_index: 0,
                candidate_position: position(),
                passability_fn: "query_surface_passability",
                blocker_description: "test".into(),
                segment_boundary_checked: false,
                segment_boundary_ok: true,
            });
        }
        assert_eq!(
            trace.blocked.len(),
            MovementAuthorityTrace::BLOCKED_CAPACITY
        );
    }

    #[test]
    fn diagnostic_line_summarizes_latest_command_and_block() {
        let mut trace = MovementAuthorityTrace::default();
        let unit = UnitId::new(7);
        trace.record_command(MovementCommandAuthorityRecord {
            sequence: 0,
            unit_id: unit,
            unit_space_before: SpaceId::new(3),
            click_target: position(),
            start_space: SpaceId::new(3),
            goal_space: SpaceId::new(3),
            grounded_goal: position(),
            waypoint_spaces: vec![SpaceId::new(3), SpaceId::new(3)],
        });
        trace.record_blocked(MovementBlockedAuthorityRecord {
            sequence: 0,
            unit_id: unit,
            unit_space_id: SpaceId::new(3),
            waypoint_space_id: SpaceId::new(3),
            validation_space_id: SpaceId::new(3),
            waypoint_index: 1,
            candidate_position: position(),
            passability_fn: "query_interior_passability",
            blocker_description: "region boundary".into(),
            segment_boundary_checked: true,
            segment_boundary_ok: false,
        });
        let line = trace.diagnostic_line_for_unit(unit);
        assert!(line.contains("goal=3"));
        assert!(line.contains("query_interior_passability"));
        assert!(line.contains("segment_boundary_ok=false"));
    }
}
