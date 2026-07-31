//! World inspector derived snapshot cache (ADR-048).
//!
//! Selection authority lives in [`crate::client::selection::WorldSelectionState`].
//! This resource holds read-only cached snapshots for dev inspection UI.

use bevy::prelude::*;

use super::snapshot::{
    BuildingInspectorSnapshot, DoodadInspectorSnapshot, InteractionInspectorSnapshot,
    ItemPileInspectorSnapshot, UnitInspectorSnapshot,
};

/// Cached read-only inspection state — not simulation truth.
#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub struct WorldInspectorState {
    pub unit_snapshot: Option<UnitInspectorSnapshot>,
    pub building_snapshot: Option<BuildingInspectorSnapshot>,
    pub blueprint_snapshot: Option<super::snapshot::BuildingBlueprintInspectorSnapshot>,
    pub doodad_snapshot: Option<DoodadInspectorSnapshot>,
    pub pile_snapshot: Option<ItemPileInspectorSnapshot>,
    pub interaction_snapshot: Option<InteractionInspectorSnapshot>,
    pub cache_key: InspectorCacheKey,
    pub last_message: String,
    pub production_advanced_expanded: bool,
}

/// Invalidates cached snapshots when selection or pause state changes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct InspectorCacheKey {
    pub category: crate::client::selection::WorldSelectionCategory,
    pub unit_id: Option<crate::world::UnitId>,
    pub building_id: Option<crate::world::BuildingId>,
    pub doodad_id: Option<crate::world::DoodadId>,
    pub pile_id: Option<crate::world::ItemPileId>,
    pub simulation_tick: u64,
    pub paused: bool,
}

impl WorldInspectorState {
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn invalidate_for_selection_change(&mut self) {
        self.unit_snapshot = None;
        self.building_snapshot = None;
        self.blueprint_snapshot = None;
        self.doodad_snapshot = None;
        self.pile_snapshot = None;
        self.interaction_snapshot = None;
        self.cache_key = InspectorCacheKey::default();
    }

    pub fn needs_refresh(&self, key: InspectorCacheKey) -> bool {
        self.cache_key != key
            || (key.unit_id.is_some() && self.unit_snapshot.is_none())
            || (key.building_id.is_some() && self.building_snapshot.is_none())
            || (key.doodad_id.is_some() && self.doodad_snapshot.is_none())
    }
}
