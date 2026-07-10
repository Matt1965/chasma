//! Gameplay UI plugin wiring (P-UI1).

use bevy::prelude::*;

use crate::player::GameplayPresentationSystems;

use super::command_feedback::{
    sync_move_command_indicator, tick_move_command_indicator, MoveCommandFeedback,
};
use super::command_panel::{
    handle_command_button_clicks, sync_command_panel_buttons, update_command_button_hover,
};
use super::cursor_feedback::{
    sample_gameplay_cursor_context, GameplayCursorPresentation, GameplayHoveredUnit,
};
use super::input_gate::{update_player_hud_hover_state, PlayerHudHoverState};
use super::layout::setup_player_hud_layout;
use super::player_hud_state::{sync_primary_selection, PlayerHudState};
use super::selected_unit_panel::sync_selected_unit_panel;
use super::selection_ui::{clear_gameplay_hud_dirty, sync_gameplay_ui_state};
use super::squad_panel::{
    handle_squad_entry_clicks, sync_squad_panel, update_squad_entry_hover,
};
use super::state::GameplayUiState;

/// HUD hover gate — must run before intent collection reads [`PlayerHudHoverState`].
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct GameplayInputGateSystems;

/// HUD command/squad clicks — after collect, before dispatch.
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct GameplayCommandInputSystems;

/// Post-dispatch HUD presentation sync.
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct GameplayUiSystems;

/// Registers gameplay UI resources and presentation systems.
pub struct GameplayUiPlugin;

impl Plugin for GameplayUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameplayUiState>()
            .insert_resource(PlayerHudState::new_visible())
            .init_resource::<PlayerHudHoverState>()
            .init_resource::<GameplayCursorPresentation>()
            .init_resource::<GameplayHoveredUnit>()
            .init_resource::<MoveCommandFeedback>()
            .add_systems(Startup, setup_player_hud_layout)
            .configure_sets(
                Update,
                (
                    GameplayInputGateSystems,
                    GameplayCommandInputSystems,
                    GameplayUiSystems.in_set(GameplayPresentationSystems),
                ),
            )
            .add_systems(
                Update,
                (
                    update_player_hud_hover_state,
                    sync_player_hud_state,
                )
                    .chain()
                    .in_set(GameplayInputGateSystems),
            )
            .add_systems(
                Update,
                sample_gameplay_cursor_context.in_set(GameplayCommandInputSystems),
            )
            .add_systems(
                Update,
                sync_gameplay_ui_state.in_set(GameplayUiSystems),
            )
            .add_systems(
                Update,
                (
                    sync_selected_unit_panel,
                    sync_squad_panel,
                    sync_command_panel_buttons,
                    clear_gameplay_hud_dirty,
                    sync_move_command_indicator,
                    tick_move_command_indicator,
                )
                    .chain()
                    .after(sync_gameplay_ui_state)
                    .in_set(GameplayUiSystems),
            )
            .add_systems(
                Update,
                (
                    handle_squad_entry_clicks,
                    handle_command_button_clicks,
                    update_squad_entry_hover,
                    update_command_button_hover,
                )
                    .chain()
                    .in_set(GameplayCommandInputSystems),
            );
    }
}

fn sync_player_hud_state(selection: Res<crate::units::input::SelectedUnits>, mut hud: ResMut<PlayerHudState>) {
    sync_primary_selection(&mut hud, &selection);
}
