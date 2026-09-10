//! Geometric pointer hit tests for gameplay floating windows (BP5).

use bevy::prelude::*;

use super::id::FloatingGameplayWindowId;
use super::state::FloatingGameplayWindowRegistry;

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

impl FloatingGameplayWindowRegistry {
    /// True when the cursor lies inside an open floating window's measured bounds.
    pub fn pointer_inside_open_window(
        &self,
        window: FloatingGameplayWindowId,
        cursor: Vec2,
    ) -> bool {
        let Some(state) = self.session(window) else {
            return false;
        };
        ui_rect_contains(cursor, state.position, state.computed_size)
    }

    /// True when the cursor lies inside any of the provided open windows.
    pub fn pointer_inside_any_open(&self, cursor: Vec2, open: &[FloatingGameplayWindowId]) -> bool {
        open.iter()
            .any(|window| self.pointer_inside_open_window(*window, cursor))
    }
}

/// Collect currently open gameplay floating window ids.
pub fn open_gameplay_floating_windows(
    building_menu_open: bool,
    inventory_open: bool,
    unit_skills_open: bool,
    settlement_workforce_open: bool,
) -> Vec<FloatingGameplayWindowId> {
    let mut open = Vec::new();
    if building_menu_open {
        open.push(FloatingGameplayWindowId::BuildingMenu);
    }
    if inventory_open {
        open.push(FloatingGameplayWindowId::UnitInventory);
    }
    if unit_skills_open {
        open.push(FloatingGameplayWindowId::UnitSkills);
    }
    if settlement_workforce_open {
        open.push(FloatingGameplayWindowId::SettlementWorkforce);
    }
    open
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_contains_interior_point() {
        assert!(ui_rect_contains(
            Vec2::new(50.0, 60.0),
            Vec2::new(10.0, 20.0),
            Vec2::new(100.0, 80.0)
        ));
    }

    #[test]
    fn rect_excludes_exterior_point() {
        assert!(!ui_rect_contains(
            Vec2::new(5.0, 60.0),
            Vec2::new(10.0, 20.0),
            Vec2::new(100.0, 80.0)
        ));
    }

    #[test]
    fn dragged_window_uses_updated_bounds() {
        let mut registry = FloatingGameplayWindowRegistry::default();
        registry
            .session_mut(FloatingGameplayWindowId::BuildingMenu)
            .unwrap()
            .position = Vec2::new(200.0, 100.0);
        registry
            .session_mut(FloatingGameplayWindowId::BuildingMenu)
            .unwrap()
            .computed_size = Vec2::new(320.0, 400.0);
        let open = [FloatingGameplayWindowId::BuildingMenu];
        assert!(registry.pointer_inside_any_open(Vec2::new(250.0, 150.0), &open));
        assert!(!registry.pointer_inside_any_open(Vec2::new(50.0, 50.0), &open));
    }
}
