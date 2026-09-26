//! Player dialogue / social interaction UI.

mod content;
mod input;
mod panel;
mod state;

#[cfg(test)]
mod tests;

pub use input::collect_dialogue_keyboard_input;
pub use panel::{
    handle_dialogue_back_button, handle_dialogue_close_button, handle_dialogue_option_buttons,
    reconcile_dialogue_panel, spawn_dialogue_panel, sync_dialogue_panel, sync_dialogue_panel_visibility,
};
pub use content::target_display_name;
pub use state::DialogueSessionState;
