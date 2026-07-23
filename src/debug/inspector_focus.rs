//! Inspector ↔ overlay focus link (ADR-048 U-DEV2). Presentation only.

use bevy::prelude::*;

use crate::world::{BlueprintDiagnosticFocus, BuildingId, UnitId};

/// Unit highlighted by the world inspector for enhanced debug overlays.
#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub struct InspectorOverlayFocus {
    pub unit_id: Option<UnitId>,
    /// Path waypoint index to emphasize when linked from path inspector UI.
    pub path_waypoint_index: Option<usize>,
    /// Building under blueprint inspection overlay (NV1.2.5).
    pub blueprint_building_id: Option<BuildingId>,
    pub blueprint_floor_id: Option<i32>,
    pub blueprint_diagnostic: Option<BlueprintDiagnosticFocus>,
}

impl InspectorOverlayFocus {
    pub fn set_unit(&mut self, unit_id: Option<UnitId>) {
        self.unit_id = unit_id;
        self.path_waypoint_index = None;
    }

    pub fn clear_blueprint(&mut self) {
        self.blueprint_building_id = None;
        self.blueprint_floor_id = None;
        self.blueprint_diagnostic = None;
    }

    pub fn is_focused(&self, unit_id: UnitId) -> bool {
        self.unit_id == Some(unit_id)
    }
}
