//! Time-of-day lighting evaluation and Environment sync (ADR-052 E10).

use bevy::{
    core_pipeline::Skybox,
    light::GlobalAmbientLight,
    prelude::*,
};

use super::lighting::EnvironmentDirectionalLight;
use super::settings::EnvironmentSettings;
use super::skybox::SkyboxCamera;
use super::time_of_day::TimeOfDaySettings;

/// Computed lighting snapshot for one clock hour (testable, no ECS).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimeOfDayLighting {
    pub directional_light_rotation: Quat,
    pub directional_light_illuminance: f32,
    pub directional_light_color: Color,
    pub ambient_brightness: f32,
    pub ambient_color: Color,
    pub skybox_brightness: f32,
}


const DAY_DIRECTIONAL_COLOR: Color = Color::srgb(1.0, 0.97, 0.92);
const TWILIGHT_DIRECTIONAL_COLOR: Color = Color::srgb(1.0, 0.72, 0.38);
const NIGHT_DIRECTIONAL_COLOR: Color = Color::srgb(0.55, 0.58, 0.72);

const DAY_AMBIENT_COLOR: Color = Color::srgb(0.85, 0.88, 0.95);
const NIGHT_AMBIENT_COLOR: Color = Color::srgb(0.35, 0.38, 0.52);

/// Smooth daylight factor in `[0, 1]` — peaks at solar noon between sunrise and sunset.
pub fn daylight_factor(time_hours: f32, sunrise_hour: f32, sunset_hour: f32) -> f32 {
    let t = TimeOfDaySettings::normalize_hours(time_hours);
    if t < sunrise_hour || t >= sunset_hour {
        return 0.0;
    }
    let noon = (sunrise_hour + sunset_hour) * 0.5;
    let half_day = (sunset_hour - sunrise_hour) * 0.5;
    if half_day <= f32::EPSILON {
        return 0.0;
    }
    let x = (t - noon) / half_day;
    (1.0 - x * x).max(0.0).sqrt()
}

/// Twilight warmth in `[0, 1]` — peaks near sunrise and sunset.
pub fn twilight_warmth(time_hours: f32, sunrise_hour: f32, sunset_hour: f32) -> f32 {
    let t = TimeOfDaySettings::normalize_hours(time_hours);
    let dawn = 1.0 - (t - sunrise_hour).abs() / 1.5;
    let dusk = 1.0 - (t - sunset_hour).abs() / 1.5;
    dawn.max(dusk).clamp(0.0, 1.0)
}

fn lerp_f32(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    let [ar, ag, ab, aa] = a.to_srgba().to_f32_array();
    let [br, bg, bb, ba] = b.to_srgba().to_f32_array();
    Color::srgba(
        lerp_f32(ar, br, t),
        lerp_f32(ag, bg, t),
        lerp_f32(ab, bb, t),
        lerp_f32(aa, ba, t),
    )
}

/// Evaluate lighting for the given settings without touching ECS.
pub fn evaluate_time_of_day_lighting(settings: &TimeOfDaySettings) -> TimeOfDayLighting {
    let daylight = daylight_factor(
        settings.time_hours,
        settings.sunrise_hour,
        settings.sunset_hour,
    );
    let warmth = twilight_warmth(
        settings.time_hours,
        settings.sunrise_hour,
        settings.sunset_hour,
    );

    // Twilight adds brightness even when daylight_factor is still low at dawn/dusk.
    let effective_daylight =
        (daylight + warmth * settings.twilight_daylight_blend).clamp(0.0, 1.0);

    let directional_light_illuminance = lerp_f32(
        settings.night_directional_illuminance,
        settings.noon_directional_illuminance,
        effective_daylight,
    );

    let base_directional = lerp_color(NIGHT_DIRECTIONAL_COLOR, DAY_DIRECTIONAL_COLOR, effective_daylight);
    let directional_light_color = lerp_color(base_directional, TWILIGHT_DIRECTIONAL_COLOR, warmth);

    let night_ambient = settings.noon_ambient_brightness * settings.night_ambient_multiplier;
    let ambient_brightness =
        lerp_f32(night_ambient, settings.noon_ambient_brightness, effective_daylight);
    let ambient_color = lerp_color(NIGHT_AMBIENT_COLOR, DAY_AMBIENT_COLOR, effective_daylight);

    let skybox_brightness = lerp_f32(
        settings.night_skybox_brightness,
        settings.noon_skybox_brightness,
        effective_daylight,
    );

    let t = TimeOfDaySettings::normalize_hours(settings.time_hours);
    let yaw = (t / 24.0) * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
    let pitch_min = settings.sun_pitch_min_deg.to_radians();
    let pitch_max = settings.sun_pitch_max_deg.to_radians();
    let pitch = lerp_f32(pitch_min, pitch_max, daylight);
    let directional_light_rotation = Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0);

    TimeOfDayLighting {
        directional_light_rotation,
        directional_light_illuminance,
        directional_light_color,
        ambient_brightness,
        ambient_color,
        skybox_brightness,
    }
}

