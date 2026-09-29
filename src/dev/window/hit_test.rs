//! Geometric pointer hit tests for dev windows (window rect owns the pointer region).

use bevy::prelude::*;

use super::id::DevWindowId;
use super::math::{LAUNCHER_HEIGHT_PX, LAUNCHER_LEFT_PX, LAUNCHER_TOP_PX, TITLE_BAR_HEIGHT_PX};
use super::state::DevWindowRegistry;

/// Whether `point` lies inside a top-left anchored UI rectangle.
pub fn ui_rect_contains(point: Vec2, origin: Vec2, size: Vec2) -> bool {
    if size.x <= 0.0 || size.y <= 0.0 {
        return false;
    }
    point.x >= origin.x
        && point.y >= origin.y
        && point.x < origin.x + size.x
        && point.y < origin.y + size.y
}

impl DevWindowRegistry {
    /// Measured bounds for a visible window (collapsed windows use title-bar height only).
    pub fn visible_window_bounds(&self, id: DevWindowId) -> Option<(Vec2, Vec2)> {
        let state = self.session(id)?;
        if !state.visible {
            return None;
        }
        let width = state.computed_size.x.max(1.0);
        let height = if state.collapsed && id.supports_collapse() {
            TITLE_BAR_HEIGHT_PX
        } else {
            state.computed_size.y.max(TITLE_BAR_HEIGHT_PX)
        };
        Some((state.position, Vec2::new(width, height)))
    }

    /// True when the cursor lies inside any visible dev window's screen rectangle.
    pub fn pointer_over_visible_window(&self, cursor: Vec2) -> bool {
        for id in self.windows.keys() {
            let Some((origin, size)) = self.visible_window_bounds(*id) else {
                continue;
            };
            if ui_rect_contains(cursor, origin, size) {
                return true;
            }
        }
        false
    }

    /// Workspace launcher strip (expanded rows included) — blocks world input when visible.
    pub fn pointer_over_workspace_launcher(&self, cursor: Vec2) -> bool {
        let rows = 1u32 + u32::from(self.advanced_launcher_expanded);
        let height = LAUNCHER_TOP_PX + LAUNCHER_HEIGHT_PX * rows as f32;
        let width = (self.viewport.x - LAUNCHER_LEFT_PX).max(0.0);
        if width <= 0.0 || height <= 0.0 {
            return false;
        }
        ui_rect_contains(cursor, Vec2::new(LAUNCHER_LEFT_PX, LAUNCHER_TOP_PX), Vec2::new(width, height))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interior_padding_blocks_without_button_hit() {
        let registry = DevWindowRegistry::default();
        let catalog = DevWindowId::Catalog;
        let pos = registry.session(catalog).unwrap().position;
        let size = registry.session(catalog).unwrap().computed_size;
        let interior = pos + Vec2::new(size.x * 0.5, size.y * 0.5);
        assert!(registry.pointer_over_visible_window(interior));
    }

    #[test]
    fn click_outside_visible_window_does_not_block() {
        let registry = DevWindowRegistry::default();
        assert!(!registry.pointer_over_visible_window(Vec2::new(4.0, 4.0)));
    }

    #[test]
    fn hidden_window_does_not_block() {
        let mut registry = DevWindowRegistry::default();
        registry.hide(DevWindowId::Catalog);
        let pos = registry.session(DevWindowId::Catalog).unwrap().position;
        assert!(!registry.pointer_over_visible_window(pos));
    }

    #[test]
    fn launcher_strip_blocks_interior() {
        let registry = DevWindowRegistry::default();
        assert!(registry.pointer_over_workspace_launcher(Vec2::new(40.0, 20.0)));
    }
}
