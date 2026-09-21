//! Settlement Workforce screen module.

mod content;
mod input;
mod layout;
mod panel;
mod scroll;
mod state;

#[cfg(test)]
mod tests;

pub use content::build_settlement_workforce_snapshot;
pub use input::collect_settlement_workforce_keyboard_input;
pub use panel::{
    handle_settlement_workforce_close_button, handle_settlement_workforce_controls,
    spawn_settlement_workforce_panel, sync_settlement_workforce_panel,
    sync_settlement_workforce_panel_dimensions, sync_settlement_workforce_panel_visibility,
};
pub use scroll::SettlementWorkforceScrollPlugin;
pub use state::SettlementWorkforcePanelState;
