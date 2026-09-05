//! Client-local Settlement Workforce panel state.

use bevy::prelude::*;

/// Player-facing Settlement Workforce floating window state.
///
/// Settlement context is read live from [`crate::client::CameraSettlementContext`]; it is not
/// cached here so focus changes always rebuild the matrix.
#[derive(Resource, Debug, Clone, Default, PartialEq, Eq)]
pub struct SettlementWorkforcePanelState {
    pub open: bool,
}

impl SettlementWorkforcePanelState {
    pub fn close(&mut self) {
        self.open = false;
    }

    pub fn open_panel(&mut self) {
        self.open = true;
    }

    pub fn toggle(&mut self) {
        self.open = !self.open;
    }
}
