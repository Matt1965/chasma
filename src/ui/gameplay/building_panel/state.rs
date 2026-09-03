//! Open Building Menu presentation state (BP1).

use bevy::prelude::*;

use crate::world::BuildingId;

/// Which owned building's full menu is currently open.
///
/// Independent from [`crate::client::selection::WorldSelectionState`]: the menu can stay open
/// while world selection changes to units, terrain, or other buildings.
#[derive(Resource, Debug, Clone, Default, PartialEq, Eq)]
pub struct BuildingPanelState {
    pub open_building_id: Option<BuildingId>,
}

impl BuildingPanelState {
    pub fn open(&mut self, building_id: BuildingId) {
        self.open_building_id = Some(building_id);
    }

    pub fn close(&mut self) {
        self.open_building_id = None;
    }

    pub fn is_open(&self) -> bool {
        self.open_building_id.is_some()
    }
}