/// Write evaluated lighting into [`EnvironmentSettings`]. Returns false when cycle is disabled.
pub fn apply_time_of_day_to_settings(
    environment: &mut EnvironmentSettings,
    time_of_day: &TimeOfDaySettings,
) -> bool {
    if !time_of_day.enabled {
        return false;
    }
    let lighting = evaluate_time_of_day_lighting(time_of_day);
    environment.directional_light_rotation = lighting.directional_light_rotation;
    environment.directional_light_illuminance = lighting.directional_light_illuminance;
    environment.directional_light_color = lighting.directional_light_color;
    environment.ambient_brightness = lighting.ambient_brightness;
    environment.ambient_color = lighting.ambient_color;
    environment.skybox_brightness = lighting.skybox_brightness;
    true
}

/// Advance the visual clock from real delta time.
pub fn advance_time_of_day(time: Res<Time>, mut time_of_day: ResMut<TimeOfDaySettings>) {
    time_of_day.advance(time.delta_secs());
}

/// Push active time-of-day lighting into [`EnvironmentSettings`].
pub fn update_environment_from_time_of_day(
    time_of_day: Res<TimeOfDaySettings>,
    mut environment: ResMut<EnvironmentSettings>,
) {
    let _ = apply_time_of_day_to_settings(&mut environment, &time_of_day);
}

/// Apply [`EnvironmentSettings`] to the singleton ambient light, directional light, and skybox.
pub fn sync_environment_presentation(
    settings: Res<EnvironmentSettings>,
    mut ambient: ResMut<GlobalAmbientLight>,
    mut lights: Query<
        (&mut DirectionalLight, &mut Transform),
        With<EnvironmentDirectionalLight>,
    >,
    mut skyboxes: Query<&mut Skybox, With<SkyboxCamera>>,
) {
    ambient.color = settings.ambient_color;
    ambient.brightness = settings.ambient_brightness;

    if let Ok((mut light, mut transform)) = lights.single_mut() {
        light.color = settings.directional_light_color;
        light.illuminance = settings.directional_light_illuminance;
        transform.rotation = settings.directional_light_rotation;
    }

    for mut skybox in &mut skyboxes {
        skybox.brightness = settings.skybox_brightness;
    }
}

#[cfg(feature = "dev")]
pub fn time_of_day_dev_keyboard(
    keyboard: Res<ButtonInput<KeyCode>>,
    dev_state: Res<crate::dev::DevModeState>,
    mut time_of_day: ResMut<TimeOfDaySettings>,
) {
    if !dev_state.enabled {
        return;
    }

    if keyboard.just_pressed(KeyCode::KeyT) {
        apply_time_of_day_dev_action(TimeOfDayDevAction::ToggleEnabled, &mut time_of_day);
    }
    if keyboard.just_pressed(KeyCode::KeyP) {
        apply_time_of_day_dev_action(TimeOfDayDevAction::TogglePaused, &mut time_of_day);
    }
    if keyboard.just_pressed(KeyCode::BracketLeft) {
        apply_time_of_day_dev_action(TimeOfDayDevAction::HourEarlier, &mut time_of_day);
    }
    if keyboard.just_pressed(KeyCode::BracketRight) {
        apply_time_of_day_dev_action(TimeOfDayDevAction::HourLater, &mut time_of_day);
    }
    if keyboard.just_pressed(KeyCode::Comma) {
        apply_time_of_day_dev_action(TimeOfDayDevAction::SlowerDay, &mut time_of_day);
    }
    if keyboard.just_pressed(KeyCode::Period) {
        apply_time_of_day_dev_action(TimeOfDayDevAction::FasterDay, &mut time_of_day);
    }
    if keyboard.just_pressed(KeyCode::Digit6) {
        apply_time_of_day_dev_action(TimeOfDayDevAction::SetDawn, &mut time_of_day);
    }
    if keyboard.just_pressed(KeyCode::Digit1) {
        apply_time_of_day_dev_action(TimeOfDayDevAction::SetNoon, &mut time_of_day);
    }
    if keyboard.just_pressed(KeyCode::Digit0) {
        apply_time_of_day_dev_action(TimeOfDayDevAction::SetMidnight, &mut time_of_day);
    }
}

/// Dev panel / hotkey actions for the visual day-night clock.
#[cfg(feature = "dev")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeOfDayDevAction {
    ToggleEnabled,
    TogglePaused,
    HourEarlier,
    HourLater,
    SlowerDay,
    FasterDay,
    SetDawn,
    SetNoon,
    SetMidnight,
}

