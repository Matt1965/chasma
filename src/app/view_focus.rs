//! App-layer bridge from camera presentation to view focus (ADR-012, ADR-014).

use bevy::prelude::*;

use crate::camera::{RtsCamera, RtsCameraState};
use crate::view::PrimaryViewFocus;

/// Publishes the RTS camera focus into [`PrimaryViewFocus`].
pub fn publish_primary_view_focus(
    camera: Query<&RtsCameraState, With<RtsCamera>>,
    mut focus: ResMut<PrimaryViewFocus>,
) {
    let Ok(state) = camera.single() else {
        return;
    };
    focus.position = state.focus;
}
