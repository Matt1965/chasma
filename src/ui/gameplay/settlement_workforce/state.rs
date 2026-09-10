//! Client-local Settlement Workforce panel state.

use bevy::prelude::*;

/// Player-facing Settlement Workforce floating window state.
///
/// Settlement context is read live from [`crate::client::CameraSettlementContext`]; it is not
/// cached here so focus changes always rebuild the matrix.
#[derive(Resource, Debug, Clone, Default, PartialEq, Eq)]
pub struct SettlementWorkforcePanelState {
    pub open: bool,
    /// Bumped whenever the panel becomes visible so UI sync must materialize content even if
    /// the semantic snapshot matches a prior frame (for example reopen after close).
    pub presentation_revision: u64,
}

impl SettlementWorkforcePanelState {
    pub fn close(&mut self) {
        self.open = false;
    }

    pub fn open_panel(&mut self) {
        if !self.open {
            self.presentation_revision += 1;
        }
        self.open = true;
    }

    pub fn toggle(&mut self) {
        if self.open {
            self.close();
        } else {
            self.open_panel();
        }
    }
}
