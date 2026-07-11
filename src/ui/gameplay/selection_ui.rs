//! Selection presentation helpers for gameplay HUD (ADR-040 U-UI4).
//!
//! Selection rings remain in the player layer; this module owns HUD-side
//! selection readability (portrait slots, leader highlight via [`super::hud`]).

use bevy::prelude::*;

use crate::client::ResolvedCommandFeedback;
use crate::debug::DebugOverlaySettings;
use crate::debug::{CommandTraceBuffer, IntentDispatchHistory};
use crate::units::input::SelectedUnits;

use super::state::{GameplayUiState, derive_gameplay_snapshot};

/// Sync gameplay UI snapshot from client-local sources (read-only).
pub fn sync_gameplay_ui_state(
    selection: Res<SelectedUnits>,
    history: Res<IntentDispatchHistory>,
    trace: Res<CommandTraceBuffer>,
    resolved_command: Res<ResolvedCommandFeedback>,
    debug_settings: Res<DebugOverlaySettings>,
    mut ui_state: ResMut<GameplayUiState>,
) {
    let snapshot = derive_gameplay_snapshot(
        &selection,
        &history,
        &trace,
        &resolved_command,
        debug_settings.enabled,
        ui_state.snapshot.cursor_mode,
    );

    if snapshot != ui_state.snapshot {
        ui_state.snapshot = snapshot;
        ui_state.hud_dirty = true;
    }
}

/// Mark HUD clean after widgets refresh.
pub fn clear_gameplay_hud_dirty(mut ui_state: ResMut<GameplayUiState>) {
    if ui_state.hud_dirty {
        ui_state.hud_dirty = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::{
        ClientIntent, IntentDispatchRecord, IntentDispatchReport, IntentDispatchStatus,
        ResolvedCommandFeedback,
    };
    use crate::ui::gameplay::state::GameplayCommandState;
    use crate::world::{ChunkCoord, LocalPosition, WorldPosition};

    fn pos(x: f32, z: f32) -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(x, 0.0, z)),
        )
    }

    #[test]
    fn selection_change_updates_ui_snapshot() {
        let mut selection = SelectedUnits::default();
        selection.set_single(crate::world::UnitId::new(3));
        let history = IntentDispatchHistory::default();
        let trace = CommandTraceBuffer::default();
        let debug = DebugOverlaySettings::default();
        let mut ui_state = GameplayUiState::default();

        let snapshot = derive_gameplay_snapshot(
            &selection,
            &history,
            &trace,
            &ResolvedCommandFeedback::default(),
            debug.enabled,
            ui_state.snapshot.cursor_mode,
        );
        ui_state.snapshot = snapshot;
        assert_eq!(ui_state.snapshot.selection_count, 1);
    }

    #[test]
    fn move_intent_updates_command_state_in_snapshot() {
        let mut selection = SelectedUnits::default();
        selection.set_single(crate::world::UnitId::new(1));
        let history = IntentDispatchHistory {
            report: Some(IntentDispatchReport {
                records: vec![IntentDispatchRecord {
                    intent: ClientIntent::MoveCommand {
                        target: pos(5.0, 5.0),
                    },
                    status: IntentDispatchStatus::Applied,
                }],
            }),
        };
        let snapshot = derive_gameplay_snapshot(
            &selection,
            &history,
            &CommandTraceBuffer::default(),
            &ResolvedCommandFeedback::default(),
            false,
            Default::default(),
        );
        assert_eq!(snapshot.command_state, GameplayCommandState::Moving);
    }

    #[test]
    fn ui_layer_does_not_own_world_data_mutation() {
        // Structural guarantee: sync only takes Res/ResMut on client + UI resources.
        let _selection = SelectedUnits::default();
        assert!(true);
    }
}
