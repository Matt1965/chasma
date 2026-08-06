//! Building Navigation Editor window (Slice 7).

mod actions;
mod capabilities;
mod commands;
mod guard;
mod layout;
mod opacity;
mod panel;
mod scene_visibility;
mod selectors;
mod state;
mod sync_panel;
#[cfg(test)]
mod tests;

pub use actions::{
    handle_navigation_editor_actions, handle_navigation_editor_close_guard,
    handle_open_navigation_editor_buttons,
};
pub use commands::{navigation_editor_visible, open_navigation_editor};
pub use guard::guard_dirty_navigation_selection;
pub use opacity::{
    NAV_EDITOR_BUILDING_OPACITY_FIELD_ID, handle_navigation_editor_opacity_slider,
    sync_navigation_editor_opacity_slider,
};
pub use panel::{
    NavigationEditorAction, setup_navigation_editor_panel, spawn_open_navigation_editor_button,
    sync_navigation_editor_window_layout, sync_open_navigation_editor_buttons,
};
pub use scene_visibility::{
    BlueprintInspectionScenePresentation, sync_blueprint_inspection_scene_visibility,
};
pub use state::{
    DEFAULT_NAV_EDITOR_BUILDING_OPACITY, NavigationEditorBlockedAction, NavigationEditorUiState,
    NavigationGenerationDiagnostics, navigation_editor_owns_session,
};
pub use sync_panel::{
    apply_navigation_editor_disclosure_hints, infer_message_severity,
    sync_navigation_editor_action_buttons, sync_navigation_editor_disclosure_state,
    sync_navigation_editor_overlay_status, sync_navigation_editor_panel,
    sync_navigation_editor_panel_content, sync_navigation_editor_responsive_layout,
    sync_navigation_editor_section_visibility, sync_navigation_editor_toast,
};
