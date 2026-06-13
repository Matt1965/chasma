//! RTS orbit camera input, smoothing, and transform derivation (ADR-014).

use std::f32::consts::TAU;

use bevy::input::keyboard::KeyCode;
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseButton};
use bevy::prelude::*;

use super::components::{RtsCamera, RtsCameraState};
use super::settings::CameraSettings;

/// Systems that drive client-local RTS camera presentation.
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct CameraControlSystems;

/// Horizontal forward (XZ) from yaw: direction the camera looks projected onto XZ.
pub fn yaw_forward_xz(yaw: f32) -> Vec3 {
    Vec3::new(-yaw.sin(), 0.0, -yaw.cos())
}

/// Horizontal right (XZ) from yaw.
pub fn yaw_right_xz(yaw: f32) -> Vec3 {
    Vec3::new(yaw.cos(), 0.0, -yaw.sin())
}

/// Camera world position orbiting `focus` at `distance` with `yaw`/`pitch`.
///
/// `pitch` is elevation above the XZ plane in radians (0 = horizon, π/2 = overhead).
pub fn orbit_position(focus: Vec3, yaw: f32, pitch: f32, distance: f32) -> Vec3 {
    let horizontal = distance * pitch.cos();
    let height = distance * pitch.sin();
    focus
        + Vec3::new(
            horizontal * yaw.sin(),
            height,
            horizontal * yaw.cos(),
        )
}

/// Build a [`Transform`] that looks at `focus` from the orbit pose.
pub fn orbit_transform(focus: Vec3, yaw: f32, pitch: f32, distance: f32) -> Transform {
    let position = orbit_position(focus, yaw, pitch, distance);
    Transform::from_translation(position).looking_at(focus, Vec3::Y)
}

fn normalize_yaw(yaw: f32) -> f32 {
    yaw.rem_euclid(TAU)
}

fn exp_smooth(current: f32, target: f32, rate: f32, dt: f32) -> f32 {
    let alpha = 1.0 - (-rate * dt).exp();
    current + (target - current) * alpha
}

fn exp_smooth_vec3(current: Vec3, target: Vec3, rate: f32, dt: f32) -> Vec3 {
    let alpha = 1.0 - (-rate * dt).exp();
    current.lerp(target, alpha)
}

pub fn apply_rts_camera_control(
    time: Res<Time>,
    settings: Res<CameraSettings>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    mouse_scroll: Res<AccumulatedMouseScroll>,
    mut query: Query<(&mut RtsCameraState, &mut Transform), With<RtsCamera>>,
) {
    let dt = time.delta_secs().min(settings.max_frame_delta);

    let Ok((mut state, mut transform)) = query.single_mut() else {
        return;
    };

    // --- Pan (XZ, relative to camera yaw) ---
    let mut pan = Vec2::ZERO;
    if keys.pressed(KeyCode::KeyW) {
        pan.y += 1.0;
    }
    if keys.pressed(KeyCode::KeyS) {
        pan.y -= 1.0;
    }
    if keys.pressed(KeyCode::KeyA) {
        pan.x -= 1.0;
    }
    if keys.pressed(KeyCode::KeyD) {
        pan.x += 1.0;
    }

    if pan != Vec2::ZERO {
        let direction = pan.normalize();
        let speed = if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) {
            settings.pan_speed * settings.fast_pan_multiplier
        } else {
            settings.pan_speed
        };
        let forward = yaw_forward_xz(state.target_yaw);
        let right = yaw_right_xz(state.target_yaw);
        let delta = (forward * direction.y + right * direction.x) * speed * dt;
        state.target_focus += Vec3::new(delta.x, 0.0, delta.z);
    }

    // --- Rotate (middle mouse drag) ---
    if mouse_buttons.pressed(MouseButton::Middle) {
        let delta = mouse_motion.delta;
        state.target_yaw = normalize_yaw(
            state.target_yaw - delta.x * settings.rotate_sensitivity,
        );
        state.target_pitch = settings.clamp_pitch(
            state.target_pitch + delta.y * settings.rotate_sensitivity,
        );
    }

    // --- Zoom (mouse wheel) ---
    let scroll_y = mouse_scroll.delta.y + mouse_scroll.delta.x;
    if scroll_y.abs() > f32::EPSILON {
        let factor = 1.0 - scroll_y * settings.zoom_speed;
        state.target_distance = settings.clamp_distance(state.target_distance * factor);
    }

    // --- Smooth toward targets ---
    let rate = settings.smoothing;
    state.focus = exp_smooth_vec3(state.focus, state.target_focus, rate, dt);
    state.yaw = exp_smooth_angle(state.yaw, state.target_yaw, rate, dt);
    state.pitch = exp_smooth(state.pitch, state.target_pitch, rate, dt);
    state.distance = exp_smooth(state.distance, state.target_distance, rate, dt);

    // Keep targets in sync with clamped/smoothed canonical values for yaw wrap.
    state.target_yaw = normalize_yaw(state.target_yaw);
    state.target_pitch = settings.clamp_pitch(state.target_pitch);
    state.target_distance = settings.clamp_distance(state.target_distance);

    *transform = orbit_transform(state.focus, state.yaw, state.pitch, state.distance);
}

/// Smooth angle taking the shortest path across the yaw wrap.
fn exp_smooth_angle(current: f32, target: f32, rate: f32, dt: f32) -> f32 {
    let current = normalize_yaw(current);
    let target = normalize_yaw(target);
    let mut delta = target - current;
    if delta > core::f32::consts::PI {
        delta -= TAU;
    } else if delta < -core::f32::consts::PI {
        delta += TAU;
    }
    let alpha = 1.0 - (-rate * dt).exp();
    normalize_yaw(current + delta * alpha)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::FRAC_PI_2;

    #[test]
    fn orbit_position_is_above_focus_for_positive_pitch() {
        let focus = Vec3::new(100.0, 0.0, 50.0);
        let pos = orbit_position(focus, 0.0, 0.5, 100.0);
        assert!(pos.y > focus.y);
    }

    #[test]
    fn orbit_transform_looks_at_focus() {
        let focus = Vec3::new(0.0, 0.0, 0.0);
        let transform = orbit_transform(focus, 1.2, 0.6, 200.0);
        let forward = transform.forward().as_vec3();
        let to_focus = (focus - transform.translation).normalize();
        assert!((forward - to_focus).length() < 1e-4);
    }

    #[test]
    fn pan_axes_are_perpendicular_on_xz() {
        let forward = yaw_forward_xz(0.3);
        let right = yaw_right_xz(0.3);
        assert_eq!(forward.y, 0.0);
        assert_eq!(right.y, 0.0);
        assert!(forward.dot(right).abs() < 1e-5);
        assert!((forward.length() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn full_yaw_rotation_wraps() {
        let yaw = normalize_yaw(TAU + 0.5);
        assert!((yaw - 0.5).abs() < 1e-5);
    }

    #[test]
    fn overhead_pitch_places_camera_high() {
        let focus = Vec3::ZERO;
        let pos = orbit_position(focus, 0.0, FRAC_PI_2 - 0.1, 100.0);
        assert!(pos.y > 90.0);
        assert!(pos.x.abs() < 15.0 && pos.z.abs() < 15.0);
    }
}
