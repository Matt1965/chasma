//! Last right-click screen position for contextual unit menus.

use bevy::prelude::*;

/// Set during unit right-click intent collection; consumed when opening interaction menus.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct ContextMenuScreenAnchor {
    pub position: Option<Vec2>,
}

impl ContextMenuScreenAnchor {
    pub fn take(&mut self) -> Option<Vec2> {
        self.position.take()
    }
}
