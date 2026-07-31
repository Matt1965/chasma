//! Player-facing building selection for HUD (ADR-082 B5).
//!
//! **Transitional mirror (Dev UI Slice 1):** written only by
//! [`crate::client::selection::apply_world_selection`]. Readers should migrate to
//! [`crate::client::selection::WorldSelectionState`] directly; this resource will be retired.

use bevy::prelude::*;

use crate::world::BuildingId;

/// Client-local selected building for HUD display (not simulation truth).
#[derive(Resource, Debug, Clone, Default, PartialEq, Eq)]
pub struct GameplayBuildingSelection {
    pub building_id: Option<BuildingId>,
}

impl GameplayBuildingSelection {
    pub fn set(&mut self, building_id: Option<BuildingId>) {
        self.building_id = building_id;
    }
}
