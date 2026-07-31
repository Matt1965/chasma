//! Building opacity slider for Navigation Editor (presentation-only).

use bevy::ecs::system::ParamSet;
use bevy::input::mouse::MouseButton;
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

use crate::dev::dev_mode::DevModeState;
use crate::dev::widgets::{
    DevSliderDragState, DevWidgetSliderTrack, DevWidgetSliderValue, normalized_to_value,
    slider_normalized_x, sync_slider_fill, value_to_normalized,
};
use crate::dev::window::DevWindowRegistry;

use super::state::{
    DEFAULT_NAV_EDITOR_BUILDING_OPACITY, NavigationEditorUiState, navigation_editor_owns_session,
};
use crate::dev::inspector::BlueprintInspectionState;

pub const NAV_EDITOR_BUILDING_OPACITY_FIELD_ID: u32 = 901;

/// Push the authoritative opacity value into the shared slider widget.
///
/// Row visibility is owned by [`super::panel::sync_navigation_editor_panel`];
/// keeping it there avoids a second broad `&mut Node` query alongside the
/// shared slider-fill query.
pub fn sync_navigation_editor_opacity_slider(
    dev_state: Res<DevModeState>,
    registry: Res<DevWindowRegistry>,
    inspection: Res<BlueprintInspectionState>,
    ui_state: Res<NavigationEditorUiState>,
    mut tracks: Query<(&DevWidgetSliderTrack, &Children)>,
    mut fills: Query<&mut Node, Without<DevWidgetSliderTrack>>,
    mut values: Query<(&DevWidgetSliderValue, &mut Text)>,
) {
    if !navigation_editor_owns_session(dev_state.enabled, &registry, &inspection) {
        return;
    }

    let opacity = ui_state.building_opacity.clamp(0.0, 1.0);
    let display = format!("{}%", (opacity * 100.0).round() as i32);
    sync_slider_fill(
        NAV_EDITOR_BUILDING_OPACITY_FIELD_ID,
        value_to_normalized(opacity, 0.0, 1.0),
        &display,
        &mut tracks,
        &mut fills,
        &mut values,
    );
}

pub fn handle_navigation_editor_opacity_slider(
    dev_state: Res<DevModeState>,
    registry: Res<DevWindowRegistry>,
    inspection: Res<BlueprintInspectionState>,
    mut ui_state: ResMut<NavigationEditorUiState>,
    mut drag: ResMut<DevSliderDragState>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut gate: ResMut<crate::dev::DevModeInputGate>,
    mut interactions: ParamSet<(
        Query<
            (&Interaction, &DevWidgetSliderValue),
            (Changed<Interaction>, Without<DevWidgetSliderTrack>),
        >,
        Query<
            (&DevWidgetSliderTrack, &Interaction, &RelativeCursorPosition),
            Without<DevWidgetSliderValue>,
        >,
    )>,
) {
    if !navigation_editor_owns_session(dev_state.enabled, &registry, &inspection) {
        drag.field_id = None;
        return;
    }

    for (interaction, value_btn) in interactions.p0().iter() {
        if value_btn.id != NAV_EDITOR_BUILDING_OPACITY_FIELD_ID {
            continue;
        }
        if *interaction == Interaction::Pressed {
            gate.block_gameplay_mouse = true;
            gate.block_camera_scroll = true;
        }
    }

    if mouse.just_pressed(MouseButton::Left) {
        for (track, interaction, relative) in interactions.p1().iter() {
            if track.id != NAV_EDITOR_BUILDING_OPACITY_FIELD_ID {
                continue;
            }
            if *interaction != Interaction::Pressed {
                continue;
            }
            drag.field_id = Some(track.id);
            gate.block_gameplay_mouse = true;
            gate.block_camera_input = true;
            gate.block_camera_scroll = true;
            if let Some(norm) = slider_normalized_x(relative) {
                ui_state.building_opacity = normalized_to_value(norm, 0.0, 1.0).clamp(0.0, 1.0);
            }
        }
    }

    if mouse.just_released(MouseButton::Left) {
        drag.field_id = None;
    }

    let Some(field_id) = drag.field_id else {
        return;
    };
    if field_id != NAV_EDITOR_BUILDING_OPACITY_FIELD_ID {
        return;
    }

    for (track, _interaction, relative) in interactions.p1().iter() {
        if track.id != field_id {
            continue;
        }
        let Some(norm) = slider_normalized_x(relative) else {
            continue;
        };
        ui_state.building_opacity = normalized_to_value(norm, 0.0, 1.0).clamp(0.0, 1.0);
        gate.block_camera_input = true;
        gate.block_camera_scroll = true;
        gate.block_gameplay_mouse = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_opacity_is_editor_readable() {
        assert!(
            (DEFAULT_NAV_EDITOR_BUILDING_OPACITY - 0.42).abs() < f32::EPSILON,
            "expected ~42% default opacity"
        );
    }
}
