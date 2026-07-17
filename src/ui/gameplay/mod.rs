//! Gameplay UI layer — SC2/Kenshi hybrid player HUD (ADR-040, ADR-050 P-UI1).

pub mod build_mode;
pub mod building_selection;
pub mod combat_display;
mod command_feedback;
mod command_panel;
mod cursor_feedback;
mod input_gate;
mod inventory;
mod layout;
mod player_hud_state;
mod plugin;
mod selected_building_panel;
mod selected_unit_panel;
mod selection_ui;
mod squad_panel;
mod state;
mod styles;
pub mod terrain_analysis;

pub use build_mode::{
    BuildModeState, collect_build_mode_intents, draw_build_mode_ghost, handle_build_catalog_clicks,
    handle_build_search_keyboard, spawn_build_catalog_panel, sync_build_catalog_contents,
    sync_build_catalog_visibility, update_build_mode_ghost,
};
pub use building_selection::GameplayBuildingSelection;
pub use command_feedback::{
    MoveCommandFeedback, sync_move_command_indicator, tick_move_command_indicator,
};
pub use command_panel::{HudCommandButton, command_button_enabled};
pub use cursor_feedback::{
    GameplayCursorPresentation, GameplayHoveredUnit, sample_gameplay_cursor_context,
};
pub use input_gate::{
    PlayerHudHoverState, gameplay_input_blocked_by_hud, update_player_hud_hover_state,
};
pub use inventory::{
    InventoryUiError, InventoryUiState, collect_inventory_keyboard_input,
    collect_inventory_mouse_transfers, handle_inventory_entry_clicks,
    handle_inventory_panel_buttons, inventory_panel_blocks_world_input, spawn_inventory_panel,
    sync_inventory_panel_contents, sync_inventory_panel_visibility,
};
pub use layout::{GameplayHudRoot, PlayerHudUi, setup_player_hud_layout};
pub use player_hud_state::{
    PlayerHudState, SquadFilterMode, primary_selected_unit, sync_primary_selection,
};
pub use plugin::{
    GameplayCommandInputSystems, GameplayInputGateSystems, GameplayUiPlugin, GameplayUiSystems,
};
pub use selected_unit_panel::{
    SelectedUnitPanelSnapshot, build_selected_unit_snapshot, format_single_unit_lines,
    format_unit_detail_lines, unit_state_label,
};
pub use selection_ui::{clear_gameplay_hud_dirty, sync_gameplay_ui_state};
pub use squad_panel::{squad_display_name, squad_panel_unit_ids};
pub use state::{
    CommandHoverContext, GameplayCommandState, GameplayCursorMode, GameplayUiSnapshot,
    GameplayUiState, command_state_display, derive_command_state, derive_cursor_mode,
    derive_gameplay_snapshot,
};
pub use terrain_analysis::TerrainAnalysisToggleButton;
