//! Kenshi-style inventory UI (ADR-092 I6).

mod drag_preview;
mod errors;
mod input;
mod panel;
mod preview;
mod state;

pub use drag_preview::{
    cleanup_inventory_drag_previews, sync_inventory_drag_ghost, sync_inventory_ground_preview,
    update_inventory_drag_preview,
};
pub use errors::InventoryUiError;
pub use input::{collect_inventory_keyboard_input, inventory_panel_blocks_world_input};
pub use panel::{
    InventoryEntryWidget, InventoryGridCell, InventoryGridPane, InventoryPaneSide,
    InventoryPanelRoot, collect_inventory_mouse_transfers, handle_inventory_drag_release,
    handle_inventory_entry_clicks, handle_inventory_panel_buttons,
    reconcile_inventory_ui_from_world, spawn_inventory_panel, sync_inventory_panel_contents,
    sync_inventory_panel_visibility,
};
pub use preview::{
    INVENTORY_CELL_PX, InventoryDropTarget, InventoryPlacementPreview, drag_state_from_entry,
    evaluate_drop_target, occupied_cells,
};
pub use state::{InventoryDragPreviewState, InventoryDragState, InventoryUiState};
