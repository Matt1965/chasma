//! Dev-window drag math and viewport clamping (Slice 3).

use bevy::prelude::*;

pub const TITLE_BAR_HEIGHT_PX: f32 = 28.0;
pub const MIN_TITLE_GRAB_PX: f32 = 80.0;
pub const LAUNCHER_HEIGHT_PX: f32 = 28.0;
pub const LAUNCHER_TOP_PX: f32 = 12.0;
pub const LAUNCHER_LEFT_PX: f32 = 12.0;
pub const DEFAULT_PANEL_WIDTH_PX: f32 = 368.0;
/// Default Navigation Editor width (roughly 2x legacy dev panels).
pub const NAVIGATION_EDITOR_WIDTH_PX: f32 = 760.0;
/// Minimum usable Navigation Editor width before single-column fallback.
pub const NAVIGATION_EDITOR_MIN_WIDTH_PX: f32 = 620.0;
/// Viewport margin when clamping Navigation Editor width.
pub const NAVIGATION_EDITOR_VIEWPORT_MARGIN_PX: f32 = 12.0;
pub const DEFAULT_PANEL_BODY_PADDING_PX: f32 = 10.0;
/// Margin between the Navigation Editor bottom edge and the viewport bottom.
pub const NAV_EDITOR_VIEWPORT_MARGIN_PX: f32 = 12.0;
/// Minimum scrollable body height for the Navigation Editor.
pub const NAV_EDITOR_MIN_BODY_HEIGHT_PX: f32 = 180.0;
/// Upper cap for Navigation Editor body height on very tall displays.
pub const NAV_EDITOR_MAX_BODY_HEIGHT_PX: f32 = 720.0;

/// Top-left screen position for a window given grab offset preservation.
pub fn window_position_from_pointer(pointer: Vec2, grab_offset: Vec2) -> Vec2 {
    pointer - grab_offset
}

/// Clamp a window's top-left so the title bar remains recoverable.
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

    // Keep at least part of the window above the bottom edge when taller than viewport.
    let min_y = (viewport.y - height).min(y);
    let y = y.max(min_y);

    Vec2::new(x, y)
}

/// Default top-left for the Save window (left side, below launcher rows).
pub fn default_save_position(viewport: Vec2, window_width: f32) -> Vec2 {
    let _ = viewport;
    let top = LAUNCHER_TOP_PX + LAUNCHER_HEIGHT_PX * 2.0 + 10.0;
    Vec2::new(LAUNCHER_LEFT_PX + window_width + 16.0, top)
}

/// Default top-left for the catalog window (legacy top-right placement).
pub fn default_catalog_position(viewport: Vec2, window_width: f32) -> Vec2 {
    let top = LAUNCHER_TOP_PX + LAUNCHER_HEIGHT_PX + 6.0;
    let x = (viewport.x - window_width - 12.0).max(LAUNCHER_LEFT_PX);
    Vec2::new(x, top)
}

/// Resolve Navigation Editor panel width for the current viewport.
pub fn navigation_editor_panel_width(viewport: Vec2) -> f32 {
    let max_width = (viewport.x - NAVIGATION_EDITOR_VIEWPORT_MARGIN_PX * 2.0).max(320.0);
    NAVIGATION_EDITOR_WIDTH_PX
        .min(max_width)
        .max(NAVIGATION_EDITOR_MIN_WIDTH_PX.min(max_width))
}

/// Whether the Navigation Editor should use its two-column body layout.
pub fn navigation_editor_uses_two_columns(panel_width: f32) -> bool {
    panel_width >= NAVIGATION_EDITOR_MIN_WIDTH_PX
}

/// Maximum scrollable body height for the Navigation Editor at the given viewport and window top.
pub fn navigation_editor_body_max_height(viewport: Vec2, window_top: f32) -> f32 {
    let available = viewport.y - window_top - TITLE_BAR_HEIGHT_PX - NAV_EDITOR_VIEWPORT_MARGIN_PX;
    available.clamp(NAV_EDITOR_MIN_BODY_HEIGHT_PX, NAV_EDITOR_MAX_BODY_HEIGHT_PX)
}

/// Default top-left for the Navigation Editor (center-right, below launcher).
pub fn default_navigation_editor_position(viewport: Vec2, window_width: f32) -> Vec2 {
    let top = LAUNCHER_TOP_PX + LAUNCHER_HEIGHT_PX + 6.0;
    let x = (viewport.x - window_width - 12.0).max(LAUNCHER_LEFT_PX);
    let y = top + 120.0;
    Vec2::new(x, y)
}

/// Default top-left for the Selected Object window (left side, below launcher).
pub fn default_selected_object_position(viewport: Vec2, window_width: f32) -> Vec2 {
    let _ = viewport;
    let top = LAUNCHER_TOP_PX + LAUNCHER_HEIGHT_PX + 6.0;
    Vec2::new(LAUNCHER_LEFT_PX, top)
}

/// Default top-left for the Debug window (center-right, offset from catalog).
pub fn default_debug_position(viewport: Vec2, window_width: f32) -> Vec2 {
    let top = LAUNCHER_TOP_PX + LAUNCHER_HEIGHT_PX + 6.0;
    let x = (viewport.x - window_width - 12.0).max(LAUNCHER_LEFT_PX);
    Vec2::new(x, top + 80.0)
}

/// Default top-left for the World window (left side, below Selected Object).
pub fn default_world_position(viewport: Vec2, window_width: f32) -> Vec2 {
    let _ = window_width;
    let top = LAUNCHER_TOP_PX + LAUNCHER_HEIGHT_PX + 6.0;
    Vec2::new(LAUNCHER_LEFT_PX, top + 200.0)
}

