//! Project-level environment defaults (Slice 11).
//!
//! Three-layer model:
//! 1. Built-in defaults — [`built_in_authored_snapshot`]
//! 2. Project defaults — `assets/environment/project_defaults.ron`
//! 3. Runtime — [`TimeOfDaySettings`] + [`EnvironmentSettings`]

use std::fs;
use std::path::{Path, PathBuf};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::settings::EnvironmentSettings;
use super::time_of_day::TimeOfDaySettings;

/// On-disk path relative to the project / working directory.
pub const PROJECT_DEFAULTS_PATH: &str = "assets/environment/project_defaults.ron";

pub const PROJECT_DEFAULTS_VERSION: u32 = 2;

const FLOAT_EPS: f32 = 0.05;
const FLOAT_EPS_NORM: f32 = 0.001;

/// How project defaults were obtained at startup.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ProjectDefaultsLoadStatus {
    #[default]
    NotLoaded,
    LoadedFromFile,
    MissingFileUsedBuiltIn,
    InvalidFileUsedBuiltIn {
        error: String,
    },
}

impl ProjectDefaultsLoadStatus {
    pub fn summary(&self) -> &'static str {
        match self {
            Self::NotLoaded => "not loaded",
            Self::LoadedFromFile => "loaded from project file",
            Self::MissingFileUsedBuiltIn => "missing file — using built-in defaults",
            Self::InvalidFileUsedBuiltIn { .. } => "invalid file — using built-in defaults",
        }
    }
}

/// Manual lighting values used when the visual cycle is disabled.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManualLightingDefaults {
    pub directional_illuminance: f32,
    pub ambient_brightness: f32,
}

impl Default for ManualLightingDefaults {
    fn default() -> Self {
        let env = EnvironmentSettings::default();
        Self {
            directional_illuminance: env.directional_light_illuminance,
            ambient_brightness: env.ambient_brightness,
        }
    }
}

/// Authored environment snapshot (persisted + dirty comparison).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuthoredEnvironmentSnapshot {
    #[serde(default = "default_version")]
    pub version: u32,
    pub time_of_day: AuthoredTimeOfDay,
    #[serde(default)]
    pub manual_lighting: ManualLightingDefaults,
}

fn default_version() -> u32 {
    PROJECT_DEFAULTS_VERSION
}

/// Persisted time-of-day fields (excludes transient `time_hours` and `paused`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuthoredTimeOfDay {
    pub enabled: bool,
    pub day_length_seconds: f32,
    pub sun_pitch_min_deg: f32,
    pub sun_pitch_max_deg: f32,
    pub sunrise_hour: f32,
    pub sunset_hour: f32,
    pub night_ambient_multiplier: f32,
    pub noon_directional_illuminance: f32,
    pub night_directional_illuminance: f32,
    pub noon_ambient_brightness: f32,
    pub twilight_daylight_blend: f32,
}

impl Default for AuthoredTimeOfDay {
    fn default() -> Self {
        Self::from_settings(&TimeOfDaySettings::default())
    }
}

impl AuthoredTimeOfDay {
    pub fn from_settings(settings: &TimeOfDaySettings) -> Self {
        Self {
            enabled: settings.enabled,
            day_length_seconds: settings.day_length_seconds,
            sun_pitch_min_deg: settings.sun_pitch_min_deg,
            sun_pitch_max_deg: settings.sun_pitch_max_deg,
            sunrise_hour: settings.sunrise_hour,
            sunset_hour: settings.sunset_hour,
            night_ambient_multiplier: settings.night_ambient_multiplier,
            noon_directional_illuminance: settings.noon_directional_illuminance,
            night_directional_illuminance: settings.night_directional_illuminance,
            noon_ambient_brightness: settings.noon_ambient_brightness,
            twilight_daylight_blend: settings.twilight_daylight_blend,
        }
    }

