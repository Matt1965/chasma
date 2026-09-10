use bevy::prelude::*;

/// Minimum RTS orbit distance in world meters (ADR-014).
///
/// Chosen for close building / navigation-corner inspection while staying above the
/// terrain eye clearance clamp and avoiding near-plane clipping.
pub const CAMERA_ORBIT_DISTANCE_MIN_METERS: f32 = 2.0;

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

    /// WASD pan speed in meters per second at [`initial_distance`].
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

    /// Minimum height of the camera eye above resident terrain (render units).
    pub terrain_clearance: f32,
    /// Vertical offset applied when gluing orbit focus to the terrain surface.
    pub focus_terrain_offset: f32,
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
            distance_min: CAMERA_ORBIT_DISTANCE_MIN_METERS,
            distance_max: 5_000.0,

            pan_speed: 256.0,
            fast_pan_multiplier: 2.5,

            rotate_sensitivity: 0.004,

            zoom_speed: 0.12,

            smoothing: 12.0,
            max_frame_delta: 0.1,

            terrain_clearance: 2.0,
            focus_terrain_offset: 0.0,
        }
    }
}

/// Scale WASD pan speed by orbit distance relative to a reference distance.
///
/// At `reference_distance` the scale is ~1. Closer distances reduce speed
/// proportionally; farther distances do not exceed the baseline (`scale <= 1`).
pub fn pan_speed_scale(distance: f32, reference_distance: f32) -> f32 {
    if reference_distance <= f32::EPSILON {
        return 1.0;
    }
    (distance / reference_distance).clamp(0.0, 1.0)
}

impl CameraSettings {
    pub fn clamp_pitch(&self, pitch: f32) -> f32 {
        pitch.clamp(self.pitch_min, self.pitch_max)
    }

    pub fn clamp_distance(&self, distance: f32) -> f32 {
        distance.clamp(self.distance_min, self.distance_max)
    }

    /// WASD pan speed at the given smoothed orbit distance.
    pub fn pan_speed_at_distance(&self, smoothed_distance: f32) -> f32 {
        let distance = self.clamp_distance(smoothed_distance);
        self.pan_speed * pan_speed_scale(distance, self.initial_distance)
    }

    /// WASD pan speed after zoom scaling and optional Shift fast-pan.
    pub fn effective_pan_speed(&self, smoothed_distance: f32, shift_held: bool) -> f32 {
        let speed = self.pan_speed_at_distance(smoothed_distance);
        if shift_held {
            speed * self.fast_pan_multiplier
        } else {
            speed
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimum_orbit_distance_allows_close_inspection() {
        let settings = CameraSettings::default();
        assert_eq!(settings.distance_min, CAMERA_ORBIT_DISTANCE_MIN_METERS);
        assert!(settings.distance_min < 40.0);
        assert_eq!(
            settings.clamp_distance(5.0),
            CAMERA_ORBIT_DISTANCE_MIN_METERS
        );
        assert_eq!(settings.distance_max, 5_000.0);
    }

    #[test]
    fn pan_speed_scale_is_one_at_reference_distance() {
        let settings = CameraSettings::default();
        let scale = pan_speed_scale(settings.initial_distance, settings.initial_distance);
        assert!((scale - 1.0).abs() < 1e-5);
        assert!(
            (settings.pan_speed_at_distance(settings.initial_distance) - settings.pan_speed).abs()
                < 1e-3
        );
    }

    #[test]
    fn pan_speed_scale_decreases_when_zoomed_in() {
        let settings = CameraSettings::default();
        let reference = settings.initial_distance;
        let closer = reference * 0.5;
        let scale = pan_speed_scale(closer, reference);
        assert!((scale - 0.5).abs() < 1e-5);
        assert!(settings.pan_speed_at_distance(closer) < settings.pan_speed);
    }

    #[test]
    fn pan_speed_scale_at_minimum_distance_is_much_slower_than_reference() {
        let settings = CameraSettings::default();
        let min_scale = pan_speed_scale(settings.distance_min, settings.initial_distance);
        let min_speed = settings.pan_speed_at_distance(settings.distance_min);
        assert!(min_scale < 0.05);
        assert!(min_speed < settings.pan_speed * 0.05);
    }

    #[test]
    fn pan_speed_scale_does_not_exceed_baseline_when_zoomed_out() {
        let settings = CameraSettings::default();
        let farther = settings.initial_distance * 2.0;
        let scale = pan_speed_scale(farther, settings.initial_distance);
        assert_eq!(scale, 1.0);
        assert_eq!(settings.pan_speed_at_distance(farther), settings.pan_speed);
    }

    #[test]
    fn shift_multiplies_zoom_scaled_pan_speed() {
        let settings = CameraSettings::default();
        let distance = 200.0;
        let base = settings.pan_speed_at_distance(distance);
        let fast = settings.effective_pan_speed(distance, true);
        assert!((fast - base * settings.fast_pan_multiplier).abs() < 1e-3);
        assert_eq!(
            settings.effective_pan_speed(distance, false),
            settings.pan_speed_at_distance(distance)
        );
    }
}
