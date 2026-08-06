//! Client-local dev-window session state (Slice 3).

use std::collections::HashMap;

use bevy::prelude::*;

use super::id::DevWindowId;
use super::math::{
    DEFAULT_PANEL_WIDTH_PX, default_catalog_position, default_debug_position,
    default_fields_position, default_navigation_editor_position, default_save_position,
    default_selected_object_position, default_world_position, navigation_editor_panel_width,
    z_index_for_focus_order,
};

/// Per-window session state (not persisted to disk or scene saves).
#[derive(Debug, Clone, PartialEq)]
pub struct DevWindowSessionState {
    pub visible: bool,
    pub collapsed: bool,
    /// Top-left screen position in logical pixels.
    pub position: Vec2,
    /// Last computed rendered size (updated from layout when available).
    pub computed_size: Vec2,
}

impl DevWindowSessionState {
    pub fn new_default(id: DevWindowId, viewport: Vec2) -> Self {
        let width = match id {
            DevWindowId::NavigationEditor => navigation_editor_panel_width(viewport),
            _ => DEFAULT_PANEL_WIDTH_PX,
        };
        let position = match id {
            DevWindowId::Save => default_save_position(viewport, width),
            DevWindowId::Catalog => default_catalog_position(viewport, width),
            DevWindowId::SelectedObject => default_selected_object_position(viewport, width),
            DevWindowId::NavigationEditor => default_navigation_editor_position(viewport, width),
            DevWindowId::Debug => default_debug_position(viewport, width),
            DevWindowId::World => default_world_position(viewport, width),
            DevWindowId::Fields => default_fields_position(viewport, width),
        };
        Self {
            visible: id.default_visible(),
            collapsed: false,
            position,
            computed_size: Vec2::new(width, 400.0),
        }
    }
}

/// Active title-bar drag capture.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DevWindowDragSession {
    pub window: DevWindowId,
    /// Pointer minus window origin at grab time.
    pub grab_offset: Vec2,
}

/// Registry of all dev windows for the current client session.
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct DevWindowRegistry {
    pub windows: HashMap<DevWindowId, DevWindowSessionState>,
    /// Bottom-to-top focus order.
    pub focus_stack: Vec<DevWindowId>,
    pub drag: Option<DevWindowDragSession>,
    pub viewport: Vec2,
    /// Whether the Windows launcher row shows its window buttons.
    pub launcher_expanded: bool,
    /// Whether the Advanced launcher row shows its window buttons.
    pub advanced_launcher_expanded: bool,
}

impl Default for DevWindowRegistry {
    fn default() -> Self {
        let viewport = Vec2::new(1280.0, 720.0);
        let mut windows = HashMap::new();
        windows.insert(
            DevWindowId::Save,
            DevWindowSessionState::new_default(DevWindowId::Save, viewport),
        );
        windows.insert(
            DevWindowId::Catalog,
            DevWindowSessionState::new_default(DevWindowId::Catalog, viewport),
        );
        windows.insert(
            DevWindowId::SelectedObject,
            DevWindowSessionState::new_default(DevWindowId::SelectedObject, viewport),
        );
        windows.insert(
            DevWindowId::NavigationEditor,
            DevWindowSessionState::new_default(DevWindowId::NavigationEditor, viewport),
        );
        windows.insert(
            DevWindowId::Debug,
            DevWindowSessionState::new_default(DevWindowId::Debug, viewport),
        );
        windows.insert(
            DevWindowId::World,
            DevWindowSessionState::new_default(DevWindowId::World, viewport),
        );
        windows.insert(
            DevWindowId::Fields,
            DevWindowSessionState::new_default(DevWindowId::Fields, viewport),
        );
        Self {
            windows,
            focus_stack: vec![
                DevWindowId::Save,
                DevWindowId::Catalog,
                DevWindowId::SelectedObject,
                DevWindowId::NavigationEditor,
                DevWindowId::Debug,
                DevWindowId::World,
                DevWindowId::Fields,
            ],
            drag: None,
            viewport,
            launcher_expanded: true,
            advanced_launcher_expanded: false,
        }
    }
}

impl DevWindowRegistry {
    pub fn session(&self, id: DevWindowId) -> Option<&DevWindowSessionState> {
        self.windows.get(&id)
    }

    pub fn session_mut(&mut self, id: DevWindowId) -> Option<&mut DevWindowSessionState> {
        self.windows.get_mut(&id)
    }

    pub fn is_visible(&self, id: DevWindowId) -> bool {
        self.windows.get(&id).is_some_and(|state| state.visible)
    }

    /// Whether dev mode is on and the given window is shown (domain UI gate).
    pub fn window_active(&self, dev_enabled: bool, id: DevWindowId) -> bool {
        dev_enabled && self.is_visible(id)
    }

    pub fn show(&mut self, id: DevWindowId) {
        if let Some(state) = self.windows.get_mut(&id) {
            state.visible = true;
        }
        self.focus_window(id);
    }

    /// Show a hidden window or hide a visible one; position and collapsed state are preserved.
    pub fn toggle(&mut self, id: DevWindowId) {
        if self.is_visible(id) {
            self.hide(id);
        } else {
            self.show(id);
        }
    }

    pub fn hide(&mut self, id: DevWindowId) {
        if let Some(state) = self.windows.get_mut(&id) {
            state.visible = false;
        }
        if self.drag.is_some_and(|drag| drag.window == id) {
            self.drag = None;
        }
    }

