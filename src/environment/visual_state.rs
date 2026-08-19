//! Derived environment visual state (SKY-1, SUN-TRAJ-1).
//!
//! [`EnvironmentVisualState`] is renderer-facing output derived from [`super::time_of_day::TimeOfDaySettings`].
//! It is not a second clock and does not own pause or progression.

use bevy::prelude::*;

use super::settings::EnvironmentSettings;
use super::time_of_day::TimeOfDaySettings;

/// Art-directed sky color endpoints (Rust-side; shader receives evaluated colors).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SkyColorPalette {
    pub day_horizon: Color,
    pub day_zenith: Color,
    pub night_horizon: Color,
    pub night_zenith: Color,
    pub twilight_glow: Color,
}

impl Default for SkyColorPalette {
    fn default() -> Self {
        Self {
            day_horizon: Color::srgb(0.52, 0.68, 0.90),
            day_zenith: Color::srgb(0.10, 0.28, 0.72),
            night_horizon: Color::srgb(0.04, 0.06, 0.12),
            night_zenith: Color::srgb(0.01, 0.015, 0.05),
            twilight_glow: Color::srgb(0.92, 0.58, 0.34),
        }
    }
}

/// Derived visual environment snapshot for one clock hour (pure, testable).
#[derive(Debug, Clone, Copy, PartialEq, Resource)]
pub struct EnvironmentVisualState {
    pub time_hours: f32,
    pub normalized_day_fraction: f32,
    pub daylight_factor: f32,
    pub twilight_factor: f32,
    pub effective_daylight: f32,
    pub night_factor: f32,
    pub solar_elevation_rad: f32,
    /// Direction from the observer toward the sun (where the sky disc appears).
    pub sun_direction_world: Vec3,
    pub directional_light_rotation: Quat,
    pub sun_color: Color,
    pub sun_illuminance: f32,
    pub ambient_color: Color,
    pub ambient_brightness: f32,
    pub sky_horizon_color: Color,
    pub sky_zenith_color: Color,
    pub sun_disc_color: Color,
    pub sun_disc_intensity: f32,
}

/// Coherent 24-hour solar path for one clock sample (SUN-TRAJ-1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SolarTrajectory {
    pub time_hours: f32,
    pub azimuth_rad: f32,
    pub elevation_rad: f32,
    pub sun_direction_world: Vec3,
    pub directional_light_rotation: Quat,
    pub daylight_factor: f32,
    pub twilight_factor: f32,
}

const DAY_DIRECTIONAL_COLOR: Color = Color::srgb(1.0, 0.97, 0.92);
const TWILIGHT_DIRECTIONAL_COLOR: Color = Color::srgb(1.0, 0.72, 0.38);
const NIGHT_DIRECTIONAL_COLOR: Color = Color::srgb(0.55, 0.58, 0.72);

const DAY_AMBIENT_COLOR: Color = Color::srgb(0.85, 0.88, 0.95);
const NIGHT_AMBIENT_COLOR: Color = Color::srgb(0.35, 0.38, 0.52);

/// Sun disc half-angle (~0.9°) and soft edge width for the procedural sky shader.
pub const SUN_DISC_HALF_ANGLE_RAD: f32 = 0.008;
pub const SUN_DISC_SOFTNESS_RAD: f32 = 0.012;

/// Horizon blend exponent in the procedural sky gradient (matches WGSL).
pub const SKY_GRADIENT_HORIZON_EXPONENT: f32 = 2.0;

/// Sun-direction sharpness for localized twilight (matches WGSL).
pub const TWILIGHT_SUN_ALIGNMENT_EXPONENT: f32 = 8.0;

/// Elevation band used for geometry-driven twilight warmth (~12° full width).
pub const TWILIGHT_ELEVATION_HALF_ANGLE_RAD: f32 = 12.0_f32.to_radians();

/// Sunrise azimuth: eastern horizon in the project's Y-up frame.
const SUNRISE_AZIMUTH_RAD: f32 = -std::f32::consts::FRAC_PI_2;
/// Sunset azimuth: western horizon.
const SUNSET_AZIMUTH_RAD: f32 = std::f32::consts::FRAC_PI_2;

