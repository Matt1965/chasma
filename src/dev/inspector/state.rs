//! World inspector session state (ADR-048).

use bevy::prelude::*;

use crate::world::UnitId;

use super::snapshot::{InteractionInspectorSnapshot, UnitInspectorSnapshot};

/// Cached read-only inspection state — not simulation truth.
#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub struct WorldInspectorState {
    pub selected_unit: Option<UnitId>,
    pub unit_snapshot: Option<UnitInspectorSnapshot>,
    pub interaction_snapshot: Option<InteractionInspectorSnapshot>,
    pub cache_key: InspectorCacheKey,
    pub last_message: String,
}

/// Invalidates cached snapshots when selection or pause state changes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct InspectorCacheKey {
    pub unit_id: Option<UnitId>,
    pub simulation_tick: u64,
    pub paused: bool,
}

impl WorldInspectorState {
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn select_unit(&mut self, unit_id: UnitId) {
        self.selected_unit = Some(unit_id);
        self.interaction_snapshot = None;
        self.unit_snapshot = None;
        self.cache_key = InspectorCacheKey::default();
    }

    pub fn needs_refresh(&self, key: InspectorCacheKey) -> bool {
        self.cache_key != key || self.unit_snapshot.is_none()
    }
}
