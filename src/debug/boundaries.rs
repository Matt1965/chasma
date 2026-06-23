//! Client layer boundary guards (ADR-039 U-UI3).

use bevy::prelude::*;

/// Tracks which client pipeline phase is active for debug assertions.
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ClientBoundaryGuard {
    pub collecting_input: bool,
    pub dispatching_intents: bool,
}

impl ClientBoundaryGuard {
    pub fn begin_input_collection(&mut self) {
        debug_assert!(
            !self.dispatching_intents,
            "input collection must not overlap intent dispatch"
        );
        self.collecting_input = true;
    }

    pub fn end_input_collection(&mut self) {
        self.collecting_input = false;
    }

    pub fn begin_intent_dispatch(&mut self) {
        debug_assert!(
            !self.collecting_input,
            "intent dispatch must not overlap input collection"
        );
        self.dispatching_intents = true;
    }

    pub fn end_intent_dispatch(&mut self) {
        self.dispatching_intents = false;
    }
}

/// Advance the client frame index at the start of the player control chain.
pub fn advance_client_frame_index(mut frame_index: ResMut<crate::debug::trace::ClientFrameIndex>) {
    frame_index.0 = frame_index.0.saturating_add(1);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_settings_default_do_not_mutate_world() {
        let settings = crate::debug::settings::DebugOverlaySettings::default();
        assert!(settings.enabled);
        assert!(settings.path);
    }
}
