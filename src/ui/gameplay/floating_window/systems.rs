//! Gameplay floating-window systems — drag, focus, presentation (BP5).

use bevy::input::mouse::MouseButton;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use super::components::{FloatingGameplayWindowRoot, FloatingWindowTitleBarDragRegion};
use super::id::FloatingGameplayWindowId;
use super::math::{clamp_window_position, window_position_from_pointer};
use super::state::FloatingGameplayWindowRegistry;
use crate::ui::gameplay::inventory::InventoryUiState;

/// Track viewport size and re-clamp remembered positions on resize.
pub fn sync_floating_gameplay_window_viewport(
    windows: Query<&Window, With<PrimaryWindow>>,
    mut registry: ResMut<FloatingGameplayWindowRegistry>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    registry.set_viewport(Vec2::new(window.width(), window.height()));
}

/// Title-bar drag only — does not move windows from inventory cells or body content.
pub fn handle_floating_gameplay_window_pointer(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    inventory_ui: Res<InventoryUiState>,
    mut registry: ResMut<FloatingGameplayWindowRegistry>,
    drag_regions: Query<(&Interaction, &FloatingWindowTitleBarDragRegion)>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let pointer = window.cursor_position().unwrap_or(Vec2::ZERO);

    if let Some(drag) = registry.drag {
        if mouse.pressed(MouseButton::Left) {
            if let Some(state) = registry.session_mut(drag.window) {
                state.position = window_position_from_pointer(pointer, drag.grab_offset);
            }
        } else {
            let viewport = registry.viewport;
            if let Some(state) = registry.session_mut(drag.window) {
                state.position =
                    clamp_window_position(state.position, state.computed_size, viewport);
            }
            registry.end_drag();
        }
        return;
    }

    if !mouse.just_pressed(MouseButton::Left) || inventory_ui.dragging.is_some() {
        return;
    }

    for (interaction, region) in &drag_regions {
        if *interaction == Interaction::Pressed {
            registry.focus_window(region.id);
            if let Some(state) = registry.session(region.id) {
                let grab_offset = pointer - state.position;
                registry.begin_drag(region.id, grab_offset);
            }
            break;
        }
    }
}

/// Bring a floating window forward when its shell receives a press.
pub fn focus_floating_gameplay_window_on_ui_press(
    mouse: Res<ButtonInput<MouseButton>>,
    inventory_ui: Res<InventoryUiState>,
    mut registry: ResMut<FloatingGameplayWindowRegistry>,
    roots: Query<(&Interaction, &FloatingGameplayWindowRoot), Changed<Interaction>>,
) {
    if !mouse.just_pressed(MouseButton::Left) || inventory_ui.dragging.is_some() {
        return;
    }
    for (interaction, root) in &roots {
        if *interaction == Interaction::Pressed {
            registry.focus_window(root.id);
        }
    }
}

/// Apply remembered position and z-order to floating window roots.
pub fn sync_floating_gameplay_window_presentation(
    registry: Res<FloatingGameplayWindowRegistry>,
    mut roots: Query<(&FloatingGameplayWindowRoot, &mut Node, &mut ZIndex)>,
) {
    for (root, mut node, mut z_index) in &mut roots {
        let Some(state) = registry.session(root.id) else {
            continue;
        };
        node.left = Val::Px(state.position.x);
        node.top = Val::Px(state.position.y);
        node.bottom = Val::Auto;
        node.right = Val::Auto;
        *z_index = ZIndex(registry.z_index(root.id));
    }
}

/// Update measured window sizes for viewport recovery clamping.
pub fn measure_floating_gameplay_window_sizes(
    mut registry: ResMut<FloatingGameplayWindowRegistry>,
    roots: Query<(&FloatingGameplayWindowRoot, &Node, &Visibility)>,
) {
    for (root, node, visibility) in &roots {
        if *visibility == Visibility::Hidden {
            continue;
        }
        let width = measured_width(node, registry.viewport.x);
        let height = measured_height(node, registry.viewport.y);
        if width <= 1.0 || height <= 1.0 {
            continue;
        }
        let viewport = registry.viewport;
        if let Some(state) = registry.session_mut(root.id) {
            state.computed_size = Vec2::new(width, height);
            state.position = clamp_window_position(state.position, state.computed_size, viewport);
        }
    }
}

fn measured_width(node: &Node, viewport_width: f32) -> f32 {
    match node.width {
        Val::Px(value) => value,
        Val::Percent(pct) => viewport_width * pct / 100.0,
        _ => match node.min_width {
            Val::Px(value) => value,
            _ => 320.0,
        },
    }
}

fn measured_height(node: &Node, viewport_height: f32) -> f32 {
    match node.height {
        Val::Px(value) => value,
        Val::Percent(pct) => viewport_height * pct / 100.0,
        _ => match node.max_height {
            Val::Percent(pct) => viewport_height * pct / 100.0,
            Val::Px(value) => value,
            _ => 400.0,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_drag_changes_window_position() {
        let mut registry = FloatingGameplayWindowRegistry::default();
        let before = registry
            .session(FloatingGameplayWindowId::BuildingMenu)
            .unwrap()
            .position;
        registry.begin_drag(FloatingGameplayWindowId::BuildingMenu, Vec2::new(10.0, 5.0));
        if let Some(state) = registry.session_mut(FloatingGameplayWindowId::BuildingMenu) {
            state.position =
                window_position_from_pointer(Vec2::new(150.0, 90.0), Vec2::new(10.0, 5.0));
        }
        let after = registry
            .session(FloatingGameplayWindowId::BuildingMenu)
            .unwrap()
            .position;
        assert_ne!(after, before);
        assert!((after.x - 140.0).abs() < 0.01);
    }

    #[test]
    fn two_windows_retain_independent_positions() {
        let mut registry = FloatingGameplayWindowRegistry::default();
        registry
            .session_mut(FloatingGameplayWindowId::BuildingMenu)
            .unwrap()
            .position = Vec2::new(10.0, 20.0);
        registry
            .session_mut(FloatingGameplayWindowId::UnitInventory)
            .unwrap()
            .position = Vec2::new(500.0, 80.0);
        assert_ne!(
            registry
                .session(FloatingGameplayWindowId::BuildingMenu)
                .unwrap()
                .position,
            registry
                .session(FloatingGameplayWindowId::UnitInventory)
                .unwrap()
                .position
        );
    }

    #[test]
    fn focus_brings_window_to_front_z_order() {
        let mut registry = FloatingGameplayWindowRegistry::default();
        let before = registry.z_index(FloatingGameplayWindowId::BuildingMenu);
        registry.focus_window(FloatingGameplayWindowId::BuildingMenu);
        let after = registry.z_index(FloatingGameplayWindowId::BuildingMenu);
        assert!(after >= before);
    }
}
