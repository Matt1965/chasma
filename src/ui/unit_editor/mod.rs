//! Reusable Unit Editor and isolated 3D preview (CG3).

mod actions;
mod commit;
mod controls;
mod draft;
mod events;
mod plugin;
mod preview_animation;
mod preview_spawn;
mod preview_studio;
mod screen;
mod session;

#[cfg(test)]
mod tests;

pub use actions::open_unit_editor_for_live_unit;
pub use draft::{EquipmentPreviewLoadout, UnitAppearanceDraft};
pub use events::OpenUnitEditorRequest;
pub use plugin::{UnitEditorPlugin, UnitEditorSystems};
pub use controls::{handle_unit_editor_sliders, sync_unit_editor_control_values};
pub use preview_animation::{
    discover_preview_animation_players, install_preview_animation_graph, sync_preview_idle_animation,
};
pub use preview_spawn::unit_editor_session_owns_cg3_preview_actor;
pub use preview_studio::{
    UnitEditorPreviewCamera, UnitEditorPreviewImage, cleanup_unit_editor_preview_studio,
    setup_unit_editor_preview_studio, update_unit_editor_preview_framing,
};
pub use screen::sync_unit_editor_error_text;
pub use screen::{
    UnitEditorAction, UnitEditorActionButton, UnitEditorControlsHost, spawn_unit_editor_controls_panel,
};
pub use session::{UnitEditorMode, UnitEditorSession};
