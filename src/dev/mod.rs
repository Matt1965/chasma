//! Dev mode plugin — runtime authoring layer (ADR-043/044).

mod animation_focus;
mod animation_panel;
mod asset_sizing;
mod catalog;
mod catalog_browser;
mod catalog_cache;
mod debug_controls;
mod debug_window;
mod dev_mode;
mod fields_window;
mod gizmo;
mod history;
mod hotkeys;
mod input;
mod inspector;
pub(crate) mod inventory_tools;
mod items_browser;
mod navigation_editor;
mod panel;
mod pile_harness;
mod save_window;
mod scenes;
mod selected_object;
mod settlement_placement;
mod spawn_tools;
mod terrain_field;
mod tools;
mod tooltip;
mod treasury_harness;
mod widgets;
mod window;
mod world_environment;
mod world_window;

#[cfg(test)]
mod query_safety_tests;

#[cfg(test)]
mod polish_tests;

#[cfg(test)]
mod workspace_tests;

pub use catalog_browser::{CatalogBrowserEntry, filter_catalog_entries};
pub use catalog_cache::{
    CatalogBrowseIndex, CatalogFilterCache, DevSearchDebounce, browse_catalog_entries,
};
pub use debug_controls::{apply_dev_debug_flags, dev_flags_from_overlay, sync_dev_debug_controls};
pub use debug_window::{
    handle_debug_toggle_buttons, setup_debug_window_panel, sync_debug_panel_button_styles,
    sync_debug_panel_content,
};
pub use dev_mode::{
    DefinitionId, DevDebugFlags, DevInventoryEndpoint, DevInventoryToolState, DevModeInputGate,
    DevModeState, DevTab, DevTextFieldFocus, SpawnMode,
};
pub use fields_window::{setup_fields_window_panel, sync_dev_fields_panel_visibility};
pub use gizmo::{
    DevTool, DevToolState, DevTransformPreview, GizmoCoordinateSpace, SelectedWorldObject,
    TransformEditState,
};
pub use history::{DevSpawnHistory, DevSpawnRecord};
pub use hotkeys::{
    DEV_GIZMO_COORDINATE_SPACE, DEV_HOTKEY_REGISTRY, DevHotkeyEntry, DevShortcutLifecycle,
    DevShortcutSuppressionCtx, cancel_blueprint_edit_drag, cancel_blueprint_pending_confirmation,
    cancel_blueprint_variant_draft, dev_building_transform_edit_options,
    dev_doodad_transform_edit_options, dev_shortcuts_suppressed, exit_blueprint_inspection_from_ui,
    request_exit_blueprint_edit,
};
pub use input::{
    DevPanelHoverState, DevPanelRoot, DevPanelUi, cancel_dev_placement, dev_mode_keyboard_input,
    handle_dev_right_click_input, handle_dev_spawn_click, reset_dev_input_gate,
    sync_dev_gameplay_input_block, update_dev_preview_anchor,
};
pub use inspector::{
    BlueprintEditDrag, BlueprintEditSelection, BlueprintEditTool, BlueprintInspectionState,
    WorldInspectorState, blueprint_local_to_world, capture_unit_inspector_snapshot,
};
pub use navigation_editor::{
    BlueprintInspectionScenePresentation, NavigationEditorBlockedAction, NavigationEditorUiState,
    NavigationGenerationDiagnostics, apply_navigation_editor_disclosure_hints,
    guard_dirty_navigation_selection, handle_navigation_editor_actions,
    handle_navigation_editor_opacity_slider, handle_open_navigation_editor_buttons,
    navigation_editor_owns_session, open_navigation_editor, setup_navigation_editor_panel,
    spawn_open_navigation_editor_button, sync_blueprint_inspection_scene_visibility,
    sync_navigation_editor_action_buttons, sync_navigation_editor_disclosure_state,
    sync_navigation_editor_opacity_slider, sync_navigation_editor_overlay_status,
    sync_navigation_editor_panel, sync_navigation_editor_panel_content,
    sync_navigation_editor_responsive_layout, sync_navigation_editor_section_visibility,
    sync_navigation_editor_toast, sync_navigation_editor_window_layout,
    sync_open_navigation_editor_buttons,
};
pub use save_window::{
    handle_save_window_interaction, setup_save_window_panel, sync_dev_save_panel_visibility,
    sync_save_window_content, sync_save_window_name_field_style,
};
pub use scenes::{
    DEV_SCENES_DIR, SceneApplyReport, SceneCaptureContext, SceneDebugFlagsSnapshot, SceneRegistry,
    SceneRegistryEntry, apply_scene, capture_scene, clear_world_entities,
};
pub use spawn_tools::{
    DevSpawnOutcome, dev_spawn_position_from_terrain_click, spawn_by_mode_at_position,
    spawn_selected_at_position,
};
pub use terrain_field::DevTerrainFieldState;
pub use tools::{
    BrushMode, BrushSettings, DevPlacementPreview, DevPreviewAnchor, MAX_BRUSH_SPAWN_COUNT,
    PlacementRules,
};
pub use tooltip::{
    DevTooltipContent, DevTooltipHoverZone, DevTooltipState, DevTooltipTarget,
    TOOLTIP_HOVER_DELAY_SECS, dismiss_dev_tooltip, setup_dev_tooltip,
    sync_dev_tooltip_presentation,
};
pub use window::{DevWindowId, DevWindowInteractionState, DevWindowRegistry, setup_dev_workspace};
pub use world_window::{setup_world_window_panel, sync_dev_world_panel_visibility};

