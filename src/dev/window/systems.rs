//! Dev-window systems — drag, focus, visibility, input blocking (Slice 3).

use bevy::ecs::query::Or;
use bevy::ecs::system::ParamSet;
use bevy::input::mouse::MouseButton;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use super::components::{
    DevLauncherGroup, DevWindowBody, DevWindowCloseButton, DevWindowCollapseButton,
    DevWindowCollapseButtonLabel, DevWindowRoot, DevWindowTitleBarDragRegion, DevWindowUi,
    DevWorkspaceLauncher, DevWorkspaceLauncherButton, DevWorkspaceLauncherButtons,
    DevWorkspaceLauncherToggle,
};
use super::id::DevWindowId;
use super::math::{TITLE_BAR_HEIGHT_PX, clamp_window_position, window_position_from_pointer};
use super::state::{DevWindowInteractionState, DevWindowRegistry};
use crate::dev::NavigationEditorBlockedAction;
use crate::dev::dev_mode::{DevModeInputGate, DevModeState};
use crate::dev::input::{DevPanelHoverState, DevPanelRoot};

/// Track viewport size and re-clamp windows on resize.
pub fn sync_dev_window_viewport(
    windows: Query<&Window, With<PrimaryWindow>>,
    mut registry: ResMut<DevWindowRegistry>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let viewport = Vec2::new(window.width(), window.height());
    registry.set_viewport(viewport);
}

/// Detect hover over dev-window UI and launcher.
pub fn update_dev_window_interaction_state(
    dev_state: Res<DevModeState>,
    registry: Res<DevWindowRegistry>,
    mut interactions: ParamSet<(
        Query<
            &Interaction,
            (
                Or<(With<DevWindowUi>, With<crate::dev::input::DevPanelUi>)>,
                Without<DevWorkspaceLauncher>,
                Without<DevWorkspaceLauncherButton>,
                Without<DevWorkspaceLauncherToggle>,
            ),
        >,
        Query<
            &Interaction,
            Or<(
                With<DevWorkspaceLauncher>,
                With<DevWorkspaceLauncherButton>,
                With<DevWorkspaceLauncherToggle>,
            )>,
        >,
    )>,
    mut interaction_state: ResMut<DevWindowInteractionState>,
) {
    if !dev_state.enabled {
        *interaction_state = DevWindowInteractionState::default();
        return;
    }

    interaction_state.any_window_hovered = interactions
        .p0()
        .iter()
        .any(|state| *state != Interaction::None);
    interaction_state.launcher_hovered = interactions
        .p1()
        .iter()
        .any(|state| *state != Interaction::None);
    interaction_state.dragging = registry.drag.is_some();
}

/// Compatibility mirror — single derived writer for legacy [`DevPanelHoverState`].
pub fn sync_dev_panel_hover_from_windows(
    interaction: Res<DevWindowInteractionState>,
    mut panel_hovered: ResMut<DevPanelHoverState>,
) {
    panel_hovered.hovered = interaction.blocks_world_mouse();
}

/// Apply window interaction to the dev input gate (after per-handler mutations).
pub fn apply_dev_window_input_gate(
    interaction: Res<DevWindowInteractionState>,
    mut gate: ResMut<DevModeInputGate>,
) {
    if interaction.blocks_world_mouse() {
        gate.block_gameplay_mouse = true;
    }
    if interaction.blocks_camera() {
        gate.block_camera_input = true;
    }
    if interaction.any_window_hovered || interaction.launcher_hovered {
        gate.block_camera_scroll = true;
    }
}

