use bevy::prelude::*;

/// Tunable RTS orbit camera parameters (ADR-014).
///
/// Lives entirely in the Camera layer. Initial pose defaults are presentation
/// choices, not authoritative world data.
#[derive(Debug, Clone, Resource, Reflect)]
#[reflect(Resource)]
pub struct CameraSettings {
    /// World-space point the camera looks at.
    pub initial_focus: Vec3,
    /// Initial yaw in radians (rotation around world Y).
    pub initial_yaw: f32,
    /// Initial pitch in radians (elevation above the XZ plane).
    pub initial_pitch: f32,
    /// Initial orbit distance in world units.
    pub initial_distance: f32,

    /// Minimum pitch (radians above XZ). Prevents horizon-grazing views.
    pub pitch_min: f32,
    /// Maximum pitch (radians above XZ). Prevents straight-down gimbal lock.
    pub pitch_max: f32,

    /// Minimum orbit distance (world units).
    pub distance_min: f32,
    /// Maximum orbit distance (world units).
    pub distance_max: f32,

    /// WASD pan speed in meters per second at distance reference.
    pub pan_speed: f32,
    /// Multiplier applied while Shift is held.
    pub fast_pan_multiplier: f32,

    /// Mouse rotation sensitivity (radians per pixel) while middle mouse is held.
    pub rotate_sensitivity: f32,

    /// Zoom scale per wheel line step (multiplicative).
    pub zoom_speed: f32,

    /// Exponential smoothing rate for focus/yaw/pitch/distance convergence.
    pub smoothing: f32,

    /// Upper bound on frame delta used by camera systems (seconds).
    pub max_frame_delta: f32,
}

impl Default for CameraSettings {
    fn default() -> Self {
        Self {
            // Center of the committed Phase 2A sample patch (two 256 m chunks).
            initial_focus: Vec3::new(256.0, 0.0, 128.0),
            initial_yaw: 0.0,
            initial_pitch: 0.55,
            initial_distance: 420.0,

            pitch_min: 0.15,
            pitch_max: 1.35,
            distance_min: 40.0,
            distance_max: 5_000.0,

            pan_speed: 256.0,
            fast_pan_multiplier: 2.5,

            rotate_sensitivity: 0.004,

            zoom_speed: 0.12,

            smoothing: 12.0,
            max_frame_delta: 0.1,
        }
    }
}

impl CameraSettings {
    pub fn clamp_pitch(&self, pitch: f32) -> f32 {
        pitch.clamp(self.pitch_min, self.pitch_max)
    }

    pub fn clamp_distance(&self, distance: f32) -> f32 {
        distance.clamp(self.distance_min, self.distance_max)
    }
}