/// Vertical blend weight for horizon vs zenith at a world-up view component.
pub fn sky_horizon_weight(view_up: f32) -> f32 {
    let up = view_up.clamp(-1.0, 1.0);
    (1.0 - up.abs()).powf(SKY_GRADIENT_HORIZON_EXPONENT)
}

/// Localized twilight blend weight for one view direction (matches shader, `[0, 1]`).
pub fn twilight_localized_weight(sun_alignment: f32, view_up: f32, twilight_strength: f32) -> f32 {
    let horizon_weight = sky_horizon_weight(view_up);
    let band = sun_alignment
        .clamp(0.0, 1.0)
        .powf(TWILIGHT_SUN_ALIGNMENT_EXPONENT)
        * horizon_weight;
    (band * twilight_strength.clamp(0.0, 1.0)).clamp(0.0, 1.0)
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

fn max_day_elevation_rad(settings: &TimeOfDaySettings) -> f32 {
    settings.sun_pitch_max_deg.to_radians().max(0.0)
}

fn night_depth_rad(settings: &TimeOfDaySettings) -> f32 {
    (-settings.sun_pitch_min_deg).max(0.0).to_radians()
}

fn daylight_span_hours(settings: &TimeOfDaySettings) -> f32 {
    let span = settings.sunset_hour - settings.sunrise_hour;
    if span <= f32::EPSILON { 24.0 } else { span }
}

fn night_span_hours(settings: &TimeOfDaySettings) -> f32 {
    (24.0 - daylight_span_hours(settings)).max(f32::EPSILON)
}

fn night_elapsed_hours(time_hours: f32, settings: &TimeOfDaySettings) -> f32 {
    let t = TimeOfDaySettings::normalize_hours(time_hours);
    if t >= settings.sunset_hour {
        t - settings.sunset_hour
    } else {
        (24.0 - settings.sunset_hour) + t
    }
}

/// World-space observer→sun direction from azimuth (0 = south/+Z) and elevation.
pub fn sun_direction_from_azimuth_elevation(azimuth_rad: f32, elevation_rad: f32) -> Vec3 {
    let cos_elev = elevation_rad.cos();
    Vec3::new(
        cos_elev * azimuth_rad.sin(),
        elevation_rad.sin(),
        cos_elev * azimuth_rad.cos(),
    )
    .normalize_or_zero()
}

/// Directional-light rotation that maps local `+Z` to `sun_direction_world`.
pub fn directional_light_rotation_from_sun_direction(sun_direction_world: Vec3) -> Quat {
    let sun = sun_direction_world.normalize_or_zero();
    if sun.length_squared() <= f32::EPSILON {
        return Quat::IDENTITY;
    }
    Quat::from_rotation_arc(Vec3::Z, sun)
}

/// Daylight factor in `[0, 1]` from geometric elevation above the horizon.
pub fn daylight_from_elevation(elevation_rad: f32, max_day_elevation_rad: f32) -> f32 {
    if elevation_rad <= 0.0 || max_day_elevation_rad <= f32::EPSILON {
        return 0.0;
    }
    (elevation_rad / max_day_elevation_rad).clamp(0.0, 1.0)
}

/// Twilight warmth in `[0, 1]` from absolute elevation distance to the horizon.
pub fn twilight_from_elevation(elevation_rad: f32) -> f32 {
    let span = TWILIGHT_ELEVATION_HALF_ANGLE_RAD.max(f32::EPSILON);
    (1.0 - (elevation_rad.abs() / span).clamp(0.0, 1.0)).clamp(0.0, 1.0)
}

/// Evaluate the coherent 24-hour solar trajectory for one clock sample.
pub fn evaluate_solar_trajectory(settings: &TimeOfDaySettings) -> SolarTrajectory {
    let t = TimeOfDaySettings::normalize_hours(settings.time_hours);
    let max_elev = max_day_elevation_rad(settings);
    let night_depth = night_depth_rad(settings);
    let daylight_span = daylight_span_hours(settings);
    let night_span = night_span_hours(settings);

    let (elevation_rad, azimuth_rad) = if t >= settings.sunrise_hour && t < settings.sunset_hour {
        let day_p = ((t - settings.sunrise_hour) / daylight_span).clamp(0.0, 1.0);
        let elevation = max_elev * (std::f32::consts::PI * day_p).sin();
        let azimuth = lerp_f32(SUNRISE_AZIMUTH_RAD, SUNSET_AZIMUTH_RAD, day_p);
        (elevation, azimuth)
    } else {
        let night_p = (night_elapsed_hours(t, settings) / night_span).clamp(0.0, 1.0);
        let elevation = -night_depth * (std::f32::consts::PI * night_p).sin();
        let azimuth = SUNSET_AZIMUTH_RAD + std::f32::consts::PI * night_p;
        (elevation, azimuth)
    };

    let sun_direction_world = sun_direction_from_azimuth_elevation(azimuth_rad, elevation_rad);
    let directional_light_rotation =
        directional_light_rotation_from_sun_direction(sun_direction_world);
    let daylight_factor = daylight_from_elevation(elevation_rad, max_elev);
    let twilight_factor = twilight_from_elevation(elevation_rad);

    SolarTrajectory {
        time_hours: t,
        azimuth_rad,
        elevation_rad,
        sun_direction_world,
        directional_light_rotation,
        daylight_factor,
        twilight_factor,
    }
}

/// Smooth daylight factor in `[0, 1]` — derived from solar elevation (default pitch settings).
pub fn daylight_factor(time_hours: f32, sunrise_hour: f32, sunset_hour: f32) -> f32 {
    evaluate_solar_trajectory(&TimeOfDaySettings {
        time_hours,
        sunrise_hour,
        sunset_hour,
        ..Default::default()
    })
    .daylight_factor
}

/// Twilight warmth in `[0, 1]` — derived from geometric horizon proximity (default pitch settings).
pub fn twilight_warmth(time_hours: f32, sunrise_hour: f32, sunset_hour: f32) -> f32 {
    evaluate_solar_trajectory(&TimeOfDaySettings {
        time_hours,
        sunrise_hour,
        sunset_hour,
        ..Default::default()
    })
    .twilight_factor
}

/// Direction from the observer toward the sun, derived from the directional-light rotation.
///
/// Bevy [`Transform::forward`] (light-ray travel) equals `-sun_direction_world`.
pub fn sun_direction_world_from_light_rotation(rotation: Quat) -> Vec3 {
    (rotation * Vec3::Z).normalize_or_zero()
}

/// Light-ray travel direction for Bevy [`DirectionalLight`] (opposite of [`sun_direction_world_from_light_rotation`]).
pub fn light_travel_direction_from_sun(rotation: Quat) -> Vec3 {
    (rotation * Vec3::NEG_Z).normalize_or_zero()
}

/// Evaluate derived visual state from the authoritative clock settings.
pub fn evaluate_environment_visual_state(
    settings: &TimeOfDaySettings,
    palette: &SkyColorPalette,
) -> EnvironmentVisualState {
    let solar = evaluate_solar_trajectory(settings);
    let daylight = solar.daylight_factor;
    let twilight = solar.twilight_factor;
    let effective_daylight =
        (daylight + twilight * settings.twilight_daylight_blend).clamp(0.0, 1.0);
    let night_factor = (1.0 - effective_daylight).clamp(0.0, 1.0);

    let sun_illuminance = lerp_f32(
        settings.night_directional_illuminance,
        settings.noon_directional_illuminance,
        effective_daylight,
    );

    let base_sun = lerp_color(
        NIGHT_DIRECTIONAL_COLOR,
        DAY_DIRECTIONAL_COLOR,
        effective_daylight,
    );
    let sun_color = lerp_color(base_sun, TWILIGHT_DIRECTIONAL_COLOR, twilight);

    let night_ambient = settings.noon_ambient_brightness * settings.night_ambient_multiplier;
    let ambient_brightness = lerp_f32(
        night_ambient,
        settings.noon_ambient_brightness,
        effective_daylight,
    );
    let ambient_color = lerp_color(NIGHT_AMBIENT_COLOR, DAY_AMBIENT_COLOR, effective_daylight);

    let sun_direction_world = solar.sun_direction_world;
    let directional_light_rotation = solar.directional_light_rotation;
    let solar_elevation_rad = solar.elevation_rad;

    let sky_horizon = lerp_color(
        palette.night_horizon,
        palette.day_horizon,
        effective_daylight,
    );
    let sky_zenith = lerp_color(palette.night_zenith, palette.day_zenith, effective_daylight);

    let sun_disc_color = lerp_color(sun_color, palette.twilight_glow, twilight * 0.35);
    let sun_disc_intensity = if sun_direction_world.y > -0.02 {
        (effective_daylight * 1.15 + twilight * 0.55).clamp(0.0, 1.35)
    } else {
        0.0
    };

    EnvironmentVisualState {
        time_hours: solar.time_hours,
        normalized_day_fraction: solar.time_hours / 24.0,
        daylight_factor: daylight,
        twilight_factor: twilight,
        effective_daylight,
        night_factor,
        solar_elevation_rad,
        sun_direction_world,
        directional_light_rotation,
        sun_color,
        sun_illuminance,
        ambient_color,
        ambient_brightness,
        sky_horizon_color: sky_horizon,
        sky_zenith_color: sky_zenith,
        sun_disc_color,
        sun_disc_intensity,
    }
}

/// Write evaluated lighting fields into [`EnvironmentSettings`].
pub fn apply_visual_state_to_environment(
    environment: &mut EnvironmentSettings,
    visual: &EnvironmentVisualState,
) {
    environment.directional_light_rotation = visual.directional_light_rotation;
    environment.directional_light_illuminance = visual.sun_illuminance;
    environment.directional_light_color = visual.sun_color;
    environment.ambient_brightness = visual.ambient_brightness;
    environment.ambient_color = visual.ambient_color;
}

/// Advance derived state from the authoritative clock and optionally sync [`EnvironmentSettings`].
pub fn update_environment_visual_state(
    time_of_day: Res<TimeOfDaySettings>,
    mut visual: ResMut<EnvironmentVisualState>,
    mut environment: ResMut<EnvironmentSettings>,
) {
    let palette = SkyColorPalette::default();
    *visual = evaluate_environment_visual_state(&time_of_day, &palette);
    if time_of_day.enabled {
        apply_visual_state_to_environment(&mut environment, &visual);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn visual_at(hour: f32) -> EnvironmentVisualState {
        evaluate_environment_visual_state(
            &TimeOfDaySettings {
                time_hours: hour,
                ..Default::default()
            },
            &SkyColorPalette::default(),
        )
    }

    fn solar_at(hour: f32) -> SolarTrajectory {
        evaluate_solar_trajectory(&TimeOfDaySettings {
            time_hours: hour,
            ..Default::default()
        })
    }

    fn assert_unit(v: Vec3) {
        assert!(v.is_finite());
        assert!(
            (v.length() - 1.0).abs() < 1e-4,
            "expected unit vector, got {v:?}"
        );
    }

    fn assert_factor(name: &str, value: f32) {
        assert!(value.is_finite(), "{name} must be finite");
        assert!((0.0..=1.0).contains(&value), "{name}={value} out of [0,1]");
    }

    fn assert_color_finite(color: Color) {
        let [r, g, b, a] = color.to_srgba().to_f32_array();
        assert!(r.is_finite() && g.is_finite() && b.is_finite() && a.is_finite());
        assert!(r >= 0.0 && g >= 0.0 && b >= 0.0 && a >= 0.0);
    }

    fn elevation_deg(hour: f32) -> f32 {
        solar_at(hour).elevation_rad.to_degrees()
    }

    fn east_face_direct_proxy(hour: f32) -> f32 {
        let visual = visual_at(hour);
        (-visual.sun_direction_world.x).max(0.0) * visual.effective_daylight
    }

    #[test]
    fn normalized_day_fraction_wraps_with_clock() {
        let mut settings = TimeOfDaySettings::default();
        settings.set_time_hours(25.0);
        let visual = evaluate_environment_visual_state(&settings, &SkyColorPalette::default());
        assert!((visual.normalized_day_fraction - (1.0 / 24.0)).abs() < 1e-4);
    }

    #[test]
    fn factors_are_bounded_and_finite() {
        for hour in [0.0, 3.0, 6.0, 8.0, 12.0, 17.0, 18.0, 21.0, 23.5] {
            let visual = visual_at(hour);
            assert_factor("daylight_factor", visual.daylight_factor);
            assert_factor("twilight_factor", visual.twilight_factor);
            assert_factor("effective_daylight", visual.effective_daylight);
            assert_factor("night_factor", visual.night_factor);
        }
    }

    #[test]
    fn sun_direction_is_normalized_and_finite() {
        for hour in [0.0, 6.0, 12.0, 18.0, 23.0] {
            assert_unit(visual_at(hour).sun_direction_world);
        }
    }

    #[test]
    fn sun_direction_matches_light_travel_convention() {
        let visual = visual_at(12.0);
        let travel = light_travel_direction_from_sun(visual.directional_light_rotation);
        let sum = visual.sun_direction_world + travel;
        assert!(
            sum.length() < 1e-4,
            "sun and light travel must oppose: {sum:?}"
        );
    }

    #[test]
    fn midnight_sun_is_below_horizon_at_max_negative_elevation() {
        let midnight = solar_at(0.0);
        assert!(midnight.elevation_rad < 0.0);
        assert!(
            (midnight.elevation_rad + night_depth_rad(&TimeOfDaySettings::default())).abs() < 1e-4
        );
        assert!(visual_at(0.0).daylight_factor < f32::EPSILON);
        assert!(visual_at(0.0).sun_disc_intensity < 0.05);
    }

    #[test]
    fn noon_is_maximum_positive_elevation() {
        let noon = solar_at(12.0);
        let max_elev = max_day_elevation_rad(&TimeOfDaySettings::default());
        assert!((noon.elevation_rad - max_elev).abs() < 1e-4);
        assert!((visual_at(12.0).daylight_factor - 1.0).abs() < 1e-4);
    }

    #[test]
    fn sunrise_is_upward_horizon_crossing() {
        assert!(elevation_deg(5.0) < 0.0);
        assert!(elevation_deg(6.0).abs() < 0.05);
        assert!(elevation_deg(7.0) > elevation_deg(6.0));
        assert!(elevation_deg(7.0) > 0.0);
    }

    #[test]
    fn sunset_is_downward_horizon_crossing() {
        assert!(elevation_deg(17.0) > 0.0);
        assert!(elevation_deg(18.0).abs() < 0.05);
        assert!(elevation_deg(19.0) < 0.0);
        assert!(elevation_deg(19.0) < elevation_deg(18.0));
    }

    #[test]
    fn elevation_monotonic_during_daylight() {
        let mut prev = elevation_deg(6.0);
        for minute in (6 * 60 + 15)..=(12 * 60) {
            let hour = minute as f32 / 60.0;
            let elev = elevation_deg(hour);
            assert!(
                elev + 1e-3 >= prev,
                "rise failed at {hour}: {elev} < {prev}"
            );
            prev = elev;
        }

        prev = elevation_deg(12.0);
        for minute in (12 * 60 + 15)..=(18 * 60) {
            let hour = minute as f32 / 60.0;
            let elev = elevation_deg(hour);
            assert!(
                elev <= prev + 1e-3,
                "fall failed at {hour}: {elev} > {prev}"
            );
            prev = elev;
        }
    }

    #[test]
    fn interior_night_elevation_stays_negative() {
        for hour in [19.0, 20.0, 21.0, 22.0, 23.0, 1.0, 2.0, 3.0, 4.0, 5.0] {
            assert!(
                solar_at(hour).elevation_rad < 0.0,
                "night interior above horizon at {hour}"
            );
        }
    }

    #[test]
    fn exactly_one_upward_and_one_downward_horizon_crossing_per_day() {
        let mut upward = 0;
        let mut downward = 0;
        let mut prev = elevation_deg(0.0);
        for minute in 1..=(24 * 60) {
            let hour = minute as f32 / 60.0;
            let elev = elevation_deg(hour);
            if prev <= 0.0 && elev > 0.0 {
                upward += 1;
            }
            if prev >= 0.0 && elev < 0.0 {
                downward += 1;
            }
            prev = elev;
        }
        assert_eq!(upward, 1);
        assert_eq!(downward, 1);
    }

    #[test]
    fn solar_direction_is_continuous_across_midnight() {
        let before = solar_at(23.99);
        let after = solar_at(0.0);
        assert!(
            (before.sun_direction_world - after.sun_direction_world).length() < 0.02,
            "midnight discontinuity"
        );
    }

    #[test]
    fn daylight_and_twilight_follow_elevation() {
        let deep_night = visual_at(2.0);
        let sunrise = visual_at(6.0);
        let noon = visual_at(12.0);
        assert!(deep_night.daylight_factor < 0.01);
        assert!(deep_night.twilight_factor < 0.01);
        assert!(sunrise.twilight_factor > 0.9);
        assert!(sunrise.daylight_factor < 0.01);
        assert!((noon.daylight_factor - 1.0).abs() < 1e-4);
        assert!(noon.twilight_factor < 0.01);
    }

    #[test]
    fn arbitrary_sunrise_sunset_preserves_horizon_crossings() {
        let settings = TimeOfDaySettings {
            sunrise_hour: 5.0,
            sunset_hour: 19.0,
            ..Default::default()
        };
        let before = evaluate_solar_trajectory(&TimeOfDaySettings {
            time_hours: 4.5,
            ..settings
        });
        let at = evaluate_solar_trajectory(&TimeOfDaySettings {
            time_hours: 5.0,
            ..settings
        });
        let after = evaluate_solar_trajectory(&TimeOfDaySettings {
            time_hours: 5.5,
            ..settings
        });
        assert!(before.elevation_rad < 0.0);
        assert!(at.elevation_rad.abs() < 0.01);
        assert!(after.elevation_rad > 0.0);

        let before = evaluate_solar_trajectory(&TimeOfDaySettings {
            time_hours: 18.5,
            ..settings
        });
        let at = evaluate_solar_trajectory(&TimeOfDaySettings {
            time_hours: 19.0,
            ..settings
        });
        let after = evaluate_solar_trajectory(&TimeOfDaySettings {
            time_hours: 19.5,
            ..settings
        });
        assert!(before.elevation_rad > 0.0);
        assert!(at.elevation_rad.abs() < 0.01);
        assert!(after.elevation_rad < 0.0);
    }

    #[test]
    fn east_face_does_not_reproduce_afternoon_second_sunrise() {
        let mut morning_peak = 0.0f32;
        let mut afternoon_peak = 0.0f32;
        for minute in 0..=(24 * 60) {
            let hour = minute as f32 / 60.0;
            let proxy = east_face_direct_proxy(hour);
            if hour >= 5.0 && hour <= 11.5 {
                morning_peak = morning_peak.max(proxy);
            }
            if hour >= 12.5 && hour <= 17.0 {
                afternoon_peak = afternoon_peak.max(proxy);
            }
        }
        assert!(morning_peak > 0.15);
        assert!(
            afternoon_peak < morning_peak * 0.2,
            "afternoon east-face peak {afternoon_peak} too close to morning {morning_peak}"
        );
    }

    #[test]
    fn noon_is_brighter_than_deep_night() {
        let noon = visual_at(12.0);
        let night = visual_at(2.0);
        assert!(noon.sun_illuminance > night.sun_illuminance * 10.0);
        let noon_zenith = noon.sky_zenith_color.to_srgba();
        let night_zenith = night.sky_zenith_color.to_srgba();
        assert!(noon_zenith.blue > night_zenith.blue);
    }

    fn expected_horizon_without_twilight_prewarm(hour: f32) -> Color {
        let visual = visual_at(hour);
        let effective = (visual.daylight_factor
            + visual.twilight_factor * TimeOfDaySettings::default().twilight_daylight_blend)
            .clamp(0.0, 1.0);
        let palette = SkyColorPalette::default();
        lerp_color(palette.night_horizon, palette.day_horizon, effective)
    }

    #[test]
    fn horizon_is_not_globally_pre_warmed_at_twilight() {
        for hour in [6.0, 18.0, 17.5] {
            let visual = visual_at(hour);
            let expected = expected_horizon_without_twilight_prewarm(hour);
            let actual = visual.sky_horizon_color.to_srgba();
            let expected = expected.to_srgba();
            assert!(
                (actual.red - expected.red).abs() < 0.02,
                "hour {hour}: horizon red pre-warmed"
            );
            assert!(
                (actual.green - expected.green).abs() < 0.02,
                "hour {hour}: horizon green pre-warmed"
            );
            assert!(
                (actual.blue - expected.blue).abs() < 0.02,
                "hour {hour}: horizon blue pre-warmed"
            );
        }
    }

    #[test]
    fn twilight_localized_weight_is_zero_away_from_sun_at_horizon() {
        assert!(twilight_localized_weight(0.0, 0.0, 1.0) < f32::EPSILON);
    }

    #[test]
    fn twilight_localized_weight_is_bounded() {
        for sun_dot in [0.0, 0.25, 0.5, 0.75, 1.0] {
            for up in [-1.0, 0.0, 0.5, 1.0] {
                for twilight in [0.0, 0.33, 1.0] {
                    let weight = twilight_localized_weight(sun_dot, up, twilight);
                    assert!(weight.is_finite());
                    assert!((0.0..=1.0).contains(&weight));
                }
            }
        }
    }

    #[test]
    fn twilight_localized_weight_peaks_toward_sun_at_horizon() {
        let peak = twilight_localized_weight(1.0, 0.0, 1.0);
        let side = twilight_localized_weight(0.0, 0.0, 1.0);
        let zenith = twilight_localized_weight(1.0, 1.0, 1.0);
        assert!(peak > side);
        assert!(peak > zenith);
    }

    #[test]
    fn day_zenith_is_deeper_blue_than_day_horizon() {
        let palette = SkyColorPalette::default();
        let horizon = palette.day_horizon.to_srgba();
        let zenith = palette.day_zenith.to_srgba();
        assert!(zenith.red < horizon.red);
        assert!(zenith.green < horizon.green);
        let zenith_luma = zenith.red * 0.2126 + zenith.green * 0.7152 + zenith.blue * 0.0722;
        let horizon_luma = horizon.red * 0.2126 + horizon.green * 0.7152 + horizon.blue * 0.0722;
        assert!(zenith_luma < horizon_luma);
    }

    #[test]
    fn sunrise_sunset_continuity_with_adjacent_hours() {
        let before_sunrise = visual_at(5.5);
        let at_sunrise = visual_at(6.0);
        let after_sunrise = visual_at(6.5);
        assert!((before_sunrise.effective_daylight - at_sunrise.effective_daylight).abs() < 0.35);
        assert!((at_sunrise.effective_daylight - after_sunrise.effective_daylight).abs() < 0.35);

        let before_sunset = visual_at(17.5);
        let at_sunset = visual_at(18.0);
        assert!((before_sunset.effective_daylight - at_sunset.effective_daylight).abs() < 0.35);
    }

    #[test]
    fn evaluated_colors_and_lighting_are_finite_and_nonnegative() {
        for hour in [0.0, 6.0, 12.0, 18.0, 23.0] {
            let visual = visual_at(hour);
            assert_color_finite(visual.sun_color);
            assert_color_finite(visual.ambient_color);
            assert_color_finite(visual.sky_horizon_color);
            assert_color_finite(visual.sky_zenith_color);
            assert_color_finite(visual.sun_disc_color);
            assert!(visual.sun_illuminance.is_finite() && visual.sun_illuminance >= 0.0);
            assert!(visual.ambient_brightness.is_finite() && visual.ambient_brightness >= 0.0);
            assert!(visual.sun_disc_intensity.is_finite() && visual.sun_disc_intensity >= 0.0);
        }
    }

    #[test]
    fn night_sky_is_darker_than_day() {
        let day = visual_at(12.0);
        let night = visual_at(2.0);
        let day_luma = day.sky_zenith_color.to_srgba().green;
        let night_luma = night.sky_zenith_color.to_srgba().green;
        assert!(day_luma > night_luma);
        assert!(night.sun_disc_intensity < 0.05);
    }

    #[test]
    fn rotation_matches_sun_direction_authority() {
        for hour in [0.0, 6.0, 12.0, 18.0, 23.0] {
            let solar = solar_at(hour);
            let from_rotation =
                sun_direction_world_from_light_rotation(solar.directional_light_rotation);
            assert!(
                (from_rotation - solar.sun_direction_world).length() < 1e-4,
                "hour {hour}"
            );
        }
    }
}