/// Default top-left for the Settlement window (left side, below World).
pub fn default_settlement_position(viewport: Vec2, window_width: f32) -> Vec2 {
    let _ = viewport;
    let top = LAUNCHER_TOP_PX + LAUNCHER_HEIGHT_PX + 6.0;
    Vec2::new(LAUNCHER_LEFT_PX, top + 320.0)
}

/// Default top-left for the Fields window (wider placement, center-left).
pub fn default_fields_position(viewport: Vec2, window_width: f32) -> Vec2 {
    let _ = viewport;
    let top = LAUNCHER_TOP_PX + LAUNCHER_HEIGHT_PX + 6.0;
    Vec2::new(LAUNCHER_LEFT_PX + window_width + 16.0, top + 40.0)
}

/// Z-index for a window given its focus-stack index.
pub fn z_index_for_focus_order(focus_index: usize) -> i32 {
    const BASE: i32 = 900;
    BASE + focus_index as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drag_preserves_grab_offset() {
        let window_origin = Vec2::new(100.0, 50.0);
        let pointer_at_grab = Vec2::new(140.0, 70.0);
        let offset = pointer_at_grab - window_origin;
        let moved_pointer = Vec2::new(200.0, 90.0);
        let new_origin = window_position_from_pointer(moved_pointer, offset);
        assert!((new_origin.x - 160.0).abs() < 0.01);
        assert!((new_origin.y - 70.0).abs() < 0.01);
    }

    #[test]
    fn drag_does_not_snap_origin_to_cursor() {
        let offset = Vec2::new(30.0, 10.0);
        let pos = window_position_from_pointer(Vec2::new(500.0, 300.0), offset);
        assert_ne!(pos, Vec2::new(500.0, 300.0));
    }

    #[test]
    fn clamp_keeps_title_bar_recoverable_at_right_edge() {
        let viewport = Vec2::new(1280.0, 720.0);
        let size = Vec2::new(368.0, 500.0);
        let clamped = clamp_window_position(Vec2::new(2000.0, 0.0), size, viewport);
        assert!(clamped.x <= viewport.x - MIN_TITLE_GRAB_PX);
    }

    #[test]
    fn clamp_prevents_above_top() {
        let viewport = Vec2::new(1920.0, 1080.0);
        let size = Vec2::new(368.0, 400.0);
        let clamped = clamp_window_position(Vec2::new(100.0, -50.0), size, viewport);
        assert!(clamped.y >= 0.0);
    }

    #[test]
    fn clamp_wide_window_on_narrow_viewport() {
        let viewport = Vec2::new(800.0, 600.0);
        let size = Vec2::new(900.0, 400.0);
        let clamped = clamp_window_position(Vec2::new(-500.0, 20.0), size, viewport);
        assert!(clamped.x >= -(size.x - MIN_TITLE_GRAB_PX));
        assert!(clamped.x <= viewport.x - MIN_TITLE_GRAB_PX);
    }

    #[test]
    fn z_index_grows_with_focus_but_is_bounded_per_stack() {
        assert_eq!(z_index_for_focus_order(0), 900);
        assert_eq!(z_index_for_focus_order(3), 903);
    }

    #[test]
    fn navigation_editor_width_defaults_near_target() {
        let width = navigation_editor_panel_width(Vec2::new(1920.0, 1080.0));
        assert!((width - NAVIGATION_EDITOR_WIDTH_PX).abs() < 1.0);
    }

    #[test]
    fn navigation_editor_width_clamps_to_viewport() {
        let width = navigation_editor_panel_width(Vec2::new(640.0, 480.0));
        assert!(width <= 640.0 - NAVIGATION_EDITOR_VIEWPORT_MARGIN_PX * 2.0);
    }

    #[test]
    fn navigation_editor_two_column_threshold() {
        assert!(navigation_editor_uses_two_columns(
            NAVIGATION_EDITOR_MIN_WIDTH_PX
        ));
        assert!(!navigation_editor_uses_two_columns(500.0));
    }

    #[test]
    fn navigation_editor_body_height_clamps_to_viewport_720p() {
        let viewport = Vec2::new(1280.0, 720.0);
        let width = navigation_editor_panel_width(viewport);
        let top = default_navigation_editor_position(viewport, width).y;
        let height = navigation_editor_body_max_height(viewport, top);
        assert!(height >= NAV_EDITOR_MIN_BODY_HEIGHT_PX);
        assert!(top + TITLE_BAR_HEIGHT_PX + height + NAV_EDITOR_VIEWPORT_MARGIN_PX <= viewport.y);
    }

    #[test]
    fn navigation_editor_body_height_grows_on_1080p() {
        let viewport = Vec2::new(1920.0, 1080.0);
        let width = navigation_editor_panel_width(viewport);
        let top = default_navigation_editor_position(viewport, width).y;
        let height_1080 = navigation_editor_body_max_height(viewport, top);
        let height_720 = navigation_editor_body_max_height(Vec2::new(1280.0, 720.0), top);
        assert!(height_1080 > height_720);
    }

    #[test]
    fn navigation_editor_body_height_respects_minimum_on_short_viewport() {
        let viewport = Vec2::new(800.0, 480.0);
        let top = 300.0;
        let height = navigation_editor_body_max_height(viewport, top);
        assert_eq!(height, NAV_EDITOR_MIN_BODY_HEIGHT_PX);
    }
}
