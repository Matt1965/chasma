//! Gameplay floating-window drag math and viewport clamping (BP5).

use bevy::prelude::*;

pub const TITLE_BAR_HEIGHT_PX: f32 = 28.0;
pub const MIN_TITLE_GRAB_PX: f32 = 80.0;
pub const Z_INDEX_BASE: i32 = 410;

/// Top-left screen position preserving grab offset during drag.
pub fn window_position_from_pointer(pointer: Vec2, grab_offset: Vec2) -> Vec2 {
    pointer - grab_offset
}

/// Clamp top-left so the title bar remains recoverable after viewport shrink.
pub fn clamp_window_position(position: Vec2, window_size: Vec2, viewport: Vec2) -> Vec2 {
    if viewport.x <= 1.0 || viewport.y <= 1.0 {
        return position;
    }

    let width = window_size.x.max(1.0);
    let height = window_size.y.max(TITLE_BAR_HEIGHT_PX);
    let min_grab = MIN_TITLE_GRAB_PX.min(width);

    let min_x = -(width - min_grab);
    let max_x = (viewport.x - min_grab).max(0.0);
    let x = position.x.clamp(min_x, max_x);

    let max_y = (viewport.y - TITLE_BAR_HEIGHT_PX).max(0.0);
    let y = position.y.clamp(0.0, max_y);

    let min_y = (viewport.y - height).min(y);
    let y = y.max(min_y);

    Vec2::new(x, y)
}

pub fn default_building_menu_position(_viewport: Vec2) -> Vec2 {
    Vec2::new(12.0, 72.0)
}

pub fn default_unit_inventory_position(viewport: Vec2) -> Vec2 {
    let width = (viewport.x * 0.42).clamp(280.0, 520.0);
    let x = (viewport.x - width - 16.0).max(12.0);
    Vec2::new(x, 72.0)
}

pub fn z_index_for_focus_order(focus_index: usize) -> i32 {
    Z_INDEX_BASE + focus_index as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_keeps_title_bar_recoverable_at_right_edge() {
        let viewport = Vec2::new(1280.0, 720.0);
        let size = Vec2::new(360.0, 500.0);
        let clamped = clamp_window_position(Vec2::new(2000.0, 0.0), size, viewport);
        assert!(clamped.x <= viewport.x - MIN_TITLE_GRAB_PX);
    }

    #[test]
    fn default_positions_do_not_overlap_on_typical_viewport() {
        let viewport = Vec2::new(1280.0, 720.0);
        let building = default_building_menu_position(viewport);
        let inventory = default_unit_inventory_position(viewport);
        assert!(inventory.x > building.x + 200.0);
    }
}
