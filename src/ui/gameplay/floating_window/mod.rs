//! Reusable draggable gameplay floating-window foundation (BP5).

mod components;
mod id;
mod math;
mod state;
mod systems;
#[cfg(test)]
mod tests;

pub use components::{FloatingGameplayWindowRoot, FloatingWindowTitleBarDragRegion};
pub use id::FloatingGameplayWindowId;
pub use math::{
    TITLE_BAR_HEIGHT_PX, default_building_menu_position, default_unit_inventory_position,
};
pub use state::{FloatingGameplayWindowRegistry, FloatingWindowSessionState};
pub use systems::{
    focus_floating_gameplay_window_on_ui_press, handle_floating_gameplay_window_pointer,
    measure_floating_gameplay_window_sizes, sync_floating_gameplay_window_presentation,
    sync_floating_gameplay_window_viewport,
};
