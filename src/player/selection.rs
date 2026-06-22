//! Client-local unit selection state (ADR-033 U8).

use bevy::prelude::*;

use crate::world::UnitId;

/// Single-unit selection for the local player (SC2-style, U8).
///
/// Client-local only — not written to [`crate::world::WorldData`].
#[derive(Debug, Resource, Default, Clone, Copy, PartialEq, Eq, Reflect)]
pub struct PlayerUnitSelection {
    pub selected: Option<UnitId>,
}

impl PlayerUnitSelection {
    pub fn select(&mut self, unit_id: UnitId) {
        self.selected = Some(unit_id);
    }

    pub fn clear(&mut self) {
        self.selected = None;
    }
}