#[cfg(feature = "dev")]
pub fn apply_time_of_day_dev_action(action: TimeOfDayDevAction, time_of_day: &mut TimeOfDaySettings) {
    match action {
        TimeOfDayDevAction::ToggleEnabled => {
            time_of_day.enabled = !time_of_day.enabled;
            bevy::log::info!(
                target: "chasma::environment",
                "Time of day {}",
                if time_of_day.enabled { "enabled" } else { "disabled" }
            );
        }
        TimeOfDayDevAction::TogglePaused => {
            time_of_day.paused = !time_of_day.paused;
            bevy::log::info!(
                target: "chasma::environment",
                "Time of day {}",
                if time_of_day.paused { "paused" } else { "running" }
            );
        }
        TimeOfDayDevAction::HourEarlier => {
            time_of_day.set_time_hours(time_of_day.time_hours - 1.0);
        }
        TimeOfDayDevAction::HourLater => {
            time_of_day.set_time_hours(time_of_day.time_hours + 1.0);
        }
        TimeOfDayDevAction::SlowerDay => {
            time_of_day.day_length_seconds = (time_of_day.day_length_seconds - 60.0).max(30.0);
        }
        TimeOfDayDevAction::FasterDay => {
            time_of_day.day_length_seconds = (time_of_day.day_length_seconds + 60.0).min(3600.0);
        }
        TimeOfDayDevAction::SetDawn => {
            let sunrise = time_of_day.sunrise_hour;
            time_of_day.set_time_hours(sunrise);
        }
        TimeOfDayDevAction::SetNoon => {
            time_of_day.set_time_hours(12.0);
        }
        TimeOfDayDevAction::SetMidnight => {
            time_of_day.set_time_hours(0.0);
        }
    }
}

#[cfg(feature = "dev")]
pub fn format_time_of_day_status(settings: &TimeOfDaySettings) -> String {
    let hours = settings.time_hours.floor() as u32;
    let minutes = ((settings.time_hours.fract()) * 60.0).floor() as u32;
    format!(
        "Time: {:02}:{:02}  cycle={}  paused={}  day_len={:.0}s\n[ / ] hour  6 dawn  1 noon  0 night  T toggle  P pause",
        hours % 24,
        minutes,
        if settings.enabled { "on" } else { "off" },
        settings.paused,
        settings.day_length_seconds,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lighting_at(hour: f32) -> TimeOfDayLighting {
        let settings = TimeOfDaySettings {
            time_hours: hour,
            ..Default::default()
        };
        evaluate_time_of_day_lighting(&settings)
    }

    #[test]
    fn noon_gives_high_light_intensity() {
        let noon = lighting_at(12.0);
        let night = lighting_at(2.0);
        assert!(noon.directional_light_illuminance > night.directional_light_illuminance * 10.0);
        assert!(noon.skybox_brightness > night.skybox_brightness);
    }

    #[test]
    fn night_gives_low_directional_intensity() {
        let night = lighting_at(3.0);
        assert!(night.directional_light_illuminance < 200.0);
    }

    #[test]
    fn sunrise_directional_exceeds_deep_night() {
        let night = lighting_at(2.0);
        let sunrise = lighting_at(7.0);
        assert!(sunrise.directional_light_illuminance > night.directional_light_illuminance);
        assert!(sunrise.skybox_brightness >= night.skybox_brightness);
    }

    #[test]
    fn sunrise_and_sunset_produce_warmer_light_than_noon() {
        let noon = lighting_at(12.0);
        let sunrise = lighting_at(6.0);
        let sunset = lighting_at(18.0);
        let noon_rgb = noon.directional_light_color.to_srgba();
        let sunrise_rgb = sunrise.directional_light_color.to_srgba();
        let sunset_rgb = sunset.directional_light_color.to_srgba();
        assert!(sunrise_rgb.green < noon_rgb.green);
        assert!(sunset_rgb.green < noon_rgb.green);
        assert!(sunrise_rgb.red >= noon_rgb.red - 0.05);
    }

    #[test]
    fn disabled_system_does_not_mutate_environment_settings() {
        let mut environment = EnvironmentSettings::default();
        let before = environment.clone();
        let time_of_day = TimeOfDaySettings {
            enabled: false,
            time_hours: 3.0,
            ..Default::default()
        };
        assert!(!apply_time_of_day_to_settings(&mut environment, &time_of_day));
        assert_eq!(environment, before);
    }

    #[test]
    fn enabled_system_updates_environment_settings() {
        let mut environment = EnvironmentSettings::default();
        let time_of_day = TimeOfDaySettings {
            enabled: true,
            time_hours: 3.0,
            ..Default::default()
        };
        assert!(apply_time_of_day_to_settings(&mut environment, &time_of_day));
        assert_ne!(
            environment.directional_light_illuminance,
            EnvironmentSettings::default().directional_light_illuminance
        );
    }

    #[test]
    fn daylight_factor_peaks_at_noon() {
        let settings = TimeOfDaySettings::default();
        let dawn = daylight_factor(6.0, settings.sunrise_hour, settings.sunset_hour);
        let noon = daylight_factor(12.0, settings.sunrise_hour, settings.sunset_hour);
        let dusk = daylight_factor(18.0, settings.sunrise_hour, settings.sunset_hour);
        assert!(noon > dawn);
        assert!(noon > dusk);
        assert!((noon - 1.0).abs() < 1e-4);
    }
}
