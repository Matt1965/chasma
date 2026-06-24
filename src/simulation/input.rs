//! Global simulation pause keyboard bindings (ADR-046).

use bevy::input::keyboard::KeyCode;
use bevy::prelude::*;

use super::control::SimulationControlState;

/// Space toggles pause/resume; Shift+Space steps one simulation tick.
pub fn handle_simulation_keyboard(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut control: ResMut<SimulationControlState>,
) {
    let shift = keyboard.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);

    if keyboard.just_pressed(KeyCode::Space) {
        if shift {
            control.request_step_once();
        } else {
            control.toggle_pause();
        }
    }
}
