//! Regression tests for dev window pointer capture vs world input.

use super::state::DevWindowInteractionState;
use crate::dev::input::DevPanelHoverState;

#[test]
fn panel_hover_mirrors_window_rect_capture() {
    let interaction = DevWindowInteractionState {
        pointer_inside_window_bounds: true,
        ..Default::default()
    };
    let panel = DevPanelHoverState {
        hovered: interaction.blocks_world_mouse(),
    };
    assert!(panel.hovered);
}

#[test]
fn panel_hover_clear_outside_ui() {
    let interaction = DevWindowInteractionState::default();
    let panel = DevPanelHoverState {
        hovered: interaction.blocks_world_mouse(),
    };
    assert!(!panel.hovered);
}

#[test]
fn widget_hover_still_blocks() {
    let interaction = DevWindowInteractionState {
        any_window_hovered: true,
        ..Default::default()
    };
    assert!(interaction.blocks_world_mouse());
}
