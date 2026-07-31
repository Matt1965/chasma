//! Reusable draggable dev-window framework (Slice 3).

mod components;
mod id;
mod math;
mod setup;
mod state;
mod systems;

pub use components::{
    DevLauncherGroup, DevWindowBody, DevWindowCloseButton, DevWindowCollapseButton, DevWindowRoot,
    DevWindowTitleBarDragRegion, DevWindowUi, DevWorkspaceLauncher, DevWorkspaceLauncherButton,
    DevWorkspaceLauncherButtons, DevWorkspaceLauncherToggle,
};
pub use id::DevWindowId;
pub use math::{
    DEFAULT_PANEL_WIDTH_PX, MIN_TITLE_GRAB_PX, TITLE_BAR_HEIGHT_PX, clamp_window_position,
    window_position_from_pointer,
};
pub use setup::setup_dev_workspace;
pub use state::{
    DevWindowDragSession, DevWindowInteractionState, DevWindowRegistry, DevWindowSessionState,
};
pub use systems::{
    apply_dev_window_input_gate, focus_dev_window_on_panel_press, focus_dev_window_on_ui_press,
    handle_dev_mode_window_lifecycle, handle_dev_window_pointer, sync_dev_panel_hover_from_windows,
    sync_dev_window_computed_sizes, sync_dev_window_presentation, sync_dev_window_viewport,
    update_dev_window_interaction_state,
};

#[cfg(test)]
mod tests;
