//! Gameplay UI layer — SC2-style player-facing feedback (ADR-040 U-UI4).

mod command_feedback;
mod cursor_feedback;
mod hud;
mod selection_ui;
mod state;

pub use command_feedback::{
    sync_move_command_indicator, tick_move_command_indicator, MoveCommandFeedback,
};
pub use cursor_feedback::{sample_gameplay_cursor_context, GameplayCursorPresentation};
pub use hud::{setup_gameplay_hud, sync_gameplay_hud};
pub use selection_ui::{clear_gameplay_hud_dirty, sync_gameplay_ui_state};
pub use state::{
    command_state_display, derive_command_state, derive_cursor_mode, derive_gameplay_snapshot,
    CommandHoverContext, GameplayCommandState, GameplayCursorMode, GameplayUiSnapshot,
    GameplayUiState,
};

use bevy::prelude::*;

use crate::player::PlayerControlSystems;

/// Gameplay HUD and command feedback systems (player experience layer).
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct GameplayUiSystems;

/// Registers gameplay UI resources and presentation systems.
pub struct GameplayUiPlugin;

impl Plugin for GameplayUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameplayUiState>()
            .init_resource::<GameplayCursorPresentation>()
            .init_resource::<MoveCommandFeedback>()
            .add_systems(Startup, setup_gameplay_hud)
            .configure_sets(
                Update,
                GameplayUiSystems.in_set(PlayerControlSystems),
            )
            .add_systems(
                Update,
                sample_gameplay_cursor_context
                    .after(crate::client::collect_unit_input_intents)
                    .in_set(GameplayUiSystems),
            )
            .add_systems(
                Update,
                sync_gameplay_ui_state
                    .after(crate::debug::flush_intent_dispatch_trace)
                    .in_set(GameplayUiSystems),
            )
            .add_systems(
                Update,
                (
                    sync_gameplay_hud,
                    clear_gameplay_hud_dirty,
                    sync_move_command_indicator,
                    tick_move_command_indicator,
                )
                    .chain()
                    .after(sync_gameplay_ui_state)
                    .in_set(GameplayUiSystems),
            );
    }
}
