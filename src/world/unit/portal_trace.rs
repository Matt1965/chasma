//! Bounded portal-transition trace for dev diagnostics (IN-11c).
//!
//! Movement records one entry per completed traversal, never per frame, so the trace
//! stays cheap enough to leave enabled and still answers "what happened when the unit
//! entered the building".

use std::collections::VecDeque;

use crate::world::{PortalId, SpaceId, UnitId, WorldPosition};

/// One completed portal traversal.
///
/// `sequence` is assigned by [`PortalTransitionTrace::record`]; callers leave it zero.
#[derive(Debug, Clone, PartialEq)]
pub struct PortalTransitionEvent {
    pub sequence: u64,
    pub unit_id: UnitId,
    pub portal_id: PortalId,
    pub from_space: SpaceId,
    pub to_space: SpaceId,
    /// Position the unit occupied when the portal triggered.
    pub from_position: WorldPosition,
    /// Portal-declared destination before grounding in the destination space.
    pub destination_position: WorldPosition,
    /// Destination after grounding — where the unit actually landed.
    pub grounded_position: WorldPosition,
    /// Floor plane of the destination space, when it has one.
    pub destination_floor_y: Option<f32>,
    /// Path waypoints still queued after the portal waypoint was consumed.
    pub waypoints_remaining: usize,
}

impl PortalTransitionEvent {
    /// Vertical distance between the landing position and the destination floor plane.
    pub fn floor_offset_meters(&self, layout: crate::world::ChunkLayout) -> Option<f32> {
        let floor_y = self.destination_floor_y?;
        Some(self.grounded_position.to_global(layout).y - floor_y)
    }
}

/// Latest portal traversals, oldest first.
#[derive(Debug, Clone, Default)]
pub struct PortalTransitionTrace {
    events: VecDeque<PortalTransitionEvent>,
    next_sequence: u64,
}

impl PortalTransitionTrace {
    /// Retained traversals. Old entries are dropped rather than growing the buffer.
    pub const CAPACITY: usize = 8;

    pub fn record(&mut self, mut event: PortalTransitionEvent) -> u64 {
        let sequence = self.next_sequence;
        event.sequence = sequence;
        self.next_sequence = self.next_sequence.wrapping_add(1);
        if self.events.len() == Self::CAPACITY {
            self.events.pop_front();
        }
        self.events.push_back(event);
        sequence
    }

    pub fn latest(&self) -> Option<&PortalTransitionEvent> {
        self.events.back()
    }

    pub fn latest_for_unit(&self, unit_id: UnitId) -> Option<&PortalTransitionEvent> {
        self.events
            .iter()
            .rev()
            .find(|event| event.unit_id == unit_id)
    }

    pub fn events(&self) -> impl Iterator<Item = &PortalTransitionEvent> {
        self.events.iter()
    }

    /// Traversals recorded for `unit_id` since the trace was created.
    pub fn count_for_unit(&self, unit_id: UnitId) -> usize {
        self.events
            .iter()
            .filter(|event| event.unit_id == unit_id)
            .count()
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn clear(&mut self) {
        self.events.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{ChunkCoord, LocalPosition};
    use bevy::prelude::Vec3;

    fn event(unit: u64) -> PortalTransitionEvent {
        let position = WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(1.0, 2.0, 3.0)),
        );
        PortalTransitionEvent {
            sequence: 0,
            unit_id: UnitId::new(unit),
            portal_id: PortalId::new(1),
            from_space: SpaceId::SURFACE,
            to_space: SpaceId::new(1),
            from_position: position,
            destination_position: position,
            grounded_position: position,
            destination_floor_y: Some(2.0),
            waypoints_remaining: 1,
        }
    }

    #[test]
    fn sequences_increase_and_capacity_is_bounded() {
        let mut trace = PortalTransitionTrace::default();
        for index in 0..(PortalTransitionTrace::CAPACITY as u64 + 3) {
            assert_eq!(trace.record(event(index)), index);
        }
        assert_eq!(trace.len(), PortalTransitionTrace::CAPACITY);
        assert_eq!(
            trace.latest().expect("latest").sequence,
            PortalTransitionTrace::CAPACITY as u64 + 2
        );
    }

    #[test]
    fn latest_for_unit_selects_the_most_recent_entry() {
        let mut trace = PortalTransitionTrace::default();
        trace.record(event(7));
        trace.record(event(9));
        trace.record(event(7));
        let latest = trace.latest_for_unit(UnitId::new(7)).expect("entry");
        assert_eq!(latest.sequence, 2);
        assert_eq!(trace.count_for_unit(UnitId::new(7)), 2);
        assert_eq!(trace.count_for_unit(UnitId::new(9)), 1);
    }

    #[test]
    fn floor_offset_measures_landing_against_the_floor_plane() {
        let layout = crate::world::ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        };
        let mut sample = event(1);
        sample.destination_floor_y = Some(1.5);
        assert_eq!(sample.floor_offset_meters(layout), Some(0.5));
    }
}
