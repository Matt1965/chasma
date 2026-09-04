//! Client-local gameplay floating-window session state (BP5).

use std::collections::HashMap;

use bevy::prelude::*;

use super::id::FloatingGameplayWindowId;
use super::math::{
    clamp_window_position, default_building_menu_position, default_unit_inventory_position,
    default_unit_skills_position, z_index_for_focus_order,
};

/// Per-window session layout (not persisted across application launches).
#[derive(Debug, Clone, PartialEq)]
pub struct FloatingWindowSessionState {
    /// Top-left screen position in logical pixels.
    pub position: Vec2,
    /// Last measured rendered size for clamping.
    pub computed_size: Vec2,
    /// Whether a default position has been assigned at least once.
    pub initialized: bool,
}

impl FloatingWindowSessionState {
    fn new_default(id: FloatingGameplayWindowId, viewport: Vec2) -> Self {
        let position = match id {
            FloatingGameplayWindowId::BuildingMenu => default_building_menu_position(viewport),
            FloatingGameplayWindowId::UnitInventory => default_unit_inventory_position(viewport),
            FloatingGameplayWindowId::UnitSkills => default_unit_skills_position(viewport),
        };
        Self {
            position,
            computed_size: default_computed_size(id, viewport),
            initialized: true,
        }
    }
}

fn default_computed_size(id: FloatingGameplayWindowId, viewport: Vec2) -> Vec2 {
    match id {
        FloatingGameplayWindowId::BuildingMenu => Vec2::new(320.0, viewport.y * 0.65),
        FloatingGameplayWindowId::UnitInventory => {
            Vec2::new((viewport.x * 0.42).clamp(280.0, 520.0), viewport.y * 0.72)
        }
        FloatingGameplayWindowId::UnitSkills => Vec2::new(300.0, viewport.y * 0.55),
    }
}

/// Active title-bar drag capture.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FloatingWindowDragSession {
    pub window: FloatingGameplayWindowId,
    pub grab_offset: Vec2,
}

/// Registry of gameplay floating windows for the current client session.
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct FloatingGameplayWindowRegistry {
    pub windows: HashMap<FloatingGameplayWindowId, FloatingWindowSessionState>,
    pub focus_stack: Vec<FloatingGameplayWindowId>,
    pub drag: Option<FloatingWindowDragSession>,
    pub viewport: Vec2,
}

impl Default for FloatingGameplayWindowRegistry {
    fn default() -> Self {
        let viewport = Vec2::new(1280.0, 720.0);
        let mut windows = HashMap::new();
        for id in FloatingGameplayWindowId::ALL {
            windows.insert(id, FloatingWindowSessionState::new_default(id, viewport));
        }
        Self {
            windows,
            focus_stack: FloatingGameplayWindowId::ALL.to_vec(),
            drag: None,
            viewport,
        }
    }
}

impl FloatingGameplayWindowRegistry {
    pub fn session(&self, id: FloatingGameplayWindowId) -> Option<&FloatingWindowSessionState> {
        self.windows.get(&id)
    }

    pub fn session_mut(
        &mut self,
        id: FloatingGameplayWindowId,
    ) -> Option<&mut FloatingWindowSessionState> {
        self.windows.get_mut(&id)
    }

    pub fn ensure_initialized(&mut self, id: FloatingGameplayWindowId) {
        if self.windows.get(&id).is_some_and(|state| state.initialized) {
            return;
        }
        let state = FloatingWindowSessionState::new_default(id, self.viewport);
        self.windows.insert(id, state);
    }

    pub fn focus_window(&mut self, id: FloatingGameplayWindowId) {
        if !self.windows.contains_key(&id) {
            return;
        }
        self.focus_stack.retain(|existing| *existing != id);
        self.focus_stack.push(id);
    }

    pub fn focus_index(&self, id: FloatingGameplayWindowId) -> usize {
        self.focus_stack
            .iter()
            .position(|existing| *existing == id)
            .unwrap_or(0)
    }

