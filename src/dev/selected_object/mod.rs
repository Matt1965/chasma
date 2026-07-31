//! Selected Object window — world-selection-driven inspection (Slice 5).

mod actions;
mod building_actions_ui;
mod format;
mod panel;
mod state;

#[cfg(test)]
mod tests;

pub use panel::{setup_selected_object_panel, sync_selected_object_panel};
pub use state::SelectedObjectUiState;

pub(crate) use actions::handle_selected_object_actions;
pub(crate) use panel::{
    DevSelectedObjectActionButton, DevSelectedObjectToggleButton, DevSelectedObjectUi,
};