    pub fn focus_window(&mut self, id: DevWindowId) {
        if !self.windows.contains_key(&id) {
            return;
        }
        self.focus_stack.retain(|&existing| existing != id);
        self.focus_stack.push(id);
    }

    pub fn focus_index(&self, id: DevWindowId) -> usize {
        self.focus_stack
            .iter()
            .position(|&existing| existing == id)
            .unwrap_or(0)
    }

    pub fn z_index(&self, id: DevWindowId) -> i32 {
        z_index_for_focus_order(self.focus_index(id))
    }

    pub fn begin_drag(&mut self, window: DevWindowId, grab_offset: Vec2) {
        self.focus_window(window);
        self.drag = Some(DevWindowDragSession {
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
        for state in self.windows.values_mut() {
            state.position =
                super::math::clamp_window_position(state.position, state.computed_size, viewport);
        }
    }
}

/// Derived interaction state — single writer source for input blocking.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DevWindowInteractionState {
    pub any_window_hovered: bool,
    pub launcher_hovered: bool,
    pub dragging: bool,
}

impl DevWindowInteractionState {
    pub fn blocks_world_mouse(self) -> bool {
        self.any_window_hovered || self.launcher_hovered || self.dragging
    }

    pub fn blocks_camera(self) -> bool {
        self.dragging
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_windows_are_visible() {
        let registry = DevWindowRegistry::default();
        assert!(registry.is_visible(DevWindowId::Catalog));
        assert!(registry.is_visible(DevWindowId::SelectedObject));
    }

    #[test]
    fn hide_preserves_position() {
        let mut registry = DevWindowRegistry::default();
        let pos = registry.session(DevWindowId::Catalog).unwrap().position;
        registry.hide(DevWindowId::Catalog);
        assert!(!registry.is_visible(DevWindowId::Catalog));
        assert_eq!(
            registry.session(DevWindowId::Catalog).unwrap().position,
            pos
        );
    }

    #[test]
    fn toggle_visibility_preserves_position() {
        let mut registry = DevWindowRegistry::default();
        let pos = registry.session(DevWindowId::Save).unwrap().position;
        registry.toggle(DevWindowId::Save);
        assert!(registry.is_visible(DevWindowId::Save));
        registry.toggle(DevWindowId::Save);
        assert!(!registry.is_visible(DevWindowId::Save));
        assert_eq!(registry.session(DevWindowId::Save).unwrap().position, pos);
    }

    #[test]
    fn focus_is_deterministic() {
        let mut registry = DevWindowRegistry::default();
        registry.focus_window(DevWindowId::Catalog);
        assert_eq!(registry.focus_stack.last(), Some(&DevWindowId::Catalog));
        assert_eq!(registry.z_index(DevWindowId::Catalog), 906);
        assert_eq!(registry.z_index(DevWindowId::SelectedObject), 901);
    }

    #[test]
    fn only_one_drag_session() {
        let mut registry = DevWindowRegistry::default();
        registry.begin_drag(DevWindowId::Catalog, Vec2::new(10.0, 5.0));
        assert!(registry.drag.is_some());
        registry.end_drag();
        assert!(registry.drag.is_none());
    }

    #[test]
    fn interaction_blocks_when_dragging() {
        let state = DevWindowInteractionState {
            dragging: true,
            ..Default::default()
        };
        assert!(state.blocks_world_mouse());
        assert!(state.blocks_camera());
    }

    #[test]
    fn hidden_window_does_not_block_by_default() {
        let state = DevWindowInteractionState::default();
        assert!(!state.blocks_world_mouse());
    }

    #[test]
    fn hover_blocks_world_mouse() {
        let state = DevWindowInteractionState {
            any_window_hovered: true,
            ..Default::default()
        };
        assert!(state.blocks_world_mouse());
        assert!(!state.blocks_camera());
    }

    #[test]
    fn viewport_resize_reclamps_windows() {
        let mut registry = DevWindowRegistry::default();
        registry.set_viewport(Vec2::new(800.0, 600.0));
        let pos = registry.session(DevWindowId::Catalog).unwrap().position;
        assert!(pos.x >= -(368.0 - 80.0));
        assert!(pos.y >= 0.0);
    }

    #[test]
    fn repeated_focus_does_not_grow_stack_unbounded() {
        let mut registry = DevWindowRegistry::default();
        for _ in 0..20 {
            registry.focus_window(DevWindowId::Catalog);
        }
        assert_eq!(registry.focus_stack.len(), 7);
        assert_eq!(registry.z_index(DevWindowId::Catalog), 906);
    }

    #[test]
    fn advanced_windows_default_hidden() {
        let registry = DevWindowRegistry::default();
        assert!(!registry.is_visible(DevWindowId::Debug));
        assert!(!registry.is_visible(DevWindowId::World));
        assert!(!registry.is_visible(DevWindowId::Fields));
    }

    #[test]
    fn window_active_requires_visibility_and_dev_enabled() {
        let registry = DevWindowRegistry::default();
        assert!(!registry.window_active(true, DevWindowId::Debug));
        let mut registry = registry;
        registry.show(DevWindowId::Debug);
        assert!(registry.window_active(true, DevWindowId::Debug));
        assert!(!registry.window_active(false, DevWindowId::Debug));
    }

    #[test]
    fn advanced_launcher_defaults_collapsed() {
        let registry = DevWindowRegistry::default();
        assert!(!registry.advanced_launcher_expanded);
    }

    #[test]
    fn save_window_defaults_hidden() {
        let registry = DevWindowRegistry::default();
        assert!(!registry.is_visible(DevWindowId::Save));
    }
}
