//! Debug diagnostic window (Slice 8).

mod brain;
mod panel;

#[cfg(test)]
mod tests;

pub use brain::{
    BrainDecisionHistory, BrainPanelState, handle_brain_view_buttons, sync_brain_panel,
    tick_brain_decision_history,
};
pub use panel::{
    DevAnimationText, handle_debug_toggle_buttons, setup_debug_window_panel,
    sync_debug_panel_button_styles, sync_debug_panel_content,
};
