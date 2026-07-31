//! World environment authoring UI (Slice 11).

mod fields;
mod panel;
mod state;
mod systems;

#[cfg(test)]
mod tests;

pub use panel::spawn_environment_controls;
pub use state::{DevWorldEnvironmentSection, WorldEnvironmentUiState};
pub use systems::{
    focus_world_environment_numeric, handle_world_cycle_toggles, handle_world_environment_actions,
    handle_world_environment_numeric_keyboard, handle_world_skybox_selection,
    handle_world_slider_interaction, handle_world_time_presets, sync_world_environment_confirm_bar,
    sync_world_environment_panel, sync_world_environment_sliders, sync_world_environment_toggles,
    sync_world_skybox_buttons, tick_world_environment_status,
};
