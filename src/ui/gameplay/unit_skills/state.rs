//! Client-local Unit Skills panel state.

use bevy::prelude::*;

use crate::world::UnitId;

/// Player-facing Unit Skills floating window state (read-only).
#[derive(Resource, Debug, Clone, Default, PartialEq, Eq)]
pub struct UnitSkillsPanelState {
    pub open: bool,
    pub displayed_unit_id: Option<UnitId>,
}

impl UnitSkillsPanelState {
    pub fn close(&mut self) {
        *self = Self::default();
    }

    pub fn open_for(&mut self, unit_id: UnitId) {
        self.open = true;
        self.displayed_unit_id = Some(unit_id);
    }
}