use catalog::{sync_dev_catalog_chrome, track_catalog_tab_selection};
use catalog_cache::{sync_catalog_browse_index, tick_dev_search_debounce};
use fields_window::forensics::{
    fields_forensics_enabled, fields_forensics_post_startup, fields_forensics_update,
    fields_launcher_trace_after_click, fields_launcher_trace_after_collapsible,
    fields_launcher_trace_after_fields_visibility, fields_launcher_trace_after_presentation,
    fields_launcher_trace_before_click, fields_launcher_trace_post_layout,
    fields_launcher_trace_scripted,
};
use gizmo::{
    apply_building_transform_preview, apply_doodad_transform_preview, draw_transform_gizmo,
    handle_gizmo_keyboard, handle_gizmo_mouse, sync_gizmo_target,
};
use inspector::{
    handle_blueprint_edit_input, handle_blueprint_inspection_input,
    handle_building_dev_action_buttons, handle_building_production_repeat_button,
    handle_inspector_input, handle_production_operation_buttons, refresh_inspector_snapshot,
    sync_inspector_on_selection_revision,
};
use panel::{
    handle_dev_panel_ui_interaction, setup_dev_panel, sync_dev_catalog_panel_visibility,
    sync_dev_panel_button_styles, sync_dev_panel_content, sync_dev_search_box_style,
    sync_dev_simulation_status,
};
use selected_object::{
    BuildingActionUiCache, SelectedObjectUiState, handle_selected_object_actions,
    setup_selected_object_panel, sync_building_dev_action_sections, sync_selected_object_panel,
};
use terrain_field::{
    draw_dev_terrain_field_gizmos, handle_terrain_field_buttons, setup_dev_terrain_field_state,
    sync_dev_terrain_field_panel, sync_terrain_field_button_styles, update_dev_terrain_field_probe,
};
use window::{
    apply_dev_window_input_gate, focus_dev_window_on_panel_press, focus_dev_window_on_ui_press,
    handle_dev_mode_window_lifecycle, handle_dev_window_pointer, sync_dev_panel_hover_from_windows,
    sync_dev_window_computed_sizes, sync_dev_window_presentation, sync_dev_window_viewport,
    update_dev_window_interaction_state,
};

use bevy::prelude::*;
use bevy::ui::UiSystems;

/// Dev mode input and panel systems (before intent collection).
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct DevModeInputSystems;

/// Dev mode presentation after dispatch trace flush.
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct DevModePresentationSystems;

/// Registers dev mode resources, UI, and input (requires `dev` feature).
pub struct DevModePlugin;