    pub fn apply_to(&self, settings: &mut TimeOfDaySettings) {
        settings.enabled = self.enabled;
        settings.day_length_seconds = self.day_length_seconds;
        settings.sun_pitch_min_deg = self.sun_pitch_min_deg;
        settings.sun_pitch_max_deg = self.sun_pitch_max_deg;
        settings.sunrise_hour = self.sunrise_hour;
        settings.sunset_hour = self.sunset_hour;
        settings.night_ambient_multiplier = self.night_ambient_multiplier;
        settings.noon_directional_illuminance = self.noon_directional_illuminance;
        settings.night_directional_illuminance = self.night_directional_illuminance;
        settings.noon_ambient_brightness = self.noon_ambient_brightness;
        settings.twilight_daylight_blend = self.twilight_daylight_blend;
    }
}

impl Default for AuthoredEnvironmentSnapshot {
    fn default() -> Self {
        built_in_authored_snapshot()
    }
}

impl AuthoredEnvironmentSnapshot {
    pub fn from_runtime(time_of_day: &TimeOfDaySettings, manual: &ManualLightingDefaults) -> Self {
        Self {
            version: PROJECT_DEFAULTS_VERSION,
            time_of_day: AuthoredTimeOfDay::from_settings(time_of_day),
            manual_lighting: manual.clone(),
        }
    }

    pub fn apply_to_runtime(
        &self,
        time_of_day: &mut TimeOfDaySettings,
        environment: &mut EnvironmentSettings,
        manual: &mut ManualLightingDefaults,
    ) {
        self.time_of_day.apply_to(time_of_day);
        *manual = self.manual_lighting.clone();
        if !time_of_day.enabled {
            apply_manual_lighting(environment, manual);
        }
    }

    pub fn semantic_equals(&self, other: &Self) -> bool {
        self.time_of_day.semantic_equals(&other.time_of_day)
            && self.manual_lighting.semantic_equals(&other.manual_lighting)
    }
}

impl AuthoredTimeOfDay {
    pub fn semantic_equals(&self, other: &Self) -> bool {
        self.enabled == other.enabled
            && approx_eq(self.day_length_seconds, other.day_length_seconds, FLOAT_EPS)
            && approx_eq(self.sun_pitch_min_deg, other.sun_pitch_min_deg, FLOAT_EPS)
            && approx_eq(self.sun_pitch_max_deg, other.sun_pitch_max_deg, FLOAT_EPS)
            && approx_eq(self.sunrise_hour, other.sunrise_hour, FLOAT_EPS_NORM)
            && approx_eq(self.sunset_hour, other.sunset_hour, FLOAT_EPS_NORM)
            && approx_eq(
                self.night_ambient_multiplier,
                other.night_ambient_multiplier,
                FLOAT_EPS_NORM,
            )
            && approx_eq(
                self.noon_directional_illuminance,
                other.noon_directional_illuminance,
                FLOAT_EPS,
            )
            && approx_eq(
                self.night_directional_illuminance,
                other.night_directional_illuminance,
                FLOAT_EPS,
            )
            && approx_eq(
                self.noon_ambient_brightness,
                other.noon_ambient_brightness,
                FLOAT_EPS,
            )
            && approx_eq(
                self.twilight_daylight_blend,
                other.twilight_daylight_blend,
                FLOAT_EPS_NORM,
            )
    }
}

impl ManualLightingDefaults {
    pub fn from_environment(environment: &EnvironmentSettings) -> Self {
        Self {
            directional_illuminance: environment.directional_light_illuminance,
            ambient_brightness: environment.ambient_brightness,
        }
    }

    pub fn semantic_equals(&self, other: &Self) -> bool {
        approx_eq(
            self.directional_illuminance,
            other.directional_illuminance,
            FLOAT_EPS,
        ) && approx_eq(self.ambient_brightness, other.ambient_brightness, FLOAT_EPS)
    }
}

/// Immutable built-in baseline from code defaults.
pub fn built_in_authored_snapshot() -> AuthoredEnvironmentSnapshot {
    AuthoredEnvironmentSnapshot::from_runtime(
        &TimeOfDaySettings::default(),
        &ManualLightingDefaults::default(),
    )
}

