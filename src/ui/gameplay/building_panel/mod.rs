//! Player-facing Building Panel (BP1 menu + BP2 content).

mod content;
mod controls;
mod format;
mod interaction;
mod logic;
mod menu;
mod state;

#[cfg(test)]
mod tests;

pub use content::{
    BuildingPanelHeader, BuildingPanelInventorySection, BuildingPanelOperationOption,
    BuildingPanelProduction, BuildingPanelSnapshot, binding_section_label,
    build_building_panel_snapshot,
};
pub use controls::handle_building_production_controls;
pub use format::{format_building_header_line, format_building_shell};
pub use interaction::{
    building_inventory_grid_interaction, building_inventory_transfer_eligible,
    resolve_building_inventory_actor,
};
pub use logic::{
    building_owned_by_local_player, on_gameplay_building_selected, reconcile_building_panel,
    try_open_building_menu,
};
pub use menu::{
    BuildingMenuCloseButton, BuildingMenuPanelRoot, handle_building_menu_close_button,
    reconcile_building_menu_panel, spawn_building_menu_panel, sync_building_menu_panel,
};
pub use state::BuildingPanelState;
