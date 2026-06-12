use bevy::prelude::*;

/// Marks the main client RTS camera entity (ADR-014).
#[derive(Component, Debug, Clone, Copy, PartialEq, Reflect)]
#[reflect(Component)]
pub struct RtsCamera;

/// Client-local orbit camera state (ADR-014).
///
/// Current values are smoothed toward targets each frame. The camera
/// [`Transform`] is derived from the current focus/yaw/pitch/distance only.
#[derive(Component, Debug, Clone, Copy, PartialEq, Reflect)]
#[reflect(Component)]
pub struct RtsCameraState {
    pub focus: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub target_focus: Vec3,
    pub target_yaw: f32,
    pub target_pitch: f32,
    pub target_distance: f32,
}

impl RtsCameraState {
    pub fn new(focus: Vec3, yaw: f32, pitch: f32, distance: f32) -> Self {
        Self {
            focus,
            yaw,
            pitch,
            distance,
            target_focus: focus,
            target_yaw: yaw,
            target_pitch: pitch,
            target_distance: distance,
        }
    }
}
