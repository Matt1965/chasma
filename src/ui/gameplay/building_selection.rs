//! Player-facing building selection for HUD (ADR-082 B5).

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
