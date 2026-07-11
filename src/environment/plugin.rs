use bevy::prelude::*;

use super::cycle::{
    advance_time_of_day, sync_environment_presentation, update_environment_from_time_of_day,
};
#[cfg(feature = "dev")]
use super::debug::{
    count_environment_singletons, log_environment_configuration, log_environment_singleton_report,
};
use super::lighting::setup_environment_lighting;
use super::settings::EnvironmentSettings;
use super::skybox::{ActiveSkyboxLoad, attach_skybox_to_primary_camera, init_skybox_load};
use super::time_of_day::TimeOfDaySettings;
use super::water::WaterPlugin;

/// Environment rendering layer: skybox, ambient light, and directional light (R8 / R9 / E10).
pub struct EnvironmentPlugin;

impl Plugin for EnvironmentPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<EnvironmentSettings>()
            .register_type::<TimeOfDaySettings>()
            .init_resource::<EnvironmentSettings>()
            .init_resource::<TimeOfDaySettings>()
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
        app.add_systems(
            Update,
            super::cycle::time_of_day_dev_keyboard.after(advance_time_of_day),
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

    #[test]
    fn plugin_initializes_environment_settings_resource() {
        let mut app = App::new();
        app.add_plugins(EnvironmentPlugin);
        assert!(app.world().get_resource::<EnvironmentSettings>().is_some());
        assert!(app.world().get_resource::<TimeOfDaySettings>().is_some());
        assert!(
            app.world()
                .get_resource::<crate::environment::WaterSettings>()
                .is_some()
        );
    }
}
