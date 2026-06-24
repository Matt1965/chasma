//! Inspector ↔ overlay focus link (ADR-048 U-DEV2). Presentation only.

use bevy::prelude::*;

use crate::world::UnitId;

/// Unit highlighted by the world inspector for enhanced debug overlays.
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct InspectorOverlayFocus {
    pub unit_id: Option<UnitId>,
    /// Path waypoint index to emphasize when linked from path inspector UI.
    pub path_waypoint_index: Option<usize>,
}

impl InspectorOverlayFocus {
    pub fn set_unit(&mut self, unit_id: Option<UnitId>) {
        self.unit_id = unit_id;
        self.path_waypoint_index = None;
    }

    pub fn is_focused(&self, unit_id: UnitId) -> bool {
        self.unit_id == Some(unit_id)
    }
}
