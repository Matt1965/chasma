//! Cursor-anchored unit interaction menu (Talk / Trade / Recruit picker).

mod content;
mod input;
mod panel;
mod state;

#[cfg(test)]
mod tests;

pub use content::{InteractionMenuRow, build_interaction_menu_rows, should_omit_interaction_menu_option};
pub use input::{
    collect_unit_interaction_menu_keyboard_input, dismiss_unit_interaction_menu_on_outside_click,
    handle_unit_interaction_menu_backdrop_click, handle_unit_interaction_menu_option_clicks,
    reconcile_unit_interaction_menu,
};
pub use panel::{
    spawn_unit_interaction_menu, sync_unit_interaction_menu, sync_unit_interaction_menu_layout,
    sync_unit_interaction_menu_visibility,
};
pub use state::UnitInteractionMenuState;
