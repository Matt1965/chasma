//! Procedural cloud presentation settings (CLOUD-1, CLOUD-1F, CLOUD-VOL-1, CLOUD-VOL-V2A).

use bevy::prelude::*;

/// Identifies the two atmospheric cloud strata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloudLayerId {
    Low,
    High,
}

/// Default LOW macro footprint scale (~2 km base noise cell at `1 / macro_scale` meters).
pub const DEFAULT_LOW_CLOUD_MACRO_SCALE: f32 = 0.0005;
/// Default HIGH macro footprint scale (~1 km base noise cell at `1 / macro_scale` meters).
pub const DEFAULT_HIGH_CLOUD_MACRO_SCALE: f32 = 0.001;
/// Back-compat alias for [`DEFAULT_LOW_CLOUD_MACRO_SCALE`].
pub const DEFAULT_LOW_CLOUD_SCALE: f32 = DEFAULT_LOW_CLOUD_MACRO_SCALE;
/// Back-compat alias for [`DEFAULT_HIGH_CLOUD_MACRO_SCALE`].
pub const DEFAULT_HIGH_CLOUD_SCALE: f32 = DEFAULT_HIGH_CLOUD_MACRO_SCALE;

/// Default vertical development seam (1.0 = current flat envelope baseline until Step 3).
pub const DEFAULT_CLOUD_VERTICAL_DEVELOPMENT: f32 = 1.0;
/// Default optical-thickness seam (was opacity in CLOUD-1F).
pub const DEFAULT_CLOUD_DENSITY_SCALE: f32 = 0.72;
/// Default edge-breakup seam (was detail in CLOUD-1F; drives L3 erosion strength at boundaries).
pub const DEFAULT_CLOUD_EDGE_BREAKUP: f32 = 0.55;
/// L3 erosion noise ratio relative to macro_scale (~180 m cells at default macro_scale).
pub const CLOUD_EROSION_NOISE_RATIO: f32 = 11.12;
/// Height bias for erosion field (mirrors WGSL `EROSION_HEIGHT_BIAS`).
pub const CLOUD_EROSION_HEIGHT_BIAS: f32 = 2.0;

/// Baseline ray-march samples (matches WGSL `CLOUD_MARCH_MAX_STEPS`).
pub const CLOUD_MARCH_MAX_STEPS: i32 = 24;
/// Adaptive upper bound when segment length requires smaller steps (matches WGSL).
pub const CLOUD_MARCH_MAX_STEPS_CAP: i32 = 32;
/// Minimum world-space step length in metres (matches WGSL).
pub const CLOUD_MARCH_MIN_STEP_METERS: f32 = 60.0;
/// Maximum world-space step length in metres (matches WGSL).
pub const CLOUD_MARCH_MAX_STEP_METERS: f32 = 400.0;
/// Maximum integrated segment along the ray after entering the cloud band (matches WGSL).
pub const CLOUD_MARCH_MAX_SEGMENT_METERS: f32 = 40_000.0;
/// Back-compat alias for [`CLOUD_MARCH_MAX_SEGMENT_METERS`].
pub const CLOUD_MARCH_MAX_DIST_METERS: f32 = CLOUD_MARCH_MAX_SEGMENT_METERS;
/// Transmittance early-out threshold (matches WGSL).
pub const CLOUD_TRANSMITTANCE_CUTOFF: f32 = 0.01;

/// Default LOW volumetric band minimum world Y (CLOUD-VOL-1).
pub const DEFAULT_LOW_CLOUD_Y_MIN: f32 = 1800.0;
/// Default LOW volumetric band maximum world Y (CLOUD-VOL-1).
pub const DEFAULT_LOW_CLOUD_Y_MAX: f32 = 3200.0;

/// Ray/grazing epsilon shared with WGSL band intersection.
pub const CLOUD_BAND_RAY_EPSILON: f32 = 1e-5;

