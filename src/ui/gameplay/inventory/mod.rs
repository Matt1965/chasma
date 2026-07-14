//! Kenshi-style inventory UI (ADR-092 I6).

mod errors;
mod input;
mod panel;
mod state;

pub use errors::InventoryUiError;
pub use input::{collect_inventory_keyboard_input, inventory_panel_blocks_world_input};
pub use panel::{
    collect_inventory_mouse_transfers, handle_inventory_entry_clicks,
    handle_inventory_panel_buttons, spawn_inventory_panel, sync_inventory_panel_contents,
    sync_inventory_panel_visibility,
};
pub use state::InventoryUiState;
