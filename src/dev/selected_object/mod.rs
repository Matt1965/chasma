//! Selected Object window — world-selection-driven inspection (Slice 5).

mod actions;
mod building_actions_sync;
mod building_actions_ui;
mod building_diagnostics;
mod building_ui_tests;
mod format;
mod panel;
mod state;

#[cfg(test)]
mod tests;

pub use panel::{setup_selected_object_panel, sync_selected_object_panel};
pub use state::SelectedObjectUiState;

pub(crate) use actions::handle_selected_object_actions;
pub(crate) use building_actions_sync::{BuildingActionUiCache, sync_building_dev_action_sections};
pub(crate) use panel::{
    DevSelectedObjectActionButton, DevSelectedObjectToggleButton, DevSelectedObjectUi,
};
