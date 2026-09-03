//! Gameplay UI plugin wiring (P-UI1).

use bevy::prelude::*;

use crate::player::GameplayPresentationSystems;

use super::build_mode::{
    BuildModeCursorAnchor, BuildModeState, clear_build_mode_terrain_overlay_on_exit,
    draw_build_mode_ghost, handle_build_catalog_clicks, handle_build_search_keyboard,
    spawn_build_catalog_panel, sync_build_catalog_contents, sync_build_catalog_visibility,
    sync_build_mode_ghost_scene, sync_build_mode_terrain_overlay, tint_build_mode_ghost_scene,
    update_build_mode_ghost,
};
use super::building_panel::{
    BuildingPanelState, handle_building_menu_close_button, handle_building_production_controls,
    reconcile_building_menu_panel, spawn_building_menu_panel, sync_building_menu_panel,
};
use super::command_feedback::{
    MoveCommandFeedback, sync_move_command_indicator, tick_move_command_indicator,
};
use super::command_panel::{
    handle_command_button_clicks, sync_command_panel_buttons, update_command_button_hover,
};
use super::cursor_feedback::{
    GameplayCursorPresentation, GameplayHoveredUnit, sample_gameplay_cursor_context,
};
use super::floating_window::{
    FloatingGameplayWindowRegistry, focus_floating_gameplay_window_on_ui_press,
    handle_floating_gameplay_window_pointer, measure_floating_gameplay_window_sizes,
    sync_floating_gameplay_window_presentation, sync_floating_gameplay_window_viewport,
};
use super::input_gate::{PlayerHudHoverState, update_player_hud_hover_state};
use super::inventory::{
    InventoryDragPreviewState, InventoryUiState, cleanup_inventory_drag_previews,
    collect_inventory_keyboard_input, collect_inventory_mouse_transfers,
    handle_inventory_drag_release, handle_inventory_entry_clicks, handle_inventory_panel_buttons,
    reconcile_inventory_ui_from_world, spawn_inventory_panel, sync_inventory_drag_ghost,
    sync_inventory_ground_preview, sync_inventory_panel_contents, sync_inventory_panel_visibility,
    update_inventory_drag_preview,
};
use super::layout::setup_player_hud_layout;
use super::player_hud_state::{PlayerHudState, sync_primary_selection};
use super::selected_unit_panel::sync_selected_unit_panel;
use super::selection_ui::{clear_gameplay_hud_dirty, sync_gameplay_ui_state};
use super::squad_panel::{handle_squad_entry_clicks, sync_squad_panel, update_squad_entry_hover};
use super::state::GameplayUiState;
#[cfg(feature = "dev")]
use super::terrain_analysis::{
    close_terrain_analysis_on_dev_exit, handle_terrain_analysis_clicks,
    handle_terrain_analysis_keyboard, populate_terrain_analysis_field_buttons,
    spawn_terrain_analysis_ui, sync_terrain_analysis_dev_diagnostics, sync_terrain_analysis_panel,
    update_terrain_analysis_cursor_readout,
};

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
            .init_resource::<InventoryUiState>()
            .init_resource::<InventoryDragPreviewState>()
            .insert_resource(PlayerHudState::new_visible())
            .init_resource::<BuildModeState>()
            .init_resource::<BuildModeCursorAnchor>()
            .init_resource::<BuildingPanelState>()
            .init_resource::<FloatingGameplayWindowRegistry>()
            .init_resource::<PlayerHudHoverState>()
            .init_resource::<GameplayCursorPresentation>()
            .init_resource::<GameplayHoveredUnit>()
            .init_resource::<MoveCommandFeedback>();
        app.add_systems(
            Startup,
            (
                setup_player_hud_layout,
                spawn_build_catalog_panel,
                spawn_inventory_panel,
                spawn_building_menu_panel,
            ),
        );
        #[cfg(feature = "dev")]
        app.add_systems(
            Startup,
            (
                spawn_terrain_analysis_ui,
                populate_terrain_analysis_field_buttons,
            ),
        );
        app.configure_sets(
            Update,
            (
                GameplayInputGateSystems,
                GameplayCommandInputSystems,
                GameplayUiSystems.in_set(GameplayPresentationSystems),
            ),
        )
        .add_systems(
            Update,
            (update_player_hud_hover_state, sync_player_hud_state)
                .chain()
                .in_set(GameplayInputGateSystems),
        )
        .add_systems(
            Update,
            sync_floating_gameplay_window_viewport.in_set(GameplayInputGateSystems),
        )
        .add_systems(
            Update,
            sample_gameplay_cursor_context.in_set(GameplayCommandInputSystems),
        )
        .add_systems(Update, sync_gameplay_ui_state.in_set(GameplayUiSystems))
        .add_systems(
            Update,
            (
                sync_selected_unit_panel,
                sync_building_menu_panel,
                reconcile_building_menu_panel,
                handle_building_menu_close_button,
                sync_inventory_panel_visibility,
                sync_floating_gameplay_window_presentation,
                measure_floating_gameplay_window_sizes,
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
                update_build_mode_ghost,
                sync_build_mode_terrain_overlay,
                clear_build_mode_terrain_overlay_on_exit,
                sync_build_mode_ghost_scene,
                tint_build_mode_ghost_scene,
                sync_build_catalog_visibility,
                sync_build_catalog_contents,
                draw_build_mode_ghost,
            )
                .chain()
                .in_set(GameplayUiSystems),
        )
        .add_systems(
            Update,
            (handle_build_catalog_clicks, handle_build_search_keyboard)
                .chain()
                .in_set(GameplayCommandInputSystems),
        )
        .add_systems(
            Update,
            (
                handle_squad_entry_clicks,
                handle_command_button_clicks,
                handle_building_production_controls,
                update_squad_entry_hover,
                update_command_button_hover,
            )
                .chain()
                .in_set(GameplayCommandInputSystems),
        )
        .add_systems(
            Update,
            (
                handle_floating_gameplay_window_pointer,
                focus_floating_gameplay_window_on_ui_press,
            )
                .chain()
                .in_set(GameplayCommandInputSystems),
        )
        .add_systems(
            Update,
            (
                update_inventory_drag_preview,
                handle_inventory_drag_release,
                handle_inventory_entry_clicks,
                collect_inventory_mouse_transfers,
                handle_inventory_panel_buttons,
                collect_inventory_keyboard_input,
                reconcile_inventory_ui_from_world,
                sync_inventory_panel_contents,
                sync_inventory_drag_ghost,
                sync_inventory_ground_preview,
                cleanup_inventory_drag_previews,
            )
                .chain()
                .after(handle_floating_gameplay_window_pointer)
                .in_set(GameplayCommandInputSystems),
        );
        #[cfg(feature = "dev")]
        app.add_systems(
            Update,
            (
                sync_terrain_analysis_panel,
                update_terrain_analysis_cursor_readout,
                close_terrain_analysis_on_dev_exit,
            )
                .chain()
                .in_set(GameplayUiSystems),
        )
        .add_systems(
            Update,
            (
                handle_terrain_analysis_clicks,
                handle_terrain_analysis_keyboard,
            )
                .chain()
                .in_set(GameplayCommandInputSystems),
        )
        .add_systems(
            Update,
            sync_terrain_analysis_dev_diagnostics.in_set(GameplayUiSystems),
        );
        app.add_systems(
            Update,
            crate::client::dispatch_inventory_intents
                .after(collect_inventory_keyboard_input)
                .in_set(GameplayCommandInputSystems),
        );
    }
}

fn sync_player_hud_state(
    selection: Res<crate::units::input::SelectedUnits>,
    mut hud: ResMut<PlayerHudState>,
) {
    sync_primary_selection(&mut hud, &selection);
}
