//! Developer item and inventory management (DV0).

mod endpoint;
mod format;
mod input;
mod ops;
pub mod panel;

pub use crate::dev::dev_mode::DevInventoryEndpoint;
pub use endpoint::{DevInventoryEndpointInfo, nearest_pile_at_position, resolve_inspector_endpoints};
pub use input::{
    handle_dev_items_ground_click, handle_dev_items_keyboard, handle_dev_items_keyboard_system,
};
pub use ops::{
    dev_add_item, dev_clear_inventory, dev_fill_inventory, dev_remove_entry, dev_set_stack_quantity,
    dev_spawn_ground_pile, dev_transfer, DevInventoryOpError,
};
pub use panel::{
    handle_dev_items_buttons, spawn_items_section, sync_item_quantity_controls,
    sync_items_panel_text, sync_items_section_visibility, DevItemsAction,
};
