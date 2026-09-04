//! Settlement Dev window — camera-derived settlement context UI.

mod model;
pub mod panel;
mod systems;

#[cfg(test)]
mod tests;

pub use model::{
    SettlementDevSummary, assign_selected_units_to_settlement, build_settlement_dev_summary,
    format_ai_line, format_focused_line,
};
pub use panel::{
    setup_settlement_window_panel, sync_dev_settlement_panel_visibility,
    sync_settlement_ai_toggle_styles, sync_settlement_dev_action_availability,
    sync_settlement_dev_button_styles, sync_settlement_dev_panel,
};
pub use systems::{handle_settlement_add_units_button, handle_settlement_ai_toggle};
