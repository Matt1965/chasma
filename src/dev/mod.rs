//! Dev mode plugin — runtime authoring layer (ADR-043/044).

mod catalog_browser;
mod catalog_cache;
mod debug_controls;
mod dev_mode;
mod history;
mod input;
mod inspector;
mod panel;
mod scenes;
mod spawn_tools;
mod tools;

#[cfg(test)]
mod polish_tests;

pub use catalog_browser::{filter_catalog_entries, CatalogBrowserEntry};
pub use catalog_cache::{
    browse_catalog_entries, CatalogBrowseIndex, CatalogFilterCache, DevSearchDebounce,
};
pub use history::{DevSpawnHistory, DevSpawnRecord};
pub use inspector::{capture_unit_inspector_snapshot, WorldInspectorState};
pub use debug_controls::{apply_dev_debug_flags, dev_flags_from_overlay, sync_dev_debug_controls};
pub use dev_mode::{
    DefinitionId, DevDebugFlags, DevModeInputGate, DevModeState, DevTab, SpawnMode,
};
pub use input::{
    dev_mode_keyboard_input, handle_dev_spawn_click, reset_dev_input_gate,
    update_dev_panel_hover_state, update_dev_preview_anchor, DevPanelHoverState, DevPanelRoot,
    DevPanelUi,
};
pub use spawn_tools::{
    dev_spawn_position_from_terrain_click, spawn_by_mode_at_position, spawn_selected_at_position,
    DevSpawnOutcome,
};
pub use scenes::{
    apply_scene, capture_scene, clear_world_entities, SceneApplyReport, SceneCaptureContext,
    SceneDebugFlagsSnapshot, SceneRegistry, SceneRegistryEntry, DEV_SCENES_DIR,
};
pub use tools::{
    BrushMode, BrushSettings, PlacementRules, DevPlacementPreview, DevPreviewAnchor,
    MAX_BRUSH_SPAWN_COUNT,
};

use catalog_cache::{sync_catalog_browse_index, tick_dev_search_debounce};
use inspector::{handle_inspector_input, refresh_inspector_snapshot, sync_inspector_panel};
use panel::{
    handle_dev_panel_ui_interaction, setup_dev_panel, sync_dev_panel_content,
    sync_dev_panel_section_visibility, sync_dev_panel_tab_sections, sync_dev_panel_visibility,
    sync_dev_simulation_status, sync_dev_panel_button_styles,
};

use bevy::prelude::*;

use crate::player::PlayerControlSystems;

/// Dev mode authoring systems (F12 panel, catalog browser, spawn tools).
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct DevModeSystems;

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
            .init_resource::<DevPanelHoverState>()
            .init_resource::<tools::DevPlacementPreview>()
            .init_resource::<tools::DevPlacementPreviewScratch>()
            .init_resource::<DevPreviewAnchor>()
            .init_resource::<scenes::DevSceneRegistry>()
            .add_systems(Startup, (setup_dev_panel, scenes::init_dev_scene_registry))
            .configure_sets(Update, DevModeSystems.in_set(PlayerControlSystems))
            .add_systems(
                Update,
                (
                    reset_dev_input_gate,
                    dev_mode_keyboard_input,
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
                )
                    .chain()
                    .in_set(DevModeSystems),
            )
            .add_systems(
                Update,
                (sync_inspector_panel, refresh_inspector_snapshot)
                    .chain()
                    .in_set(DevModeSystems),
            )
            .add_systems(
                Update,
                (
                    handle_dev_panel_ui_interaction,
                    sync_dev_debug_controls,
                )
                    .chain()
                    .after(sync_dev_panel_tab_sections)
                    .in_set(DevModeSystems),
            )
            .add_systems(
                Update,
                handle_inspector_input
                    .after(sync_dev_panel_tab_sections)
                    .before(handle_dev_spawn_click)
                    .in_set(DevModeSystems),
            )
            .add_systems(
                Update,
                handle_dev_spawn_click
                    .after(sync_dev_panel_tab_sections)
                    .before(crate::client::collect_unit_input_intents)
                    .in_set(DevModeSystems),
            )
            .add_systems(
                Update,
                tools::draw_dev_placement_preview
                    .after(crate::debug::flush_intent_dispatch_trace)
                    .in_set(DevModeSystems),
            );
    }
}