/// Loaded project baseline and load metadata (read in dev + release).
#[derive(Resource, Debug, Clone)]
pub struct ProjectEnvironmentBaseline {
    pub snapshot: AuthoredEnvironmentSnapshot,
    pub load_status: ProjectDefaultsLoadStatus,
    pub source_path: PathBuf,
}

impl Default for ProjectEnvironmentBaseline {
    fn default() -> Self {
        Self {
            snapshot: built_in_authored_snapshot(),
            load_status: ProjectDefaultsLoadStatus::NotLoaded,
            source_path: PathBuf::from(PROJECT_DEFAULTS_PATH),
        }
    }
}

/// Working manual-lighting copy (updated when manual controls change).
#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub struct EnvironmentManualLighting {
    pub values: ManualLightingDefaults,
}

impl EnvironmentManualLighting {
    pub fn sync_from_environment(&mut self, environment: &EnvironmentSettings) {
        self.values = ManualLightingDefaults::from_environment(environment);
    }
}

pub fn apply_manual_lighting(
    environment: &mut EnvironmentSettings,
    manual: &ManualLightingDefaults,
) {
    environment.directional_light_illuminance = manual.directional_illuminance;
    environment.ambient_brightness = manual.ambient_brightness;
}

/// Validation errors block project save.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvironmentValidationError {
    NonFinite(&'static str),
    OutOfRange { field: &'static str, reason: String },
    TwilightOrdering,
}

impl EnvironmentValidationError {
    pub fn message(&self) -> String {
        match self {
            Self::NonFinite(field) => format!("{field} must be a finite number"),
            Self::OutOfRange { field, reason } => format!("{field}: {reason}"),
            Self::TwilightOrdering => "Sunrise hour must be less than sunset hour".to_string(),
        }
    }
}

pub fn validate_authored_snapshot(
    snapshot: &AuthoredEnvironmentSnapshot,
) -> Result<(), EnvironmentValidationError> {
    let t = &snapshot.time_of_day;
    check_finite("day_length_seconds", t.day_length_seconds)?;
    check_finite("sun_pitch_min_deg", t.sun_pitch_min_deg)?;
    check_finite("sun_pitch_max_deg", t.sun_pitch_max_deg)?;
    check_finite("sunrise_hour", t.sunrise_hour)?;
    check_finite("sunset_hour", t.sunset_hour)?;
    check_finite("night_ambient_multiplier", t.night_ambient_multiplier)?;
    check_finite(
        "noon_directional_illuminance",
        t.noon_directional_illuminance,
    )?;
    check_finite(
        "night_directional_illuminance",
        t.night_directional_illuminance,
    )?;
    check_finite("noon_ambient_brightness", t.noon_ambient_brightness)?;
    check_finite("twilight_daylight_blend", t.twilight_daylight_blend)?;

    if t.day_length_seconds < 30.0 || t.day_length_seconds > 3600.0 {
        return Err(EnvironmentValidationError::OutOfRange {
            field: "day_length_seconds",
            reason: "expected 30–3600 seconds".to_string(),
        });
    }
    if t.sun_pitch_min_deg < -90.0 || t.sun_pitch_min_deg > 90.0 {
        return Err(EnvironmentValidationError::OutOfRange {
            field: "sun_pitch_min_deg",
            reason: "expected -90–90 degrees".to_string(),
        });
    }
    if t.sun_pitch_max_deg < -90.0 || t.sun_pitch_max_deg > 90.0 {
        return Err(EnvironmentValidationError::OutOfRange {
            field: "sun_pitch_max_deg",
            reason: "expected -90–90 degrees".to_string(),
        });
    }
    if t.sunrise_hour < 0.0 || t.sunrise_hour > 23.0 {
        return Err(EnvironmentValidationError::OutOfRange {
            field: "sunrise_hour",
            reason: "expected 0–23 hours".to_string(),
        });
    }
    if t.sunset_hour < 0.0 || t.sunset_hour > 24.0 {
        return Err(EnvironmentValidationError::OutOfRange {
            field: "sunset_hour",
            reason: "expected 0–24 hours".to_string(),
        });
    }
    if t.sunrise_hour >= t.sunset_hour {
        return Err(EnvironmentValidationError::TwilightOrdering);
    }
    if t.night_ambient_multiplier < 0.0 || t.night_ambient_multiplier > 2.0 {
        return Err(EnvironmentValidationError::OutOfRange {
            field: "night_ambient_multiplier",
            reason: "expected 0–2".to_string(),
        });
    }
    if t.twilight_daylight_blend < 0.0 || t.twilight_daylight_blend > 1.0 {
        return Err(EnvironmentValidationError::OutOfRange {
            field: "twilight_daylight_blend",
            reason: "expected 0–1".to_string(),
        });
    }
    if t.noon_directional_illuminance < 0.0 {
        return Err(EnvironmentValidationError::OutOfRange {
            field: "noon_directional_illuminance",
            reason: "must be non-negative".to_string(),
        });
    }
    if t.night_directional_illuminance < 0.0 {
        return Err(EnvironmentValidationError::OutOfRange {
            field: "night_directional_illuminance",
            reason: "must be non-negative".to_string(),
        });
    }

    let m = &snapshot.manual_lighting;
    check_finite("manual_directional_illuminance", m.directional_illuminance)?;
    check_finite("manual_ambient_brightness", m.ambient_brightness)?;

    Ok(())
}

fn check_finite(field: &'static str, value: f32) -> Result<(), EnvironmentValidationError> {
    if !value.is_finite() {
        return Err(EnvironmentValidationError::NonFinite(field));
    }
    Ok(())
}

fn approx_eq(a: f32, b: f32, eps: f32) -> bool {
    (a - b).abs() <= eps
}

/// Load project defaults from disk; fall back to built-in on missing/invalid.
pub fn load_project_environment_baseline(path: &Path) -> ProjectEnvironmentBaseline {
    let built_in = built_in_authored_snapshot();
    match fs::read_to_string(path) {
        Ok(text) => match ron::from_str::<AuthoredEnvironmentSnapshot>(&text) {
            Ok(mut snapshot) => {
                if snapshot.version == 0 {
                    snapshot.version = PROJECT_DEFAULTS_VERSION;
                }
                match validate_authored_snapshot(&snapshot) {
                    Ok(()) => ProjectEnvironmentBaseline {
                        snapshot,
                        load_status: ProjectDefaultsLoadStatus::LoadedFromFile,
                        source_path: path.to_path_buf(),
                    },
                    Err(err) => {
                        bevy::log::warn!(
                            target: "chasma::environment",
                            "Invalid project environment defaults at {}: {} — using built-in",
                            path.display(),
                            err.message()
                        );
                        ProjectEnvironmentBaseline {
                            snapshot: built_in,
                            load_status: ProjectDefaultsLoadStatus::InvalidFileUsedBuiltIn {
                                error: err.message(),
                            },
                            source_path: path.to_path_buf(),
                        }
                    }
                }
            }
            Err(err) => {
                bevy::log::warn!(
                    target: "chasma::environment",
                    "Failed to parse project environment defaults at {}: {err} — using built-in",
                    path.display()
                );
                ProjectEnvironmentBaseline {
                    snapshot: built_in,
                    load_status: ProjectDefaultsLoadStatus::InvalidFileUsedBuiltIn {
                        error: err.to_string(),
                    },
                    source_path: path.to_path_buf(),
                }
            }
        },
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            bevy::log::info!(
                target: "chasma::environment",
                "No project environment defaults at {} — using built-in",
                path.display()
            );
            ProjectEnvironmentBaseline {
                snapshot: built_in,
                load_status: ProjectDefaultsLoadStatus::MissingFileUsedBuiltIn,
                source_path: path.to_path_buf(),
            }
        }
        Err(err) => {
            bevy::log::warn!(
                target: "chasma::environment",
                "Failed to read project environment defaults at {}: {err} — using built-in",
                path.display()
            );
            ProjectEnvironmentBaseline {
                snapshot: built_in,
                load_status: ProjectDefaultsLoadStatus::InvalidFileUsedBuiltIn {
                    error: err.to_string(),
                },
                source_path: path.to_path_buf(),
            }
        }
    }
}

