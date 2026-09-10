//! Reusable draggable gameplay floating-window foundation (BP5).

mod components;
mod hit_test;
mod id;
mod math;
mod shell;
mod state;
mod systems;
#[cfg(test)]
mod tests;
mod tokens;

pub use components::{FloatingGameplayWindowRoot, FloatingWindowTitleBarDragRegion};
pub use hit_test::{open_gameplay_floating_windows, ui_rect_contains};
pub use id::FloatingGameplayWindowId;
pub use math::{
    TITLE_BAR_HEIGHT_PX, default_building_menu_position, default_unit_inventory_position,
};
pub use shell::{
    FloatingWindowBody, FloatingWindowChrome, FloatingWindowCloseChrome,
    FloatingWindowRaisedButton, FloatingWindowTitleRail, floating_row_separator_border,
    floating_window_shell_colors, floating_window_shell_node, spawn_floating_close_button,
    spawn_floating_raised_button, spawn_floating_raised_button_armed, spawn_floating_section_well,
    spawn_floating_title_rail, spawn_floating_title_rail_drag_only, spawn_floating_window_body,
    spawn_floating_window_inner_frame, update_floating_window_raised_button_hover,
};
pub use state::{FloatingGameplayWindowRegistry, FloatingWindowSessionState};
pub use systems::{
    focus_floating_gameplay_window_on_ui_press, handle_floating_gameplay_window_pointer,
    measure_floating_gameplay_window_sizes, sync_floating_gameplay_window_presentation,
    sync_floating_gameplay_window_viewport,
};
pub use tokens::{
    WINDOW_BG, WINDOW_BODY_PADDING_PX, WINDOW_BORDER, WINDOW_BORDER_PX, WINDOW_CORNER_RADIUS_PX,
    WINDOW_INNER_GROOVE, WINDOW_ROW_SEPARATOR, WINDOW_SECTION_BG, WINDOW_SECTION_GAP_PX,
    WINDOW_TITLE_ACCENT, WINDOW_TITLE_BG,
};
