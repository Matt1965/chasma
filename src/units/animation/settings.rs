use bevy::prelude::*;

/// Tunable animation presentation settings (A1/A5).
#[derive(Resource, Debug, Clone, PartialEq, Reflect)]
#[reflect(Resource)]
pub struct UnitAnimationSettings {
    /// Cross-fade duration when switching locomotion clips.
    pub default_blend_ms: u64,
    /// Enter Run when speed >= reference * this ratio (A5 hysteresis).
    pub run_enter_ratio: f32,
    /// Remain Run until speed drops below reference * this ratio (A5 hysteresis).
    pub run_exit_ratio: f32,
    /// Legacy alias for enter ratio (A1 docs).
    pub run_speed_ratio: f32,
    /// Global locomotion playback multiplier.
    pub locomotion_speed_scale: f32,
    /// Per-frame smoothing for playback speed changes (A5).
    pub locomotion_speed_smoothing: f32,
    /// Master enable for unit animation presentation.
    pub enabled: bool,
    /// Hold time after death clip completes (A3).
    pub death_clip_hold_seconds: f32,
    /// Hold time when death clip is missing — freeze pose (A3).
    pub death_freeze_hold_seconds: f32,
    /// Default hit-reaction playback duration when clip length unknown (A3).
    pub hit_reaction_hold_seconds: f32,
    /// Idle turn-in-place threshold in degrees (A5).
    pub turn_in_place_degrees: f32,
    /// Moving heading adjustment threshold in degrees (A5).
    pub turn_adjust_degrees: f32,
    pub turn_blend_ms: u64,
    pub turn_playback_speed: f32,
    pub turn_default_seconds: f32,
    /// Blend when transitioning to idle from movement (A5).
    pub stop_blend_ms: u64,
    pub accel_blend_ms: u64,
    pub decel_blend_ms: u64,
    /// Foot-slide mitigation: alignment below this (deg) is full speed (A5).
    pub foot_slide_min_alignment_degrees: f32,
    /// Max playback slowdown when facing opposes movement (A5).
    pub foot_slide_max_slowdown: f32,
    /// Speed delta below this does not restart locomotion clip (A5).
    pub speed_update_epsilon: f32,
}

impl Default for UnitAnimationSettings {
    fn default() -> Self {
        Self {
            default_blend_ms: 150,
            run_enter_ratio: 0.75,
            run_exit_ratio: 0.65,
            run_speed_ratio: 0.75,
            locomotion_speed_scale: 1.0,
            locomotion_speed_smoothing: 0.35,
            enabled: true,
            death_clip_hold_seconds: 1.5,
            death_freeze_hold_seconds: 0.75,
            hit_reaction_hold_seconds: 0.35,
            turn_in_place_degrees: 35.0,
            turn_adjust_degrees: 55.0,
            turn_blend_ms: 120,
            turn_playback_speed: 1.0,
            turn_default_seconds: 0.55,
            stop_blend_ms: 280,
            accel_blend_ms: 200,
            decel_blend_ms: 240,
            foot_slide_min_alignment_degrees: 20.0,
            foot_slide_max_slowdown: 0.45,
            speed_update_epsilon: 0.04,
        }
    }
}

/// Documented locomotion threshold: Run when unit speed >= reference * this ratio (A1).
pub const DOCUMENTED_RUN_SPEED_RATIO: f32 = 0.75;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_run_ratio_matches_documented_threshold() {
        assert!(
            (UnitAnimationSettings::default().run_enter_ratio - DOCUMENTED_RUN_SPEED_RATIO).abs()
                < f32::EPSILON
        );
        assert!(
            (UnitAnimationSettings::default().run_speed_ratio - DOCUMENTED_RUN_SPEED_RATIO).abs()
                < f32::EPSILON
        );
    }

    #[test]
    fn hysteresis_exit_below_enter() {
        let settings = UnitAnimationSettings::default();
        assert!(settings.run_exit_ratio < settings.run_enter_ratio);
    }
}
