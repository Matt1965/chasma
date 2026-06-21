use bevy::{
    light::GlobalAmbientLight,
    prelude::*,
};

use super::settings::EnvironmentSettings;

/// Marker for the environment-owned directional light (single instance).
#[derive(Component, Debug)]
pub struct EnvironmentDirectionalLight;

/// Prevents duplicate environment light spawns across repeated startup hooks.
#[derive(Resource, Debug, Default)]
pub struct EnvironmentLightingInitialized;

/// Configure global ambient and spawn the environment directional light.
pub fn setup_environment_lighting(
    mut commands: Commands,
    settings: Res<EnvironmentSettings>,
    mut ambient: ResMut<GlobalAmbientLight>,
    initialized: Option<Res<EnvironmentLightingInitialized>>,
) {
    if initialized.is_some() {
        #[cfg(feature = "dev")]
        bevy::log::warn!(
            target: "chasma::environment",
            "Environment lighting already initialized; skipping duplicate spawn"
        );
        return;
    }

    ambient.color = settings.ambient_color;
    ambient.brightness = settings.ambient_brightness;

    commands.spawn((
        DirectionalLight {
            color: settings.directional_light_color,
            illuminance: settings.directional_light_illuminance,
            shadows_enabled: settings.directional_shadows_enabled,
            ..default()
        },
        Transform::from_rotation(settings.directional_light_rotation),
        EnvironmentDirectionalLight,
    ));
    commands.insert_resource(EnvironmentLightingInitialized);
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::app::App;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn lighting_setup_spawns_single_directional_light() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<EnvironmentSettings>();
        app.init_resource::<GlobalAmbientLight>();
        app.world_mut()
            .run_system_once(setup_environment_lighting)
            .unwrap();
        app.world_mut()
            .run_system_once(setup_environment_lighting)
            .unwrap();

        let mut world = app.world_mut();
        let lights = world
            .query::<&EnvironmentDirectionalLight>()
            .iter(&mut world)
            .count();
        assert_eq!(lights, 1);
        assert!(world.get_resource::<EnvironmentLightingInitialized>().is_some());
    }
}
