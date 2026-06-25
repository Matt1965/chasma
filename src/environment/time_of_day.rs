//! Visual time-of-day settings (ADR-052 E10).
//!
//! Client-local presentation clock — not gameplay simulation time.

use bevy::prelude::*;

/// Visual day-night cycle configuration (Environment layer only).
#[derive(Debug, Clone, Resource, Reflect)]
#[reflect(Resource)]
pub struct TimeOfDaySettings {
    /// When false, the cycle does not advance or mutate [`super::settings::EnvironmentSettings`].
    pub enabled: bool,
    /// Current clock hour in `[0.0, 24.0)`.
    pub time_hours: f32,
    /// Real-time seconds for one full 24-hour visual cycle.
    pub day_length_seconds: f32,
    /// When true, [`Self::time_hours`] does not advance.
    pub paused: bool,
    /// Sun elevation at night horizon / solar noon (degrees).
    pub sun_pitch_min_deg: f32,
    pub sun_pitch_max_deg: f32,
    /// Hours when direct sunlight begins / ends (visual twilight model).
    pub sunrise_hour: f32,
    pub sunset_hour: f32,
    /// Ambient brightness multiplier at full night (relative to noon ambient).
    pub night_ambient_multiplier: f32,
    /// Directional illuminance (lux) at solar noon and deep night.
    pub noon_directional_illuminance: f32,
    pub night_directional_illuminance: f32,
    /// Ambient and skybox brightness at noon and night (before night ambient multiplier).
    pub noon_ambient_brightness: f32,
    pub noon_skybox_brightness: f32,
    pub night_skybox_brightness: f32,
    /// Extra daylight factor from twilight warmth at dawn/dusk.
    pub twilight_daylight_blend: f32,
}

impl Default for TimeOfDaySettings {
    fn default() -> Self {
        Self {
            enabled: true,
            time_hours: 12.0,
            day_length_seconds: 600.0,
            paused: false,
            sun_pitch_min_deg: -12.0,
            sun_pitch_max_deg: 58.0,
            sunrise_hour: 6.0,
            sunset_hour: 18.0,
            night_ambient_multiplier: 0.32,
            noon_directional_illuminance: 24_000.0,
            night_directional_illuminance: 40.0,
            noon_ambient_brightness: 320.0,
            noon_skybox_brightness: 1_200.0,
            night_skybox_brightness: 160.0,
            twilight_daylight_blend: 0.5,
        }
    }
}

impl TimeOfDaySettings {
    /// Wrap `time_hours` into `[0.0, 24.0)`.
    pub fn normalize_hours(hours: f32) -> f32 {
        let wrapped = hours % 24.0;
        if wrapped < 0.0 {
            wrapped + 24.0
        } else {
            wrapped
        }
    }

    pub fn set_time_hours(&mut self, hours: f32) {
        self.time_hours = Self::normalize_hours(hours);
    }

    /// Advance the visual clock by real-time `delta_seconds` when active.
    pub fn advance(&mut self, delta_seconds: f32) {
        if !self.enabled || self.paused || self.day_length_seconds <= 0.0 {
            return;
        }
        let hours_per_second = 24.0 / self.day_length_seconds;
        self.set_time_hours(self.time_hours + delta_seconds * hours_per_second);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_wraps_after_24h() {
        let mut settings = TimeOfDaySettings {
            day_length_seconds: 24.0,
            ..Default::default()
        };
        settings.time_hours = 23.0;
        settings.advance(1.0);
        assert!((settings.time_hours - 0.0).abs() < 1e-4);
        settings.set_time_hours(25.5);
        assert!((settings.time_hours - 1.5).abs() < 1e-4);
        settings.set_time_hours(-1.0);
        assert!((settings.time_hours - 23.0).abs() < 1e-4);
    }

    #[test]
    fn paused_time_does_not_advance() {
        let mut settings = TimeOfDaySettings {
            paused: true,
            time_hours: 8.0,
            ..Default::default()
        };
        settings.advance(100.0);
        assert!((settings.time_hours - 8.0).abs() < f32::EPSILON);
    }

    #[test]
    fn disabled_time_does_not_advance() {
        let mut settings = TimeOfDaySettings {
            enabled: false,
            time_hours: 8.0,
            ..Default::default()
        };
        settings.advance(100.0);
        assert!((settings.time_hours - 8.0).abs() < f32::EPSILON);
    }
}
