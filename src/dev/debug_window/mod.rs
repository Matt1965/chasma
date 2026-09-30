//! Debug diagnostic window (Slice 8).

mod panel;

#[cfg(test)]
mod tests;

pub use panel::{
    DevAnimationText, handle_debug_toggle_buttons, setup_debug_window_panel,
    sync_debug_panel_button_styles, sync_debug_panel_content,
};