/// Integration stop distance along the camera ray after band entry.
///
/// Caps integrated segment length from band entry, not absolute camera distance. Shallow
/// horizon rays can enter the cloud band far from the camera; an absolute cap would skip them.
pub fn cloud_march_t_limit(t_start: f32, t_exit: f32, t_scene: f32, max_segment: f32) -> f32 {
    t_exit.min(t_scene).min(t_start + max_segment)
}

/// Step plan for one cloud-band integration segment (mirrors WGSL march helpers).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CloudMarchStepPlan {
    pub steps: i32,
    pub step_len: f32,
}

pub fn cloud_march_step_count(segment: f32) -> i32 {
    let mut steps = CLOUD_MARCH_MAX_STEPS;
    let baseline = segment / steps as f32;
    if baseline > CLOUD_MARCH_MAX_STEP_METERS {
        let needed = (segment / CLOUD_MARCH_MAX_STEP_METERS).ceil() as i32;
        steps = needed
            .max(CLOUD_MARCH_MAX_STEPS)
            .min(CLOUD_MARCH_MAX_STEPS_CAP);
    }
    steps
}

pub fn cloud_march_step_len(segment: f32, steps: i32) -> f32 {
    let ideal = segment / steps as f32;
    if ideal < CLOUD_MARCH_MIN_STEP_METERS {
        return ideal;
    }
    ideal.min(CLOUD_MARCH_MAX_STEP_METERS)
}

pub fn cloud_march_step_plan(segment: f32) -> CloudMarchStepPlan {
    let steps = cloud_march_step_count(segment);
    CloudMarchStepPlan {
        steps,
        step_len: cloud_march_step_len(segment, steps),
    }
}

/// Band-entry distance for a horizontal slab at `y_min` when the ray travels upward.
pub fn band_entry_distance_upward(origin_y: f32, direction_y: f32, y_min: f32) -> f32 {
    if direction_y <= CLOUD_BAND_RAY_EPSILON {
        return f32::INFINITY;
    }
    (y_min - origin_y) / direction_y
}

/// Per-layer procedural cloud tuning (code defaults; no dev UI in CLOUD-1).
///
/// Weather-ready seams (`coverage`, `macro_scale`, `vertical_development`, `density_scale`,
/// `edge_breakup`) are intended for future weather-field control. `anisotropy` and wind fields
/// remain layer presentation settings.
#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
pub struct CloudLayerSettings {
    /// Target cloud amount in `[0, 1]` (shader threshold mapping).
    pub coverage: f32,
    /// Macro cloud footprint scale applied after world-space XZ (1 / macro_scale ≈ base wavelength metres).
    pub macro_scale: f32,
    /// Vertical billow/stratiform development (1.0 = baseline until profile shaping in Step 3).
    pub vertical_development: f32,
    /// Optical thickness / extinction multiplier in `[0, 1]`.
    pub density_scale: f32,
    /// Edge breakup weight: erosion strength at cloud boundaries (L3 subtractive pass).
    pub edge_breakup: f32,
    /// Directional stretch for ribbon-like upper clouds (`1` = isotropic).
    pub anisotropy: f32,
    /// Horizontal wind direction in world XZ (need not be normalized).
    pub wind_direction: Vec2,
    /// World-space drift speed in meters / second (independent of day length).
    pub wind_speed: f32,
}

impl CloudLayerSettings {
    pub const fn low_default() -> Self {
        Self {
            coverage: 0.58,
            macro_scale: DEFAULT_LOW_CLOUD_MACRO_SCALE,
            vertical_development: DEFAULT_CLOUD_VERTICAL_DEVELOPMENT,
            density_scale: DEFAULT_CLOUD_DENSITY_SCALE,
            edge_breakup: DEFAULT_CLOUD_EDGE_BREAKUP,
            anisotropy: 1.0,
            wind_direction: Vec2::new(0.22, 0.08),
            wind_speed: 3.5,
        }
    }

    pub const fn high_default() -> Self {
        Self {
            coverage: 0.34,
            macro_scale: DEFAULT_HIGH_CLOUD_MACRO_SCALE,
            vertical_development: DEFAULT_CLOUD_VERTICAL_DEVELOPMENT,
            density_scale: 0.48,
            edge_breakup: 0.42,
            anisotropy: 5.5,
            wind_direction: Vec2::new(-0.35, 0.92),
            wind_speed: 14.0,
        }
    }
}