impl Plugin for DevModePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DevModeState>()
            .init_resource::<DevModeInputGate>()
            .init_resource::<CatalogBrowseIndex>()
            .init_resource::<CatalogFilterCache>()
            .init_resource::<DevSearchDebounce>()
            .init_resource::<DevWindowRegistry>()
            .init_resource::<DevWindowInteractionState>()
            .init_resource::<inspector::WorldInspectorState>()
            .init_resource::<SelectedObjectUiState>()
            .init_resource::<BuildingActionUiCache>()
            .init_resource::<NavigationEditorUiState>()
            .init_resource::<inspector::BlueprintInspectionState>()
            .init_resource::<BlueprintInspectionScenePresentation>()
            .init_resource::<tooltip::DevTooltipState>()
            .init_resource::<widgets::DevCollapsibleState>()
            .init_resource::<world_environment::WorldEnvironmentUiState>()
            .init_resource::<widgets::DevSliderDragState>()
            .init_resource::<gizmo::DevToolState>()
            .init_resource::<gizmo::TransformEditState>()
            .init_resource::<DevPanelHoverState>()
            .init_resource::<tools::DevPlacementPreview>()
            .init_resource::<tools::DevPlacementPreviewScratch>()
            .init_resource::<DevPreviewAnchor>()
            .init_resource::<scenes::DevSceneRegistry>()
            .init_resource::<settlement_placement::SettlementPlacementPreview>()
            .init_resource::<settlement_placement::SettlementPlacementRejectionFeedbacks>()
            .init_resource::<settlement_placement::SettlementPlacementRejectionLabelIndex>()
            .add_systems(
                Startup,
                (
                    setup_dev_workspace,
                    setup_dev_panel,
                    setup_save_window_panel,
                    setup_selected_object_panel,
                    setup_navigation_editor_panel,
                    setup_debug_window_panel,
                    setup_world_window_panel,
                    setup_fields_window_panel,
                    setup_dev_tooltip,
                    scenes::init_dev_scene_registry,
                    setup_dev_terrain_field_state,
                )
                    .chain(),
            );
        if fields_forensics_enabled() {
            app.add_systems(PostStartup, fields_forensics_post_startup);
            app.add_systems(
                Update,
                (
                    fields_launcher_trace_scripted,
                    fields_launcher_trace_before_click,
                )
                    .before(handle_dev_window_pointer),
            );
            app.add_systems(
                Update,
                fields_launcher_trace_after_click.after(handle_dev_window_pointer),
            );
            app.add_systems(
                Update,
                fields_launcher_trace_after_presentation.after(sync_dev_window_presentation),
            );
            app.add_systems(
                Update,
                fields_launcher_trace_after_fields_visibility
                    .after(sync_dev_fields_panel_visibility),
            );
            app.add_systems(
                Update,
                fields_launcher_trace_after_collapsible
                    .after(crate::dev::widgets::sync_collapsible_sections),
            );
            app.add_systems(
                PostUpdate,
                fields_launcher_trace_post_layout.after(UiSystems::Layout),
            );
            app.add_systems(Update, fields_forensics_update);
        }
        app.add_systems(
            Update,
            (
                (
                    reset_dev_input_gate,
                    sync_dev_window_viewport,
                    dev_mode_keyboard_input,
                    handle_dev_mode_window_lifecycle,
                    tick_dev_search_debounce,
                    sync_catalog_browse_index,
                    update_dev_window_interaction_state,
                    sync_dev_panel_hover_from_windows,
                    handle_dev_window_pointer,
                    focus_dev_window_on_ui_press,
                    focus_dev_window_on_panel_press,
                    sync_dev_window_presentation,
                    sync_dev_world_panel_visibility,
                    sync_dev_fields_panel_visibility,
                    sync_dev_window_computed_sizes,
                    update_dev_preview_anchor,
                    tools::update_dev_placement_preview,
                    sync_dev_panel_content,
                    sync_dev_simulation_status,
                )
                    .chain(),
                (
                    sync_dev_catalog_chrome,
                    track_catalog_tab_selection,
                    sync_dev_save_panel_visibility,
                    sync_save_window_content,
                    sync_save_window_name_field_style,
                    sync_dev_panel_button_styles,
                    widgets::sync_dev_button_chrome,
                    widgets::sync_status_line_color,
                    sync_dev_catalog_panel_visibility,
                    sync_debug_panel_content,
                    sync_debug_panel_button_styles,
                    sync_terrain_field_button_styles,
                    widgets::sync_collapsible_sections,
                    widgets::handle_collapsible_toggles,
                    animation_panel::sync_dev_animation_panel,
                    animation_focus::sync_animation_presentation_focus,
                )
                    .chain(),
                (
                    world_environment::sync_world_environment_panel,
                    world_environment::sync_world_environment_sliders,
                    world_environment::sync_world_water_level_slider,
                    world_environment::sync_world_water_enabled_toggle,
                    world_environment::sync_world_environment_toggles,
                    world_environment::sync_world_environment_confirm_bar,
                    world_environment::tick_world_environment_status,
                )
                    .chain(),
            )
                .chain()
                .in_set(DevModeInputSystems),
        )
        .add_systems(
            Update,
            widgets::tick_dev_button_activation_flashes.in_set(DevModeInputSystems),
        )
        .add_systems(
            Update,
            sync_dev_search_box_style.in_set(DevModeInputSystems),
        )
        .add_systems(
            Update,
            guard_dirty_navigation_selection
                .after(sync_inspector_on_selection_revision)
                .in_set(DevModeInputSystems),
        )
        .add_systems(
            Update,
            sync_blueprint_inspection_scene_visibility.in_set(DevModePresentationSystems),
        )
        .add_systems(
            Update,
            (
                apply_navigation_editor_disclosure_hints,
                sync_navigation_editor_window_layout,
                sync_navigation_editor_responsive_layout,
                sync_navigation_editor_overlay_status,
                sync_navigation_editor_panel_content,
                sync_navigation_editor_panel,
                sync_navigation_editor_action_buttons,
                sync_navigation_editor_section_visibility,
                sync_navigation_editor_disclosure_state,
                sync_navigation_editor_toast,
                sync_navigation_editor_opacity_slider,
                handle_navigation_editor_opacity_slider,
            )
                .chain()
                .in_set(DevModeInputSystems),
        )
        .add_systems(
            Update,
            sync_open_navigation_editor_buttons.in_set(DevModeInputSystems),
        )
        .add_systems(
            Update,
            handle_open_navigation_editor_buttons.in_set(DevModeInputSystems),
        )
        .add_systems(
            Update,
            handle_navigation_editor_actions.in_set(DevModeInputSystems),
        )
        .add_systems(
            Update,
            sync_dev_tooltip_presentation.in_set(DevModePresentationSystems),
        )
        .add_systems(
            Update,
            sync_selected_object_panel
                .after(handle_blueprint_inspection_input)
                .in_set(DevModeInputSystems),
        )
        .add_systems(
            Update,
            sync_building_dev_action_sections
                .after(refresh_inspector_snapshot)
                .in_set(DevModeInputSystems),
        )
        .add_systems(
            Update,
            handle_selected_object_actions
                .after(handle_gizmo_keyboard)
                .in_set(DevModeInputSystems),
        )
        .add_systems(
            Update,
            sync_inspector_on_selection_revision.in_set(DevModeInputSystems),
        )
        .add_systems(
            Update,
            handle_dev_right_click_input
                .after(sync_dev_panel_hover_from_windows)
                .before(handle_dev_spawn_click)
                .in_set(DevModeInputSystems),
        )
        .add_systems(
            Update,
            refresh_inspector_snapshot
                .after(sync_inspector_on_selection_revision)
                .after(handle_inspector_input)
                .after(sync_gizmo_target)
                .in_set(DevModeInputSystems),
        )
        .add_systems(
            Update,
            (
                handle_dev_panel_ui_interaction,
                handle_save_window_interaction,
                handle_debug_toggle_buttons,
                world_environment::handle_world_environment_actions,
                world_environment::handle_world_cycle_toggles,
                world_environment::handle_world_time_presets,
                world_environment::handle_world_slider_interaction,
                world_environment::handle_world_water_level_slider,
                world_environment::handle_world_water_enabled_toggle,
                world_environment::handle_world_environment_numeric_keyboard,
                world_environment::focus_world_environment_numeric,
            )
                .after(sync_save_window_content)
                .in_set(DevModeInputSystems),
        )
        .add_systems(
            Update,
            handle_terrain_field_buttons
                .after(sync_save_window_content)
                .in_set(DevModeInputSystems),
        )
        .add_systems(Update, sync_dev_debug_controls.in_set(DevModeInputSystems))
        .add_systems(Update, handle_gizmo_keyboard.in_set(DevModeInputSystems))
        .add_systems(
            Update,
            handle_gizmo_mouse
                .after(handle_gizmo_keyboard)
                .before(handle_inspector_input)
                .in_set(DevModeInputSystems),
        )
        .add_systems(
            Update,
            handle_inspector_input
                .after(sync_save_window_content)
                .before(handle_dev_spawn_click)
                .in_set(DevModeInputSystems),
        )
        // After inspector so a fresh doodad/building pick arms gizmos the same frame
        // without letting handle_gizmo_mouse treat that click as a TranslateXZ grab.
        .add_systems(
            Update,
            sync_gizmo_target
                .after(handle_inspector_input)
                .before(handle_dev_spawn_click)
                .in_set(DevModeInputSystems),
        )
        .add_systems(
            Update,
            (
                handle_building_dev_action_buttons,
                handle_production_operation_buttons,
            )
                .in_set(DevModeInputSystems),
        )
        .add_systems(
            Update,
            handle_building_production_repeat_button.in_set(DevModeInputSystems),
        )
        .add_systems(
            Update,
            handle_blueprint_inspection_input.in_set(DevModeInputSystems),
        )
        .add_systems(
            Update,
            handle_blueprint_edit_input
                .after(handle_blueprint_inspection_input)
                .before(handle_inspector_input)
                .before(sync_dev_gameplay_input_block)
                .in_set(DevModeInputSystems),
        )
        .add_systems(
            Update,
            inventory_tools::handle_dev_items_ground_click
                .after(sync_save_window_content)
                .before(inventory_tools::handle_dev_held_item_input)
                .in_set(DevModeInputSystems),
        )
        .add_systems(
            Update,
            inventory_tools::handle_dev_held_item_input
                .after(inventory_tools::handle_dev_items_ground_click)
                .before(handle_dev_spawn_click)
                .in_set(DevModeInputSystems),
        )
        .add_systems(
            Update,
            handle_dev_spawn_click
                .after(sync_save_window_content)
                .in_set(DevModeInputSystems),
        )
        .add_systems(
            Update,
            sync_dev_gameplay_input_block
                .after(handle_dev_spawn_click)
                .after(apply_dev_window_input_gate)
                .in_set(DevModeInputSystems),
        )
        .add_systems(
            Update,
            apply_dev_window_input_gate
                .after(handle_dev_window_pointer)
                .before(sync_dev_gameplay_input_block)
                .in_set(DevModeInputSystems),
        )
        .add_systems(
            Update,
            (
                inventory_tools::sync_items_section_visibility,
                inventory_tools::handle_dev_items_buttons,
                inventory_tools::sync_item_quantity_controls,
                inventory_tools::sync_items_panel_text,
                settlement_placement::handle_settlement_placement_button,
                settlement_placement::handle_unit_assignment_button,
                settlement_placement::handle_settlement_placement_click,
                settlement_placement::handle_unit_assignment_click,
                settlement_placement::sync_settlement_placement_button_active,
                settlement_placement::sync_unit_assignment_button_active,
                settlement_placement::update_settlement_placement_preview,
                settlement_placement::clear_settlement_placement_preview_when_disarmed,
                settlement_placement::sync_settlement_placement_rejection_labels,
            )
                .in_set(DevModeInputSystems),
        )
        .add_systems(
            Update,
            (
                apply_doodad_transform_preview,
                apply_building_transform_preview,
                draw_transform_gizmo,
                sync_dev_terrain_field_panel,
                update_dev_terrain_field_probe,
                draw_dev_terrain_field_gizmos,
                settlement_placement::draw_settlement_placement_preview,
                settlement_placement::billboard_settlement_placement_rejection_labels,
                inventory_tools::sync_dev_held_item_screen_ghost,
                inventory_tools::sync_dev_held_item_world_ghost,
            )
                .chain()
                .in_set(DevModePresentationSystems),
        );
    }
}
