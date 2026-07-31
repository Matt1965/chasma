use bevy::prelude::*;

use super::cycle::{
    advance_time_of_day, sync_environment_presentation, update_environment_from_time_of_day,
};
#[cfg(feature = "dev")]
use super::debug::{
    count_environment_singletons, log_environment_configuration, log_environment_singleton_report,
};
use super::lighting::setup_environment_lighting;
use super::project_defaults::{
    EnvironmentManualLighting, PROJECT_DEFAULTS_PATH, ProjectEnvironmentBaseline,
    initialize_runtime_from_baseline, load_project_environment_baseline,
};
use super::settings::EnvironmentSettings;
use super::skybox::{ActiveSkyboxLoad, attach_skybox_to_primary_camera, init_skybox_load};
use super::time_of_day::TimeOfDaySettings;
use super::water::WaterPlugin;

/// Environment rendering layer: skybox, ambient light, and directional light (R8 / R9 / E10).
pub struct EnvironmentPlugin;

impl Plugin for EnvironmentPlugin {
    fn build(&self, app: &mut App) {
        let baseline =
            load_project_environment_baseline(std::path::Path::new(PROJECT_DEFAULTS_PATH));
        let mut time_of_day = TimeOfDaySettings::default();
        let mut environment = EnvironmentSettings::default();
        let mut manual = EnvironmentManualLighting::default();
        initialize_runtime_from_baseline(
            &baseline,
            &mut time_of_day,
            &mut environment,
            &mut manual,
        );

        app.register_type::<EnvironmentSettings>()
            .register_type::<TimeOfDaySettings>()
            .insert_resource(baseline)
            .insert_resource(time_of_day)
            .insert_resource(environment)
            .insert_resource(manual)
            .add_plugins(WaterPlugin)
            .add_systems(
                Startup,
                (
                    setup_environment_lighting,
                    init_skybox_load,
                    log_environment_startup,
                )
                    .chain(),
            )
            .add_systems(
                Update,
                (
                    advance_time_of_day,
                    update_environment_from_time_of_day,
                    sync_environment_presentation,
                    attach_skybox_to_primary_camera,
                )
                    .chain(),
            );

        #[cfg(feature = "dev")]
        app.add_systems(PostStartup, validate_environment_startup);
    }
}

fn log_environment_startup(
    settings: Res<EnvironmentSettings>,
    load: Option<Res<ActiveSkyboxLoad>>,
) {
    #[cfg(feature = "dev")]
    {
        bevy::log::info!(target: "chasma::environment", "Environment initialized");
        log_environment_configuration(&settings);
        if load.is_some() {
            bevy::log::info!(target: "chasma::environment", "Skybox load started");
        } else {
            bevy::log::info!(target: "chasma::environment", "Skybox missing");
        }
    }

    let _ = (settings, load);
}

#[cfg(feature = "dev")]
fn validate_environment_startup(
    settings: Res<EnvironmentSettings>,
    directional: Query<(), With<DirectionalLight>>,
    environment_directional: Query<(), With<super::lighting::EnvironmentDirectionalLight>>,
    skybox_cameras: Query<(), With<super::skybox::SkyboxCamera>>,
) {
    let _ = &settings;
    let report = count_environment_singletons(directional, environment_directional, skybox_cameras);
    log_environment_singleton_report(&report);
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::app::App;

    use crate::environment::{
        EnvironmentManualLighting, TimeOfDaySettings, initialize_runtime_from_baseline,
        load_project_environment_baseline,
    };

    #[test]
    fn plugin_initializes_environment_settings_resource() {
        let mut app = App::new();
        app.add_plugins(EnvironmentPlugin);
        assert!(app.world().get_resource::<EnvironmentSettings>().is_some());
        assert!(app.world().get_resource::<TimeOfDaySettings>().is_some());
        assert!(
            app.world()
                .get_resource::<ProjectEnvironmentBaseline>()
                .is_some()
        );
        assert!(
            app.world()
                .get_resource::<EnvironmentManualLighting>()
                .is_some()
        );
        assert!(
            app.world()
                .get_resource::<crate::environment::WaterSettings>()
                .is_some()
        );
    }

    #[test]
    fn bootstrap_does_not_use_hardcoded_defaults_when_file_differs() {
        let path =
            std::env::temp_dir().join(format!("chasma_plugin_defaults_{}.ron", std::process::id()));
        let mut snap = crate::environment::built_in_authored_snapshot();
        snap.time_of_day.day_length_seconds = 777.0;
        crate::environment::save_project_environment_defaults(&path, &snap).unwrap();

        let baseline = load_project_environment_baseline(&path);
        let mut time = TimeOfDaySettings::default();
        let mut env = EnvironmentSettings::default();
        let mut manual = EnvironmentManualLighting::default();
        initialize_runtime_from_baseline(&baseline, &mut time, &mut env, &mut manual);
        assert!((time.day_length_seconds - 777.0).abs() < f32::EPSILON);
        let _ = std::fs::remove_file(&path);
    }
}