/// Fixed world-space Y interval for one volumetric cloud band.
#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
pub struct CloudAltitudeBand {
    pub y_min: f32,
    pub y_max: f32,
}

impl CloudAltitudeBand {
    pub const fn low_default() -> Self {
        Self {
            y_min: DEFAULT_LOW_CLOUD_Y_MIN,
            y_max: DEFAULT_LOW_CLOUD_Y_MAX,
        }
    }

    pub fn thickness(self) -> f32 {
        self.y_max - self.y_min
    }
}

/// Global procedural cloud configuration.
#[derive(Debug, Clone, Resource, Reflect, PartialEq)]
#[reflect(Resource)]
pub struct CloudSettings {
    pub enabled: bool,
    pub low: CloudLayerSettings,
    pub high: CloudLayerSettings,
    /// World-space LOW volumetric band (`CLOUD-VOL-1`).
    pub low_band: CloudAltitudeBand,
    /// High clouds retain illumination briefly after lower clouds enter night.
    pub high_twilight_persistence: f32,
}

impl Default for CloudSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            low: CloudLayerSettings::low_default(),
            high: CloudLayerSettings::high_default(),
            low_band: CloudAltitudeBand::low_default(),
            high_twilight_persistence: 0.22,
        }
    }
}

/// Analytic Y-band intersection for ray `origin + direction * t`.
///
/// Returns `(t_enter, t_exit)` before camera clamping, or `None` on miss.
pub fn intersect_ray_y_band(
    origin: Vec3,
    direction: Vec3,
    band: CloudAltitudeBand,
) -> Option<(f32, f32)> {
    if direction.y.abs() < CLOUD_BAND_RAY_EPSILON {
        return None;
    }

    let t0 = (band.y_min - origin.y) / direction.y;
    let t1 = (band.y_max - origin.y) / direction.y;
    let t_enter = t0.min(t1);
    let t_exit = t0.max(t1);
    if t_exit <= 0.0 || t_enter >= t_exit {
        return None;
    }
    Some((t_enter, t_exit))
}

/// Integration start distance along the camera ray after entering the band.
pub fn ray_band_march_start(t_enter: f32) -> f32 {
    t_enter.max(0.0)
}

/// World-space sample position for one march step: `origin + direction * t`.
pub fn world_ray_sample(origin: Vec3, direction: Vec3, t: f32) -> Vec3 {
    origin + direction * t
}

/// World-space wind displacement in meters from presentation elapsed time (not day length).
pub fn cloud_wind_displacement_world(layer: &CloudLayerSettings, elapsed_seconds: f32) -> Vec2 {
    let direction = layer.wind_direction.normalize_or_zero();
    if direction.length_squared() <= f32::EPSILON {
        Vec2::ZERO
    } else {
        direction * layer.wind_speed * elapsed_seconds
    }
}

/// Back-compat alias for [`cloud_wind_displacement_world`].
pub fn cloud_wind_offset(layer: &CloudLayerSettings, elapsed_seconds: f32) -> Vec2 {
    cloud_wind_displacement_world(layer, elapsed_seconds)
}

/// Procedural coordinate delta from a world-space XZ displacement (before anisotropy).
pub fn cloud_sample_delta_from_world(delta_world_xz: Vec2, scale: f32) -> Vec2 {
    delta_world_xz * scale
}

/// Approximate base noise-cell wavelength in world meters (`1 / scale`).
pub fn base_feature_wavelength_meters(scale: f32) -> f32 {
    1.0 / scale
}

/// Night weight for one layer; high strata fade into night more slowly.
pub fn layer_night_factor(base_night: f32, layer: CloudLayerId, settings: &CloudSettings) -> f32 {
    match layer {
        CloudLayerId::Low => base_night.clamp(0.0, 1.0),
        CloudLayerId::High => (base_night - settings.high_twilight_persistence).clamp(0.0, 1.0),
    }
}

