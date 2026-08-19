//! Time-of-day lighting evaluation and Environment sync (ADR-052 E10, SKY-1).

use bevy::{light::GlobalAmbientLight, prelude::*};

use super::lighting::EnvironmentDirectionalLight;
use super::settings::EnvironmentSettings;
use super::singleton::{
    EnvironmentDirectionalLightResolution, update_environment_directional_light,
};
use super::time_of_day::TimeOfDaySettings;
use super::visual_state::{
    EnvironmentVisualState, SkyColorPalette, apply_visual_state_to_environment,
    evaluate_environment_visual_state, update_environment_visual_state,
};

pub use super::visual_state::{daylight_factor, twilight_warmth};

/// Computed lighting snapshot for one clock hour (testable, no ECS).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimeOfDayLighting {
    pub directional_light_rotation: Quat,
    pub directional_light_illuminance: f32,
    pub directional_light_color: Color,
    pub ambient_brightness: f32,
    pub ambient_color: Color,
}

impl From<EnvironmentVisualState> for TimeOfDayLighting {
    fn from(visual: EnvironmentVisualState) -> Self {
        Self {
            directional_light_rotation: visual.directional_light_rotation,
            directional_light_illuminance: visual.sun_illuminance,
            directional_light_color: visual.sun_color,
            ambient_brightness: visual.ambient_brightness,
            ambient_color: visual.ambient_color,
        }
    }
}

/// Evaluate lighting for the given settings without touching ECS.
pub fn evaluate_time_of_day_lighting(settings: &TimeOfDaySettings) -> TimeOfDayLighting {
    evaluate_environment_visual_state(settings, &SkyColorPalette::default()).into()
}

/// Write evaluated lighting into [`EnvironmentSettings`]. Returns false when cycle is disabled.
pub fn apply_time_of_day_to_settings(
    environment: &mut EnvironmentSettings,
    time_of_day: &TimeOfDaySettings,
) -> bool {
    if !time_of_day.enabled {
        return false;
    }
    let visual = evaluate_environment_visual_state(time_of_day, &SkyColorPalette::default());
    apply_visual_state_to_environment(environment, &visual);
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

/// Apply [`EnvironmentSettings`] to the singleton ambient and directional lights.
pub fn sync_environment_presentation(
    settings: Res<EnvironmentSettings>,
    mut ambient: ResMut<GlobalAmbientLight>,
    lights: Query<(&mut DirectionalLight, &mut Transform), With<EnvironmentDirectionalLight>>,
) {
    ambient.color = settings.ambient_color;
    ambient.brightness = settings.ambient_brightness;

    let count = lights.iter().count();
    let resolution = match count {
        0 => EnvironmentDirectionalLightResolution::Missing,
        1 => EnvironmentDirectionalLightResolution::Single,
        n => EnvironmentDirectionalLightResolution::Duplicate { count: n },
    };
    if !matches!(resolution, EnvironmentDirectionalLightResolution::Single) {
        #[cfg(feature = "dev")]
        if resolution != EnvironmentDirectionalLightResolution::Missing {
            bevy::log::warn!(
                target: "chasma::environment",
                "Skipping directional light update: {resolution:?}"
            );
        }
    } else {
        let _ = update_environment_directional_light(resolution, lights, |light, transform| {
            light.color = settings.directional_light_color;
            light.illuminance = settings.directional_light_illuminance;
            transform.rotation = settings.directional_light_rotation;
        });
    }
}

#[cfg(feature = "dev")]
/// Dev panel actions for the visual day-night clock (World window UI)
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
pub fn apply_time_of_day_dev_action(
    action: TimeOfDayDevAction,
    time_of_day: &mut TimeOfDaySettings,
) {
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
        "Time: {:02}:{:02}  cycle={}  paused={}  day_len={:.0}s\nWorld tab: [ / ] hour  , / . speed  (use panel for cycle/pause/presets)",
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
    use bevy::ecs::system::RunSystemOnce;

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
        assert!(!apply_time_of_day_to_settings(
            &mut environment,
            &time_of_day
        ));
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
        assert!(apply_time_of_day_to_settings(
            &mut environment,
            &time_of_day
        ));
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

    #[test]
    fn sync_environment_presentation_does_not_panic_without_directional_light() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<EnvironmentSettings>();
        app.init_resource::<GlobalAmbientLight>();
        app.world_mut()
            .run_system_once(sync_environment_presentation)
            .unwrap();
    }

    #[test]
    fn sync_environment_presentation_does_not_mutate_duplicate_lights() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<EnvironmentSettings>();
        app.init_resource::<GlobalAmbientLight>();
        app.world_mut().spawn((
            DirectionalLight {
                illuminance: 1.0,
                ..default()
            },
            Transform::default(),
            EnvironmentDirectionalLight,
        ));
        app.world_mut().spawn((
            DirectionalLight {
                illuminance: 2.0,
                ..default()
            },
            Transform::default(),
            EnvironmentDirectionalLight,
        ));
        let before: Vec<f32> = app
            .world_mut()
            .query::<&DirectionalLight>()
            .iter(app.world_mut())
            .map(|light| light.illuminance)
            .collect();
        app.world_mut()
            .run_system_once(sync_environment_presentation)
            .unwrap();
        let after: Vec<f32> = app
            .world_mut()
            .query::<&DirectionalLight>()
            .iter(app.world_mut())
            .map(|light| light.illuminance)
            .collect();
        assert_eq!(before, after);
    }

    #[test]
    fn lighting_and_visual_state_share_sun_direction() {
        let settings = TimeOfDaySettings {
            time_hours: 9.0,
            ..Default::default()
        };
        let lighting = evaluate_time_of_day_lighting(&settings);
        let visual = evaluate_environment_visual_state(&settings, &SkyColorPalette::default());
        assert_eq!(
            lighting.directional_light_rotation,
            visual.directional_light_rotation
        );
        assert_eq!(
            lighting.directional_light_illuminance,
            visual.sun_illuminance
        );
    }
}