/// Initialize runtime resources from a loaded baseline.
pub fn initialize_runtime_from_baseline(
    baseline: &ProjectEnvironmentBaseline,
    time_of_day: &mut TimeOfDaySettings,
    environment: &mut EnvironmentSettings,
    manual: &mut EnvironmentManualLighting,
) {
    baseline
        .snapshot
        .apply_to_runtime(time_of_day, environment, &mut manual.values);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectDefaultsSaveError {
    Validation(EnvironmentValidationError),
    Io(String),
    Serialize(String),
}

impl std::fmt::Display for ProjectDefaultsSaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Validation(err) => write!(f, "validation: {}", err.message()),
            Self::Io(msg) => write!(f, "io: {msg}"),
            Self::Serialize(msg) => write!(f, "serialize: {msg}"),
        }
    }
}

/// Write project defaults atomically (dev authoring only).
pub fn save_project_environment_defaults(
    path: &Path,
    snapshot: &AuthoredEnvironmentSnapshot,
) -> Result<(), ProjectDefaultsSaveError> {
    validate_authored_snapshot(snapshot).map_err(ProjectDefaultsSaveError::Validation)?;

    let text = ron::ser::to_string_pretty(snapshot, ron::ser::PrettyConfig::default())
        .map_err(|err| ProjectDefaultsSaveError::Serialize(err.to_string()))?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            ProjectDefaultsSaveError::Io(format!("create_dir_all {}: {err}", parent.display()))
        })?;
    }

    let temp_path = path.with_extension("ron.tmp");
    fs::write(&temp_path, &text).map_err(|err| {
        ProjectDefaultsSaveError::Io(format!("write {}: {err}", temp_path.display()))
    })?;
    fs::rename(&temp_path, path)
        .map_err(|err| ProjectDefaultsSaveError::Io(format!("rename {}: {err}", path.display())))?;

    Ok(())
}

