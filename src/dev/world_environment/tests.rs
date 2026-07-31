//! World environment UI tests (Slice 11).

use crate::environment::{EnvironmentManualLighting, EnvironmentSettings, TimeOfDaySettings};
use crate::environment::{
    ProjectDefaultsLoadStatus, ProjectEnvironmentBaseline, built_in_authored_snapshot,
    environment_is_dirty,
};

use super::fields::EnvFieldId;

#[test]
fn field_specs_have_valid_ranges() {
    for field in EnvFieldId::ALL {
        let spec = field.spec();
        assert!(spec.max > spec.min, "{:?}", field);
    }
}

#[test]
fn manual_fields_round_trip() {
    let mut time = TimeOfDaySettings::default();
    let mut env = EnvironmentSettings::default();
    let mut manual = EnvironmentManualLighting::default();
    EnvFieldId::ManualDirectional.write(12_345.0, &mut time, &mut env, &mut manual);
    assert!((manual.values.directional_illuminance - 12_345.0).abs() < f32::EPSILON);
}

#[test]
fn dirty_after_reset_from_custom_baseline() {
    let baseline = ProjectEnvironmentBaseline {
        snapshot: {
            let mut s = built_in_authored_snapshot();
            s.time_of_day.day_length_seconds = 200.0;
            s
        },
        load_status: ProjectDefaultsLoadStatus::LoadedFromFile,
        source_path: std::path::PathBuf::from(crate::environment::PROJECT_DEFAULTS_PATH),
    };
    let mut time = TimeOfDaySettings::default();
    let mut env = EnvironmentSettings::default();
    let mut manual = EnvironmentManualLighting::default();
    crate::environment::initialize_runtime_from_baseline(
        &baseline,
        &mut time,
        &mut env,
        &mut manual,
    );
    assert!(!environment_is_dirty(&baseline, &time, &env, &manual));
    let built_in = built_in_authored_snapshot();
    built_in.apply_to_runtime(&mut time, &mut env, &mut manual.values);
    assert!(environment_is_dirty(&baseline, &time, &env, &manual));
}
