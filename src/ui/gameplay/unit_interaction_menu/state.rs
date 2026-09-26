//! Client-local unit interaction menu state.

use bevy::prelude::*;

use crate::world::UnitId;

/// Generic contextual unit-action picker (not dialogue session state).
#[derive(Resource, Debug, Clone, Default)]
pub struct UnitInteractionMenuState {
    pub open: bool,
    pub actor_unit_id: Option<UnitId>,
    pub target_unit_id: Option<UnitId>,
    pub screen_position: Vec2,
}

impl UnitInteractionMenuState {
    pub fn open_at(&mut self, actor_unit_id: UnitId, target_unit_id: UnitId, screen_position: Vec2) {
        self.open = true;
        self.actor_unit_id = Some(actor_unit_id);
        self.target_unit_id = Some(target_unit_id);
        self.screen_position = screen_position;
    }

    pub fn close(&mut self) {
        *self = Self::default();
    }

    pub fn blocks_world_input(&self) -> bool {
        self.open
    }
}