pub fn capture_current_authored_snapshot(
    time_of_day: &TimeOfDaySettings,
    manual: &EnvironmentManualLighting,
) -> AuthoredEnvironmentSnapshot {
    AuthoredEnvironmentSnapshot::from_runtime(time_of_day, &manual.values)
}

pub fn environment_is_dirty(
    baseline: &ProjectEnvironmentBaseline,
    time_of_day: &TimeOfDaySettings,
    manual: &EnvironmentManualLighting,
) -> bool {
    let current = capture_current_authored_snapshot(time_of_day, manual);
    !baseline.snapshot.semantic_equals(&current)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn built_in_snapshot_matches_defaults() {
        let snap = built_in_authored_snapshot();
        assert!(snap.time_of_day.enabled);
        assert_eq!(snap.version, PROJECT_DEFAULTS_VERSION);
    }

    #[test]
    fn semantic_equals_ignores_tiny_float_noise() {
        let mut a = built_in_authored_snapshot();
        let mut b = a.clone();
        b.time_of_day.noon_directional_illuminance += 0.001;
        assert!(a.semantic_equals(&b));
        b.time_of_day.noon_directional_illuminance += 1.0;
        assert!(!a.semantic_equals(&b));
    }

    #[test]
    fn twilight_validation_rejects_inverted_hours() {
        let mut snap = built_in_authored_snapshot();
        snap.time_of_day.sunrise_hour = 18.0;
        snap.time_of_day.sunset_hour = 6.0;
        assert_eq!(
            validate_authored_snapshot(&snap),
            Err(EnvironmentValidationError::TwilightOrdering)
        );
    }

    #[test]
    fn round_trip_serialization() {
        let snap = built_in_authored_snapshot();
        let text = ron::ser::to_string_pretty(&snap, ron::ser::PrettyConfig::default()).unwrap();
        let parsed: AuthoredEnvironmentSnapshot = ron::from_str(&text).unwrap();
        assert!(snap.semantic_equals(&parsed));
    }

    #[test]
    fn legacy_v1_defaults_deserialize_without_skybox_fields() {
        let legacy = r#"
(
    version: 1,
    time_of_day: (
        enabled: true,
        day_length_seconds: 600.0,
        sun_pitch_min_deg: -34.0,
        sun_pitch_max_deg: 26.0,
        sunrise_hour: 6.0,
        sunset_hour: 18.0,
        night_ambient_multiplier: 1.77,
        noon_directional_illuminance: 24000.0,
        night_directional_illuminance: 40.0,
        noon_ambient_brightness: 320.0,
        noon_skybox_brightness: 1200.0,
        night_skybox_brightness: 160.0,
        twilight_daylight_blend: 0.5,
    ),
    environment: (
        skybox_set: "default",
        skybox_rotation_yaw_deg: 0.0,
    ),
    manual_lighting: (
        directional_illuminance: 21189.0,
        ambient_brightness: 349.0,
        skybox_brightness: 1078.0,
    ),
)
"#;
        let parsed: AuthoredEnvironmentSnapshot = ron::from_str(legacy).unwrap();
        assert!(validate_authored_snapshot(&parsed).is_ok());
        assert_eq!(parsed.version, 1);
    }

    #[test]
    fn missing_file_uses_built_in() {
        let path = std::env::temp_dir().join(format!(
            "chasma_missing_defaults_{}.ron",
            std::process::id()
        ));
        let _ = fs::remove_file(&path);
        let baseline = load_project_environment_baseline(&path);
        assert_eq!(
            baseline.load_status,
            ProjectDefaultsLoadStatus::MissingFileUsedBuiltIn
        );
        assert!(
            baseline
                .snapshot
                .semantic_equals(&built_in_authored_snapshot())
        );
    }

    #[test]
    fn invalid_file_uses_built_in() {
        let path = std::env::temp_dir().join(format!(
            "chasma_invalid_defaults_{}.ron",
            std::process::id()
        ));
        let mut file = fs::File::create(&path).unwrap();
        writeln!(file, "not valid ron").unwrap();
        let baseline = load_project_environment_baseline(&path);
        assert!(matches!(
            baseline.load_status,
            ProjectDefaultsLoadStatus::InvalidFileUsedBuiltIn { .. }
        ));
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn save_and_reload_round_trip() {
        let path =
            std::env::temp_dir().join(format!("chasma_save_defaults_{}.ron", std::process::id()));
        let mut snap = built_in_authored_snapshot();
        snap.time_of_day.day_length_seconds = 900.0;
        save_project_environment_defaults(&path, &snap).unwrap();
        let loaded = load_project_environment_baseline(&path);
        assert_eq!(
            loaded.load_status,
            ProjectDefaultsLoadStatus::LoadedFromFile
        );
        assert!(loaded.snapshot.semantic_equals(&snap));
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn dirty_state_tracks_meaningful_change() {
        let baseline = ProjectEnvironmentBaseline {
            snapshot: built_in_authored_snapshot(),
            load_status: ProjectDefaultsLoadStatus::LoadedFromFile,
            source_path: PathBuf::from(PROJECT_DEFAULTS_PATH),
        };
        let mut time = TimeOfDaySettings::default();
        let mut env = EnvironmentSettings::default();
        let mut manual = EnvironmentManualLighting::default();
        initialize_runtime_from_baseline(&baseline, &mut time, &mut env, &mut manual);
        assert!(!environment_is_dirty(&baseline, &time, &manual));
        time.day_length_seconds = 120.0;
        assert!(environment_is_dirty(&baseline, &time, &manual));
    }

    #[test]
    fn reset_to_built_in_differs_from_custom_baseline() {
        let mut baseline = built_in_authored_snapshot();
        baseline.time_of_day.day_length_seconds = 120.0;
        let built_in = built_in_authored_snapshot();
        assert!(!baseline.semantic_equals(&built_in));
    }
}
