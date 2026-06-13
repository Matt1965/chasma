//! Chunk residency tracker (ADR-012 Phase 2B.5).
//!
//! Owns in-flight and resident lifecycle state for streaming. Authoritative
//! terrain remains in [`WorldData`]; this tracker does not hold [`ChunkData`].

use std::collections::HashMap;

use bevy::prelude::*;

use crate::world::ChunkId;

/// Per-chunk residency state for terrain streaming.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChunkResidencyState {
    /// Not resident and not loading.
    Absent,
    /// Async materialization in progress.
    Loading { generation: u64 },
    /// Resident in [`WorldData`] with a derived mesh entity.
    Resident,
}

/// Tracks chunk residency for async materialization (terrain runtime only).
#[derive(Debug, Resource, Default)]
pub struct ChunkResidencyTracker {
    states: HashMap<ChunkId, ChunkResidencyState>,
    next_generation: u64,
}

impl ChunkResidencyTracker {
    pub fn state(&self, chunk_id: ChunkId) -> ChunkResidencyState {
        self.states
            .get(&chunk_id)
            .copied()
            .unwrap_or(ChunkResidencyState::Absent)
    }

    pub fn is_loading(&self, chunk_id: ChunkId) -> bool {
        matches!(self.state(chunk_id), ChunkResidencyState::Loading { .. })
    }

    pub fn is_resident(&self, chunk_id: ChunkId) -> bool {
        matches!(self.state(chunk_id), ChunkResidencyState::Resident)
    }

    /// Start loading if the chunk is [`ChunkResidencyState::Absent`].
    ///
    /// Returns the new generation token when loading begins. Returns `None` if
    /// the chunk is already loading or resident (duplicate request blocked).
    pub fn begin_loading(&mut self, chunk_id: ChunkId) -> Option<u64> {
        match self.state(chunk_id) {
            ChunkResidencyState::Absent => {
                let generation = self.next_generation;
                self.next_generation += 1;
                self.states.insert(
                    chunk_id,
                    ChunkResidencyState::Loading { generation },
                );
                Some(generation)
            }
            ChunkResidencyState::Loading { .. } | ChunkResidencyState::Resident => None,
        }
    }

    /// Cancel loading or clear residency bookkeeping (does not touch [`WorldData`]).
    pub fn cancel(&mut self, chunk_id: ChunkId) {
        self.states.insert(chunk_id, ChunkResidencyState::Absent);
    }

    /// Mark a chunk resident after a successful apply (Phase 2B.5 step 2+).
    pub fn mark_resident(&mut self, chunk_id: ChunkId) {
        self.states.insert(chunk_id, ChunkResidencyState::Resident);
    }

    /// Returns true if `generation` matches the tracker's current loading token.
    pub fn loading_generation_matches(&self, chunk_id: ChunkId, generation: u64) -> bool {
        matches!(
            self.state(chunk_id),
            ChunkResidencyState::Loading {
                generation: current
            } if current == generation
        )
    }

    /// All chunks currently in [`ChunkResidencyState::Loading`].
    pub fn loading_chunk_ids(&self) -> Vec<(ChunkId, u64)> {
        self.states
            .iter()
            .filter_map(|(&id, &state)| match state {
                ChunkResidencyState::Loading { generation } => Some((id, generation)),
                _ => None,
            })
            .collect()
    }
}

/// Why chunk residency / pipeline state is being torn down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChunkDiscardKind {
    /// Chunk left residency rings or is being unloaded — clear unconditionally.
    Revoked,
    /// Async work was rejected or failed; clear only when `generation` still matches.
    RejectedCompletion { generation: u64 },
}

/// Canonical residency cleanup for abandoned chunk work.
pub fn discard_chunk_residency(
    residency: &mut ChunkResidencyTracker,
    chunk_id: ChunkId,
    kind: ChunkDiscardKind,
) {
    match kind {
        ChunkDiscardKind::Revoked => residency.cancel(chunk_id),
        ChunkDiscardKind::RejectedCompletion { generation } => {
            if residency.loading_generation_matches(chunk_id, generation) {
                residency.cancel(chunk_id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::ChunkCoord;

    fn id(x: i32, z: i32) -> ChunkId {
        ChunkId::new(ChunkCoord::new(x, z))
    }

    #[test]
    fn registers_loading_once_per_chunk() {
        let mut tracker = ChunkResidencyTracker::default();
        let chunk = id(0, 0);

        let generation = tracker.begin_loading(chunk).expect("first request");
        assert!(tracker.is_loading(chunk));
        assert!(tracker.loading_generation_matches(chunk, generation));
        assert!(tracker.begin_loading(chunk).is_none());
    }

    #[test]
    fn duplicate_load_attempts_are_blocked() {
        let mut tracker = ChunkResidencyTracker::default();
        let a = id(1, 0);
        let b = id(2, 0);

        assert!(tracker.begin_loading(a).is_some());
        assert!(tracker.begin_loading(a).is_none());
        assert!(tracker.begin_loading(b).is_some());
        assert!(tracker.begin_loading(b).is_none());
    }

    #[test]
    fn cancel_allows_reregistration() {
        let mut tracker = ChunkResidencyTracker::default();
        let chunk = id(0, 1);

        let first = tracker.begin_loading(chunk).unwrap();
        tracker.cancel(chunk);
        assert_eq!(tracker.state(chunk), ChunkResidencyState::Absent);

        let second = tracker.begin_loading(chunk).unwrap();
        assert_ne!(first, second);
        assert!(tracker.loading_generation_matches(chunk, second));
        assert!(!tracker.loading_generation_matches(chunk, first));
    }

    #[test]
    fn resident_blocks_new_load_until_cancelled() {
        let mut tracker = ChunkResidencyTracker::default();
        let chunk = id(3, 3);

        tracker.mark_resident(chunk);
        assert!(tracker.begin_loading(chunk).is_none());
        tracker.cancel(chunk);
        assert!(tracker.begin_loading(chunk).is_some());
    }

    #[test]
    fn discard_rejected_completion_clears_matching_generation() {
        let mut tracker = ChunkResidencyTracker::default();
        let chunk = id(2, 2);
        let generation = tracker.begin_loading(chunk).unwrap();

        discard_chunk_residency(
            &mut tracker,
            chunk,
            ChunkDiscardKind::RejectedCompletion { generation },
        );
        assert_eq!(tracker.state(chunk), ChunkResidencyState::Absent);
        assert!(tracker.begin_loading(chunk).is_some());
    }

    #[test]
    fn discard_rejected_completion_preserves_newer_generation() {
        let mut tracker = ChunkResidencyTracker::default();
        let chunk = id(1, 1);
        let stale = tracker.begin_loading(chunk).unwrap();
        tracker.cancel(chunk);
        let current = tracker.begin_loading(chunk).unwrap();

        discard_chunk_residency(
            &mut tracker,
            chunk,
            ChunkDiscardKind::RejectedCompletion { generation: stale },
        );
        assert!(tracker.loading_generation_matches(chunk, current));
    }

    #[test]
    fn discard_revoked_clears_unconditionally() {
        let mut tracker = ChunkResidencyTracker::default();
        let chunk = id(4, 4);
        let _ = tracker.begin_loading(chunk).unwrap();
        discard_chunk_residency(&mut tracker, chunk, ChunkDiscardKind::Revoked);
        assert_eq!(tracker.state(chunk), ChunkResidencyState::Absent);
    }
}