/// Piecewise remap used by the cloud density shader (mirrors WGSL `remap`).
pub fn remap(v: f32, l0: f32, h0: f32, l1: f32, h1: f32) -> f32 {
    l1 + (v - l0) * (h1 - l1) / (h0 - l0).max(1e-5)
}

/// Vertical density profile within the cloud band (mirrors WGSL `height_profile`).
pub fn height_profile(height01: f32, type_field: f32) -> f32 {
    let height01 = height01.clamp(0.0, 1.0);
    let type_field = type_field.clamp(0.0, 1.0);

    let strat = smoothstep(0.05, 0.25, height01) * (1.0 - smoothstep(0.75, 0.95, height01));
    let cumulus_bottom = smoothstep(0.0, 0.12, height01);
    let cumulus_bulge = 1.0 - smoothstep(0.35, 0.65, (height01 - 0.45).abs());
    let cumulus_top = 1.0 - smoothstep(0.82, 1.0, height01);
    let cumulus = cumulus_bottom * cumulus_bulge.max(0.35) * cumulus_top;

    strat * (1.0 - type_field) + cumulus * type_field
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    if edge0 >= edge1 {
        return if x >= edge1 { 1.0 } else { 0.0 };
    }
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Height-biased erosion mix used by the cloud density shader (mirrors WGSL `erosion_field` bias).
pub fn erosion_height_bias(height01: f32) -> f32 {
    (height01 * CLOUD_EROSION_HEIGHT_BIAS).clamp(0.0, 1.0)
}

/// Apply L3 subtractive erosion remap to a body-shape sample (mirrors WGSL density composition).
pub fn apply_erosion_to_shape(shape: f32, erosion: f32, edge_breakup: f32) -> f32 {
    if shape <= 0.0 {
        return 0.0;
    }
    remap(shape, erosion * edge_breakup, 1.0, 0.0, 1.0).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_finite_and_in_range() {
        let settings = CloudSettings::default();
        for layer in [settings.low, settings.high] {
            assert!(layer.coverage.is_finite() && (0.0..=1.0).contains(&layer.coverage));
            assert!(layer.macro_scale.is_finite() && layer.macro_scale > 0.0);
            assert!(layer.vertical_development.is_finite() && layer.vertical_development > 0.0);
            assert!(layer.density_scale.is_finite() && (0.0..=1.0).contains(&layer.density_scale));
            assert!(layer.edge_breakup.is_finite() && (0.0..=1.0).contains(&layer.edge_breakup));
            assert!(layer.anisotropy.is_finite() && layer.anisotropy >= 1.0);
            assert!(layer.wind_speed.is_finite() && layer.wind_speed >= 0.0);
        }
    }

    #[test]
    fn low_band_defaults_and_thickness() {
        let settings = CloudSettings::default();
        assert!(settings.low_band.y_min < settings.low_band.y_max);
        assert!((settings.low_band.y_min - DEFAULT_LOW_CLOUD_Y_MIN).abs() < f32::EPSILON);
        assert!((settings.low_band.y_max - DEFAULT_LOW_CLOUD_Y_MAX).abs() < f32::EPSILON);
        assert!((settings.low_band.thickness() - 1400.0).abs() < f32::EPSILON);
    }

    #[test]
    fn band_intersection_below_camera_upward_ray() {
        let band = CloudAltitudeBand::low_default();
        let origin = Vec3::new(0.0, 500.0, 0.0);
        let direction = Vec3::new(0.1, 0.9, 0.1).normalize();
        let (t_enter, t_exit) = intersect_ray_y_band(origin, direction, band).unwrap();
        assert!(t_enter > 0.0);
        assert!(t_exit > t_enter);
        assert_eq!(ray_band_march_start(t_enter), t_enter);
    }

    #[test]
    fn band_intersection_inside_band() {
        let band = CloudAltitudeBand::low_default();
        let origin = Vec3::new(0.0, 2500.0, 0.0);
        let direction = Vec3::new(0.2, 0.3, 0.1).normalize();
        let (t_enter, t_exit) = intersect_ray_y_band(origin, direction, band).unwrap();
        assert!(t_enter < 0.0);
        assert!(t_exit > 0.0);
        assert_eq!(ray_band_march_start(t_enter), 0.0);
    }

    #[test]
    fn band_intersection_above_camera_downward_ray() {
        let band = CloudAltitudeBand::low_default();
        let origin = Vec3::new(0.0, 4000.0, 0.0);
        let direction = Vec3::new(0.1, -0.9, 0.1).normalize();
        let (t_enter, t_exit) = intersect_ray_y_band(origin, direction, band).unwrap();
        assert!(t_enter > 0.0);
        assert!(t_exit > t_enter);
    }

    #[test]
    fn band_intersection_ray_away_misses() {
        let band = CloudAltitudeBand::low_default();
        let origin = Vec3::new(0.0, 500.0, 0.0);
        let direction = Vec3::new(0.2, -0.9, 0.1).normalize();
        assert!(intersect_ray_y_band(origin, direction, band).is_none());
    }

    #[test]
    fn band_intersection_grazing_is_finite() {
        let band = CloudAltitudeBand::low_default();
        let origin = Vec3::new(0.0, 2500.0, 0.0);
        let direction = Vec3::new(1.0, 0.0, 0.0);
        assert!(intersect_ray_y_band(origin, direction, band).is_none());
    }

    #[test]
    fn world_ray_sample_uses_ray_distance() {
        let origin = Vec3::new(10.0, 20.0, 30.0);
        let direction = Vec3::new(0.0, 1.0, 0.0);
        let sample = world_ray_sample(origin, direction, 100.0);
        assert_eq!(sample, Vec3::new(10.0, 120.0, 30.0));
    }

    #[test]
    fn low_macro_scale_and_wind_defaults_preserved() {
        let settings = CloudSettings::default();
        assert!((settings.low.macro_scale - 0.0005).abs() < f32::EPSILON);
        assert!((settings.low.wind_speed - 3.5).abs() < f32::EPSILON);
    }

    #[test]
    fn depth_prepass_required_on_rts_camera() {
        let setup = include_str!("../camera/setup.rs");
        assert!(setup.contains("DepthPrepass"));
    }

    #[test]
    fn upper_anisotropy_exceeds_lower() {
        let settings = CloudSettings::default();
        assert!(settings.high.anisotropy > settings.low.anisotropy);
    }

    #[test]
    fn wind_vectors_differ_between_layers() {
        let settings = CloudSettings::default();
        let low = settings.low.wind_direction.normalize_or_zero();
        let high = settings.high.wind_direction.normalize_or_zero();
        assert!((low - high).length() > 0.2);
        assert!(settings.high.wind_speed > settings.low.wind_speed);
    }

    #[test]
    fn wind_displacement_scales_with_elapsed_not_day_length() {
        let layer = CloudLayerSettings::low_default();
        let a = cloud_wind_displacement_world(&layer, 120.0);
        let b = cloud_wind_displacement_world(&layer, 240.0);
        assert!((b.length() - a.length() * 2.0).abs() < 1e-3);
    }

    #[test]
    fn authored_wind_speeds_remain_world_units_per_second() {
        let settings = CloudSettings::default();
        assert!((settings.low.wind_speed - 3.5).abs() < f32::EPSILON);
        assert!((settings.high.wind_speed - 14.0).abs() < f32::EPSILON);
    }

    #[test]
    fn wind_and_translation_share_spatial_units_before_scale() {
        let layer = CloudLayerSettings::low_default();
        let direction = layer.wind_direction.normalize_or_zero();
        let one_meter_along_wind = direction;
        let from_translation =
            cloud_sample_delta_from_world(one_meter_along_wind, layer.macro_scale);
        let from_wind = cloud_sample_delta_from_world(
            cloud_wind_displacement_world(&layer, 1.0 / layer.wind_speed),
            layer.macro_scale,
        );
        assert!((from_translation - from_wind).length() < 1e-5);
    }

    #[test]
    fn default_feature_wavelengths_are_in_target_ranges() {
        let settings = CloudSettings::default();
        let low_km = base_feature_wavelength_meters(settings.low.macro_scale) / 1000.0;
        let high_km = base_feature_wavelength_meters(settings.high.macro_scale) / 1000.0;
        assert!(
            (1.5..=3.0).contains(&low_km),
            "low base wavelength {low_km} km"
        );
        assert!(
            (0.7..=1.5).contains(&high_km),
            "high base wavelength {high_km} km"
        );
    }

    #[test]
    fn one_second_low_wind_displacement_matches_speed() {
        let layer = CloudLayerSettings::low_default();
        let displacement = cloud_wind_displacement_world(&layer, 1.0);
        assert!((displacement.length() - 3.5).abs() < 1e-3);
    }

    #[test]
    fn high_layer_night_factor_persists_into_twilight() {
        let settings = CloudSettings::default();
        let base = 0.6;
        let low = layer_night_factor(base, CloudLayerId::Low, &settings);
        let high = layer_night_factor(base, CloudLayerId::High, &settings);
        assert!((low - base).abs() < f32::EPSILON);
        assert!(high < low);
    }

    #[test]
    fn weather_seam_defaults_match_v2a_baseline() {
        let low = CloudLayerSettings::low_default();
        assert!((low.coverage - 0.58).abs() < f32::EPSILON);
        assert!((low.macro_scale - DEFAULT_LOW_CLOUD_MACRO_SCALE).abs() < f32::EPSILON);
        assert!(
            (low.vertical_development - DEFAULT_CLOUD_VERTICAL_DEVELOPMENT).abs() < f32::EPSILON
        );
        assert!((low.density_scale - DEFAULT_CLOUD_DENSITY_SCALE).abs() < f32::EPSILON);
        assert!((low.edge_breakup - DEFAULT_CLOUD_EDGE_BREAKUP).abs() < f32::EPSILON);
    }

    #[test]
    fn march_constants_are_ordered_sanely() {
        assert!(CLOUD_MARCH_MAX_STEPS > 0);
        assert!(CLOUD_MARCH_MAX_STEPS_CAP >= CLOUD_MARCH_MAX_STEPS);
        assert!(CLOUD_MARCH_MIN_STEP_METERS > 0.0);
        assert!(CLOUD_MARCH_MAX_STEP_METERS > CLOUD_MARCH_MIN_STEP_METERS);
        assert!(CLOUD_MARCH_MAX_SEGMENT_METERS > 0.0);
        assert!(CLOUD_TRANSMITTANCE_CUTOFF > 0.0 && CLOUD_TRANSMITTANCE_CUTOFF < 1.0);
    }

    #[test]
    fn short_segment_uses_baseline_step_count() {
        let plan = cloud_march_step_plan(4_000.0);
        assert_eq!(plan.steps, CLOUD_MARCH_MAX_STEPS);
        assert!(plan.step_len <= CLOUD_MARCH_MAX_STEP_METERS);
    }

    #[test]
    fn long_segment_increases_step_count_up_to_cap() {
        let plan = cloud_march_step_plan(40_000.0);
        assert!(plan.steps > CLOUD_MARCH_MAX_STEPS);
        assert_eq!(plan.steps, CLOUD_MARCH_MAX_STEPS_CAP);
        assert!((plan.step_len - CLOUD_MARCH_MAX_STEP_METERS).abs() < 1e-3);
    }

    #[test]
    fn short_segment_step_len_is_not_clamped_above_slice() {
        let plan = cloud_march_step_plan(500.0);
        assert!(plan.step_len < CLOUD_MARCH_MIN_STEP_METERS);
        assert!((plan.step_len - 500.0 / plan.steps as f32).abs() < 1e-3);
    }

    #[test]
    fn t_limit_uses_nested_min_like_shader() {
        let t_start = 130_000.0_f32;
        let t_exit = 300_000.0_f32;
        let t_scene = f32::INFINITY;
        let t_limit = cloud_march_t_limit(t_start, t_exit, t_scene, CLOUD_MARCH_MAX_SEGMENT_METERS);
        assert!(t_limit > t_start);
    }

    #[test]
    fn shallow_horizon_band_entry_exceeds_old_absolute_cap() {
        let origin_y = 500.0;
        let y_min = DEFAULT_LOW_CLOUD_Y_MIN;
        let shallow_dir_y = 0.01;
        let t_enter = band_entry_distance_upward(origin_y, shallow_dir_y, y_min);
        assert!(t_enter > CLOUD_MARCH_MAX_SEGMENT_METERS);
    }

    #[test]
    fn absolute_cap_blocked_horizon_rays_old_behavior() {
        let t_start = 130_000.0_f32;
        let t_exit = 300_000.0_f32;
        let t_scene = f32::INFINITY;
        let old_limit = t_exit.min(t_scene).min(CLOUD_MARCH_MAX_SEGMENT_METERS);
        assert!(old_limit <= t_start);
    }

    #[test]
    fn segment_cap_allows_distant_band_entry() {
        let t_start = 130_000.0_f32;
        let t_exit = 300_000.0_f32;
        let t_scene = f32::INFINITY;
        let t_limit = cloud_march_t_limit(t_start, t_exit, t_scene, CLOUD_MARCH_MAX_SEGMENT_METERS);
        assert!(t_limit > t_start);
        assert!((t_limit - (t_start + CLOUD_MARCH_MAX_SEGMENT_METERS)).abs() < 1e-3);
    }

    #[test]
    fn segment_cap_still_respects_scene_occlusion() {
        let t_start = 10_000.0_f32;
        let t_exit = 200_000.0_f32;
        let t_scene = 25_000.0_f32;
        let t_limit = cloud_march_t_limit(t_start, t_exit, t_scene, CLOUD_MARCH_MAX_SEGMENT_METERS);
        assert!((t_limit - t_scene).abs() < 1e-3);
    }

    #[test]
    fn remap_endpoints_match_shader_contract() {
        assert!((remap(0.2, 0.2, 0.8, 0.0, 1.0) - 0.0).abs() < 1e-5);
        assert!((remap(0.8, 0.2, 0.8, 0.0, 1.0) - 1.0).abs() < 1e-5);
        assert!((remap(0.5, 0.2, 0.8, 0.0, 1.0) - 0.5).abs() < 1e-5);
    }

    #[test]
    fn height_profile_zero_at_band_edges() {
        assert!(height_profile(0.0, 0.0) <= 1e-5);
        assert!(height_profile(1.0, 0.0) <= 1e-5);
        assert!(height_profile(0.0, 1.0) <= 1e-5);
        assert!(height_profile(1.0, 1.0) <= 1e-5);
    }

    #[test]
    fn height_profile_positive_in_band_interior() {
        assert!(height_profile(0.5, 0.0) > 0.1);
        assert!(height_profile(0.45, 1.0) > 0.1);
    }

    #[test]
    fn cumuliform_profile_develops_lower_band_more_than_stratiform() {
        let strat_lower = height_profile(0.1, 0.0);
        let cumulus_lower = height_profile(0.1, 1.0);
        assert!(cumulus_lower > strat_lower);
    }

    #[test]
    fn erosion_height_bias_increases_with_altitude_in_band() {
        assert!(erosion_height_bias(0.0) < erosion_height_bias(0.5));
        assert!((erosion_height_bias(1.0) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn erosion_preserves_dense_interior_more_than_boundary() {
        let edge_breakup = DEFAULT_CLOUD_EDGE_BREAKUP;
        let erosion = 0.5;
        let interior = apply_erosion_to_shape(0.85, erosion, edge_breakup);
        let boundary = apply_erosion_to_shape(0.35, erosion, edge_breakup);
        assert!(interior > boundary);
        assert!(interior > 0.5);
    }

    #[test]
    fn erosion_strength_scales_with_edge_breakup() {
        let shape = 0.45;
        let erosion = 0.55;
        let low = apply_erosion_to_shape(shape, erosion, 0.2);
        let high = apply_erosion_to_shape(shape, erosion, 0.9);
        assert!(low > high);
    }
}
