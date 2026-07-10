//! Gameplay UI layer — SC2/Kenshi hybrid player HUD (ADR-040, ADR-050 P-UI1).

pub mod combat_display;
mod command_feedback;
mod command_panel;
mod cursor_feedback;
mod input_gate;
mod layout;
mod player_hud_state;
mod plugin;
mod selected_unit_panel;
mod selection_ui;
mod squad_panel;
mod state;
mod styles;

pub use command_feedback::{
    sync_move_command_indicator, tick_move_command_indicator, MoveCommandFeedback,
};
pub use command_panel::{command_button_enabled, HudCommandButton};
pub use cursor_feedback::{
    sample_gameplay_cursor_context, GameplayCursorPresentation, GameplayHoveredUnit,
};
pub use input_gate::{
    gameplay_input_blocked_by_hud, update_player_hud_hover_state, PlayerHudHoverState,
};
pub use layout::{GameplayHudRoot, PlayerHudUi, setup_player_hud_layout};
pub use player_hud_state::{
    primary_selected_unit, sync_primary_selection, PlayerHudState, SquadFilterMode,
};
pub use plugin::{GameplayUiPlugin, GameplayUiSystems};
pub use selected_unit_panel::{
    build_selected_unit_snapshot, format_single_unit_lines, format_unit_detail_lines,
    unit_state_label, SelectedUnitPanelSnapshot,
};
pub use selection_ui::{clear_gameplay_hud_dirty, sync_gameplay_ui_state};
pub use squad_panel::{squad_display_name, squad_panel_unit_ids};
pub use state::{
    command_state_display, derive_command_state, derive_cursor_mode, derive_gameplay_snapshot,
    CommandHoverContext, GameplayCommandState, GameplayCursorMode, GameplayUiSnapshot,
    GameplayUiState,
};