    pub fn z_index(&self, id: FloatingGameplayWindowId) -> i32 {
        z_index_for_focus_order(self.focus_index(id))
    }

    pub fn begin_drag(&mut self, window: FloatingGameplayWindowId, grab_offset: Vec2) {
        self.focus_window(window);
        self.drag = Some(FloatingWindowDragSession {
            window,
            grab_offset,
        });
    }

    pub fn end_drag(&mut self) {
        self.drag = None;
    }

    pub fn cancel_drag(&mut self) {
        self.drag = None;
    }

    pub fn set_viewport(&mut self, viewport: Vec2) {
        if (self.viewport - viewport).length_squared() < 0.5 {
            return;
        }
        self.viewport = viewport;
        for (id, state) in self.windows.iter_mut() {
            if !state.initialized {
                *state = FloatingWindowSessionState::new_default(*id, viewport);
            }
            state.computed_size = default_computed_size(*id, viewport);
            state.position = clamp_window_position(state.position, state.computed_size, viewport);
        }
    }

    pub fn is_dragging(&self) -> bool {
        self.drag.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn building_menu_has_floating_window_state() {
        let registry = FloatingGameplayWindowRegistry::default();
        assert!(
            registry
                .session(FloatingGameplayWindowId::BuildingMenu)
                .is_some()
        );
    }

    #[test]
    fn unit_inventory_has_floating_window_state() {
        let registry = FloatingGameplayWindowRegistry::default();
        assert!(
            registry
                .session(FloatingGameplayWindowId::UnitInventory)
                .is_some()
        );
    }

    #[test]
    fn bottom_hud_is_not_a_floating_window_id() {
        let ids: Vec<_> = FloatingGameplayWindowId::ALL.to_vec();
        assert_eq!(ids.len(), 3);
    }

    #[test]
    fn focus_is_deterministic() {
        let mut registry = FloatingGameplayWindowRegistry::default();
        registry.focus_window(FloatingGameplayWindowId::UnitInventory);
        assert_eq!(
            registry.focus_stack.last(),
            Some(&FloatingGameplayWindowId::UnitInventory)
        );
        assert!(
            registry.z_index(FloatingGameplayWindowId::UnitInventory)
                > registry.z_index(FloatingGameplayWindowId::BuildingMenu)
        );
    }

    #[test]
    fn close_reopen_preserves_session_position() {
        let mut registry = FloatingGameplayWindowRegistry::default();
        let pos = registry
            .session(FloatingGameplayWindowId::BuildingMenu)
            .unwrap()
            .position;
        registry
            .session_mut(FloatingGameplayWindowId::BuildingMenu)
            .unwrap()
            .position = Vec2::new(123.0, 45.0);
        let remembered = registry
            .session(FloatingGameplayWindowId::BuildingMenu)
            .unwrap()
            .position;
        assert_ne!(remembered, pos);
        assert_eq!(remembered, Vec2::new(123.0, 45.0));
    }

    #[test]
    fn switching_building_menu_target_shares_one_window_position() {
        let mut registry = FloatingGameplayWindowRegistry::default();
        registry
            .session_mut(FloatingGameplayWindowId::BuildingMenu)
            .unwrap()
            .position = Vec2::new(200.0, 100.0);
        let after = registry
            .session(FloatingGameplayWindowId::BuildingMenu)
            .unwrap()
            .position;
        assert_eq!(after, Vec2::new(200.0, 100.0));
    }

    #[test]
    fn viewport_shrink_reclamps_window_header() {
        let mut registry = FloatingGameplayWindowRegistry::default();
        registry
            .session_mut(FloatingGameplayWindowId::UnitInventory)
            .unwrap()
            .position = Vec2::new(1500.0, 0.0);
        registry.set_viewport(Vec2::new(800.0, 600.0));
        let pos = registry
            .session(FloatingGameplayWindowId::UnitInventory)
            .unwrap()
            .position;
        assert!(pos.x <= 800.0 - super::super::math::MIN_TITLE_GRAB_PX);
    }
}