/// Title-bar drag, close/collapse buttons, launcher reopen, focus-on-click.
pub fn handle_dev_window_pointer(
    mut dev_state: ResMut<DevModeState>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut registry: ResMut<DevWindowRegistry>,
    mut gate: ResMut<DevModeInputGate>,
    mut blueprint_inspection: ResMut<crate::dev::BlueprintInspectionState>,
    mut nav_ui: ResMut<crate::dev::NavigationEditorUiState>,
    ui_inventory: Res<crate::ui::gameplay::InventoryUiState>,
    mut interactions: ParamSet<(
        Query<(&Interaction, &DevWindowTitleBarDragRegion)>,
        Query<(&Interaction, &DevWindowCloseButton), Changed<Interaction>>,
        Query<(&Interaction, &DevWindowCollapseButton), Changed<Interaction>>,
        Query<(&Interaction, &DevWorkspaceLauncherButton), Changed<Interaction>>,
        Query<(&Interaction, &DevWorkspaceLauncherToggle), Changed<Interaction>>,
    )>,
) {
    if !dev_state.enabled {
        if registry.drag.is_some() {
            registry.cancel_drag();
        }
        return;
    }

    let Ok(window) = windows.single() else {
        return;
    };
    let pointer = window.cursor_position().unwrap_or(Vec2::ZERO);

    for (interaction, toggle) in interactions.p4().iter() {
        if *interaction == Interaction::Pressed {
            gate.block_gameplay_mouse = true;
            match toggle.group {
                DevLauncherGroup::Windows => {
                    registry.launcher_expanded = !registry.launcher_expanded;
                }
                DevLauncherGroup::Advanced => {
                    registry.advanced_launcher_expanded = !registry.advanced_launcher_expanded;
                }
            }
        }
    }

    for (interaction, button) in interactions.p3().iter() {
        if *interaction == Interaction::Pressed {
            gate.block_gameplay_mouse = true;
            registry.toggle(button.window);
        }
    }

    for (interaction, button) in interactions.p1().iter() {
        if *interaction == Interaction::Pressed {
            if button.id == DevWindowId::NavigationEditor
                && blueprint_inspection.editing
                && blueprint_inspection.dirty
            {
                nav_ui.pending_blocked_action = Some(NavigationEditorBlockedAction::CloseWindow);
                blueprint_inspection.pending_confirmation = Some(
                    crate::dev::inspector::BlueprintPendingConfirmation::DiscardEdits {
                        action: "close Navigation Editor".into(),
                    },
                );
                continue;
            }
            registry.hide(button.id);
            dev_state.clear_text_focus();
        }
    }

    for (interaction, button) in interactions.p2().iter() {
        if *interaction == Interaction::Pressed {
            if let Some(state) = registry.session_mut(button.id) {
                state.collapsed = !state.collapsed;
            }
        }
    }

    if let Some(drag) = registry.drag {
        if mouse.pressed(MouseButton::Left) {
            if let Some(state) = registry.session_mut(drag.window) {
                state.position = window_position_from_pointer(pointer, drag.grab_offset);
                gate.block_gameplay_mouse = true;
                gate.block_camera_input = true;
            }
        } else {
            let viewport = registry.viewport;
            if let Some(state) = registry.session_mut(drag.window) {
                state.position =
                    clamp_window_position(state.position, state.computed_size, viewport);
            }
            registry.end_drag();
            gate.block_gameplay_mouse = true;
        }
        return;
    }

    if mouse.just_pressed(MouseButton::Left) {
        if ui_inventory.dragging.is_some() {
            return;
        }
        for (interaction, region) in interactions.p0().iter() {
            if *interaction == Interaction::Pressed {
                registry.focus_window(region.id);
                if let Some(state) = registry.session(region.id) {
                    let grab_offset = pointer - state.position;
                    registry.begin_drag(region.id, grab_offset);
                    gate.block_gameplay_mouse = true;
                    gate.block_camera_input = true;
                }
                break;
            }
        }
    }
}

/// Bring the dev window forward when any panel control is pressed.
pub fn focus_dev_window_on_panel_press(
    dev_state: Res<DevModeState>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut registry: ResMut<DevWindowRegistry>,
    mut interactions: ParamSet<(
        Query<
            &Interaction,
            (
                With<crate::dev::selected_object::DevSelectedObjectActionButton>,
                Changed<Interaction>,
            ),
        >,
        Query<
            &Interaction,
            (
                With<crate::dev::selected_object::DevSelectedObjectToggleButton>,
                Changed<Interaction>,
            ),
        >,
        Query<&Interaction, (With<DevPanelRoot>, Changed<Interaction>)>,
        Query<
            &Interaction,
            (
                With<crate::dev::save_window::DevSaveWindowUi>,
                Changed<Interaction>,
            ),
        >,
    )>,
) {
    if !dev_state.enabled || !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    if interactions.p0().iter().any(|i| *i == Interaction::Pressed)
        || interactions.p1().iter().any(|i| *i == Interaction::Pressed)
    {
        registry.focus_window(DevWindowId::SelectedObject);
        return;
    }
    if interactions
        .p3()
        .iter()
        .any(|interaction| *interaction == Interaction::Pressed)
    {
        registry.focus_window(DevWindowId::Save);
        return;
    }
    if interactions
        .p2()
        .iter()
        .any(|interaction| *interaction == Interaction::Pressed)
    {
        registry.focus_window(DevWindowId::Catalog);
    }
}

