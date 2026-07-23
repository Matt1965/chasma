//! Dev mode plugin — runtime authoring layer (ADR-043/044).

mod animation_focus;
mod animation_panel;
mod asset_sizing;
mod catalog_browser;
mod catalog_cache;
mod debug_controls;
mod dev_mode;
mod gizmo;
mod history;
mod input;
mod inspector;
mod inventory_tools;
mod items_browser;
mod lighting_panel;
mod panel;
mod pile_harness;
mod scenes;
mod spawn_tools;
mod terrain_field;
mod time_of_day_panel;
mod tools;
mod treasury_harness;

#[cfg(test)]
mod polish_tests;

pub use catalog_browser::{CatalogBrowserEntry, filter_catalog_entries};
pub use catalog_cache::{
    CatalogBrowseIndex, CatalogFilterCache, DevSearchDebounce, browse_catalog_entries,
};
pub use debug_controls::{apply_dev_debug_flags, dev_flags_from_overlay, sync_dev_debug_controls};
pub use dev_mode::{
    DefinitionId, DevDebugFlags, DevInventoryEndpoint, DevInventoryToolState, DevModeInputGate,
    DevModeState, DevTab, DevTextFieldFocus, ItemsBrowserSubtab, SpawnMode,
};
pub use gizmo::{
    DevTool, DevToolState, DevTransformPreview, GizmoCoordinateSpace, SelectedWorldObject,
    TransformEditState,
};
pub use history::{DevSpawnHistory, DevSpawnRecord};
pub use input::{
    DevPanelHoverState, DevPanelRoot, DevPanelUi, cancel_dev_placement, dev_mode_keyboard_input,
    handle_dev_spawn_click, handle_dev_tool_cancel_input, reset_dev_input_gate,
    sync_dev_gameplay_input_block, update_dev_panel_hover_state, update_dev_preview_anchor,
};
pub use inspector::{
    BlueprintEditDrag, BlueprintEditSelection, BlueprintEditTool, BlueprintInspectionState,
    WorldInspectorState, capture_unit_inspector_snapshot,
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

use catalog_cache::{sync_catalog_browse_index, tick_dev_search_debounce};
use gizmo::{
    apply_building_transform_preview, apply_doodad_transform_preview, draw_transform_gizmo,
    handle_gizmo_keyboard, handle_gizmo_mouse, sync_gizmo_target,
};
use inspector::{
    handle_blueprint_edit_input, handle_blueprint_inspection_input, handle_building_dev_actions,
    handle_doodad_transform_hotkeys, handle_inspector_input, refresh_inspector_snapshot,
    sync_inspector_panel,
};
use panel::{
    handle_dev_panel_ui_interaction, setup_dev_panel, sync_dev_panel_button_styles,
    sync_dev_panel_content, sync_dev_panel_section_visibility, sync_dev_panel_tab_sections,
    sync_dev_panel_visibility, sync_dev_search_box_style, sync_dev_simulation_status,
};
use terrain_field::{
    draw_dev_terrain_field_gizmos, handle_terrain_field_buttons, setup_dev_terrain_field_state,
    sync_dev_terrain_field_panel, sync_terrain_field_button_styles,
    sync_terrain_field_section_visibility, update_dev_terrain_field_probe,
};

use bevy::prelude::*;

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
            .init_resource::<inspector::WorldInspectorState>()
            .init_resource::<inspector::BlueprintInspectionState>()
            .init_resource::<gizmo::DevToolState>()
            .init_resource::<gizmo::TransformEditState>()
            .init_resource::<DevPanelHoverState>()
            .init_resource::<tools::DevPlacementPreview>()
            .init_resource::<tools::DevPlacementPreviewScratch>()
            .init_resource::<DevPreviewAnchor>()
            .init_resource::<scenes::DevSceneRegistry>()
            .add_systems(
                Startup,
                (
                    setup_dev_panel,
                    scenes::init_dev_scene_registry,
                    setup_dev_terrain_field_state,
                ),
            )
            .add_systems(
                Update,
                (
                    reset_dev_input_gate,
                    dev_mode_keyboard_input,
                    handle_dev_tool_cancel_input,
                    tick_dev_search_debounce,
                    sync_catalog_browse_index,
                    update_dev_panel_hover_state,
                    update_dev_preview_anchor,
                    tools::update_dev_placement_preview,
                    sync_dev_panel_visibility,
                    sync_dev_panel_content,
                    sync_dev_simulation_status,
                    sync_dev_panel_button_styles,
                    sync_dev_panel_section_visibility,
                    sync_dev_panel_tab_sections,
                    time_of_day_panel::sync_time_of_day_section_visibility,
                    time_of_day_panel::sync_time_of_day_panel_text,
                    lighting_panel::sync_lighting_section_visibility,
                    lighting_panel::sync_lighting_panel_text,
                    animation_panel::sync_dev_animation_panel,
                    animation_focus::sync_animation_presentation_focus,
                )
                    .chain()
                    .in_set(DevModeInputSystems),
            )
            .add_systems(
                Update,
                (
                    sync_terrain_field_section_visibility,
                    sync_terrain_field_button_styles,
                )
                    .in_set(DevModeInputSystems),
            )
            .add_systems(
                Update,
                sync_dev_search_box_style.in_set(DevModeInputSystems),
            )
            .add_systems(
                Update,
                sync_inspector_panel.in_set(DevModeInputSystems),
            )
            .add_systems(
                Update,
                refresh_inspector_snapshot
                    .after(handle_inspector_input)
                    .after(sync_gizmo_target)
                    .in_set(DevModeInputSystems),
            )
            .add_systems(
                Update,
                (
                    handle_dev_panel_ui_interaction,
                    time_of_day_panel::handle_time_of_day_buttons,
                    lighting_panel::handle_lighting_tune_buttons,
                )
                    .after(sync_dev_panel_tab_sections)
                    .in_set(DevModeInputSystems),
            )
            .add_systems(
                Update,
                handle_terrain_field_buttons
                    .after(sync_dev_panel_tab_sections)
                    .in_set(DevModeInputSystems),
            )
            .add_systems(Update, sync_dev_debug_controls.in_set(DevModeInputSystems))
            .add_systems(
                Update,
                handle_gizmo_keyboard.in_set(DevModeInputSystems),
            )
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
                    .after(sync_dev_panel_tab_sections)
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
                handle_building_dev_actions.in_set(DevModeInputSystems),
            )
            .add_systems(
                Update,
                handle_blueprint_inspection_input.in_set(DevModeInputSystems),
            )
            .add_systems(
                Update,
                handle_blueprint_edit_input
                    .after(handle_blueprint_inspection_input)
                    .in_set(DevModeInputSystems),
            )
            .add_systems(
                Update,
                handle_doodad_transform_hotkeys.in_set(DevModeInputSystems),
            )
            .add_systems(
                Update,
                inventory_tools::handle_dev_items_ground_click
                    .after(sync_dev_panel_tab_sections)
                    .before(handle_dev_spawn_click)
                    .in_set(DevModeInputSystems),
            )
            .add_systems(
                Update,
                handle_dev_spawn_click
                    .after(sync_dev_panel_tab_sections)
                    .in_set(DevModeInputSystems),
            )
            .add_systems(
                Update,
                sync_dev_gameplay_input_block
                    .after(handle_dev_spawn_click)
                    .in_set(DevModeInputSystems),
            )
            .add_systems(
                Update,
                (
                    pile_harness::handle_pile_harness_keyboard,
                    treasury_harness::handle_treasury_harness_keyboard,
                    inventory_tools::handle_dev_items_keyboard_system,
                    inventory_tools::handle_dev_items_buttons,
                    inventory_tools::sync_items_section_visibility,
                    inventory_tools::sync_item_quantity_controls,
                    inventory_tools::sync_items_panel_text,
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
                )
                    .chain()
                    .in_set(DevModePresentationSystems),
            );
    }
}
