use bevy::prelude::*;

use super::components::{RtsCamera, RtsCameraState};
use super::control::orbit_transform;
use super::settings::CameraSettings;

/// Spawn the main client RTS camera (ADR-014).
pub fn spawn_rts_camera(mut commands: Commands, settings: Res<CameraSettings>) {
    let yaw = settings.initial_yaw;
    let pitch = settings.clamp_pitch(settings.initial_pitch);
    let distance = settings.clamp_distance(settings.initial_distance);
    let focus = settings.initial_focus;

    let state = RtsCameraState::new(focus, yaw, pitch, distance);
    let transform = orbit_transform(focus, yaw, pitch, distance);

    commands.spawn((
        Camera3d::default(),
        RtsCamera,
        state,
        transform,
    ));
}