/// Bring window shell to front when its root receives a press.
pub fn focus_dev_window_on_ui_press(
    dev_state: Res<DevModeState>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut registry: ResMut<DevWindowRegistry>,
    roots: Query<(&Interaction, &DevWindowRoot), Changed<Interaction>>,
) {
    if !dev_state.enabled || !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    for (interaction, root) in &roots {
        if *interaction == Interaction::Pressed {
            registry.focus_window(root.id);
        }
    }
}

/// Sync window transforms, visibility, Z-order, and collapsed bodies.
pub fn sync_dev_window_presentation(
    dev_state: Res<DevModeState>,
    registry: Res<DevWindowRegistry>,
    mut visibility_queries: ParamSet<(
        Query<(&mut Node, &mut Visibility), (With<DevWorkspaceLauncher>, Without<DevWindowRoot>)>,
        Query<
            (&DevWindowRoot, &mut Node, &mut ZIndex, &mut Visibility),
            Without<DevWorkspaceLauncher>,
        >,
        Query<(&DevWindowBody, &mut Visibility)>,
        Query<(&DevWorkspaceLauncherButtons, &mut Node)>,
        Query<(&DevWindowCollapseButtonLabel, &mut Text)>,
    )>,
) {
    let workspace_visible = dev_state.enabled;

    for (mut node, mut visibility) in visibility_queries.p0().iter_mut() {
        *visibility = if workspace_visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        if workspace_visible {
            node.display = Display::Flex;
        }
    }

    for (root, mut node, mut z_index, mut visibility) in visibility_queries.p1().iter_mut() {
        let Some(state) = registry.session(root.id) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let show = workspace_visible && state.visible;
        *visibility = if show {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        if show {
            node.left = Val::Px(state.position.x);
            node.top = Val::Px(state.position.y);
            *z_index = ZIndex(registry.z_index(root.id));
            node.display = Display::Flex;
            if state.collapsed && root.id.supports_collapse() {
                node.height = Val::Px(TITLE_BAR_HEIGHT_PX);
                node.overflow = Overflow::clip();
            } else {
                node.height = Val::Auto;
                node.overflow = Overflow::visible();
            }
        } else {
            node.display = Display::None;
        }
    }

    for (body, mut visibility) in visibility_queries.p2().iter_mut() {
        let collapsed = registry
            .session(body.id)
            .is_some_and(|state| state.collapsed);
        let show_body = workspace_visible && registry.is_visible(body.id) && !collapsed;
        *visibility = if show_body {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }

    for (buttons, mut node) in visibility_queries.p3().iter_mut() {
        if !workspace_visible {
            node.display = Display::None;
            continue;
        }
        node.display = match buttons.group {
            DevLauncherGroup::Windows => {
                if registry.launcher_expanded {
                    Display::Flex
                } else {
                    Display::None
                }
            }
            DevLauncherGroup::Advanced => {
                if registry.advanced_launcher_expanded {
                    Display::Flex
                } else {
                    Display::None
                }
            }
        };
    }

    for (label, mut text) in visibility_queries.p4().iter_mut() {
        let collapsed = registry
            .session(label.id)
            .is_some_and(|state| state.collapsed);
        **text = if collapsed { "+" } else { "-" }.into();
    }
}

/// Update stored computed sizes from layout when available.
pub fn sync_dev_window_computed_sizes(
    mut registry: ResMut<DevWindowRegistry>,
    roots: Query<(&DevWindowRoot, &ComputedNode)>,
) {
    for (root, computed) in &roots {
        if let Some(state) = registry.session_mut(root.id) {
            let size = computed.size();
            if size.x > 1.0 && size.y > 1.0 {
                state.computed_size = size;
            }
        }
    }
}

/// Cancel drag and clear focus when dev mode is disabled.
pub fn handle_dev_mode_window_lifecycle(
    mut dev_state: ResMut<DevModeState>,
    mut registry: ResMut<DevWindowRegistry>,
    mut tooltip: ResMut<crate::dev::tooltip::DevTooltipState>,
    mut nav_ui: ResMut<crate::dev::NavigationEditorUiState>,
) {
    if dev_state.is_changed() && !dev_state.enabled {
        registry.cancel_drag();
        registry.launcher_expanded = true;
        dev_state.clear_text_focus();
        tooltip.hide();
        nav_ui.reset_session_presentation();
    }
}
