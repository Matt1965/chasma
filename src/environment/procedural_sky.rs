//! Procedural sky dome spawn and sync (SKY-1).

use bevy::light::NotShadowCaster;
use bevy::math::primitives::Sphere;
use bevy::pbr::MeshMaterial3d;
use bevy::prelude::*;

use crate::camera::RtsCamera;

use super::sky_material::{EnvironmentSkyMaterial, build_sky_material};
use super::visual_state::{DEFAULT_SKY_PRESENTATION, EnvironmentVisualState, SkyPresentation};

/// Marker for the environment-owned procedural sky dome (single instance).
#[derive(Component, Debug)]
pub struct EnvironmentProceduralSky;

/// Prevents duplicate procedural sky spawns across repeated startup hooks.
#[derive(Resource, Debug, Default)]
pub struct ProceduralSkySpawnState {
    pub spawned: bool,
}

const SKY_DOME_RADIUS: f32 = 5_000.0;

/// Spawn a camera-centered sky dome when procedural presentation is active.
pub fn setup_procedural_sky(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<EnvironmentSkyMaterial>>,
    visual: Res<EnvironmentVisualState>,
    mut spawn_state: ResMut<ProceduralSkySpawnState>,
) {
    if spawn_state.spawned || DEFAULT_SKY_PRESENTATION != SkyPresentation::Procedural {
        return;
    }

    let mesh = meshes.add(Sphere::new(SKY_DOME_RADIUS));
    let material = materials.add(build_sky_material(&visual));

    commands.spawn((
        EnvironmentProceduralSky,
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform::IDENTITY,
        Visibility::Visible,
        NotShadowCaster,
    ));
    spawn_state.spawned = true;
}

/// Follow the RTS camera translation and sync sky material uniforms from derived state.
pub fn sync_procedural_sky_presentation(
    visual: Res<EnvironmentVisualState>,
    camera: Query<&GlobalTransform, With<RtsCamera>>,
    mut sky: Query<
        (&mut Transform, &MeshMaterial3d<EnvironmentSkyMaterial>),
        With<EnvironmentProceduralSky>,
    >,
    mut materials: ResMut<Assets<EnvironmentSkyMaterial>>,
) {
    if DEFAULT_SKY_PRESENTATION != SkyPresentation::Procedural {
        return;
    }

    let Ok(camera_transform) = camera.single() else {
        return;
    };

    for (mut transform, material_handle) in &mut sky {
        transform.translation = camera_transform.translation();
        transform.rotation = Quat::IDENTITY;
        transform.scale = Vec3::ONE;

        if let Some(material) = materials.get_mut(&material_handle.0) {
            material.params = super::sky_material::build_sky_presentation_uniform(&visual);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::{
        EnvironmentVisualState, TimeOfDaySettings, evaluate_environment_visual_state,
    };
    use bevy::app::App;
    use bevy::asset::AssetPlugin;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::mesh::MeshPlugin;
    use bevy::pbr::MaterialPlugin;

    use super::super::sky_material::EnvironmentSkyMaterial;

    #[test]
    fn procedural_mode_spawns_single_sky_dome() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            MeshPlugin,
            MaterialPlugin::<EnvironmentSkyMaterial>::default(),
        ));
        app.init_resource::<ProceduralSkySpawnState>();
        app.insert_resource(evaluate_environment_visual_state(
            &TimeOfDaySettings::default(),
            &super::super::visual_state::SkyColorPalette::default(),
        ));
        app.world_mut()
            .run_system_once(setup_procedural_sky)
            .unwrap();
        app.world_mut()
            .run_system_once(setup_procedural_sky)
            .unwrap();

        let mut world = app.world_mut();
        let count = world
            .query_filtered::<(), With<EnvironmentProceduralSky>>()
            .iter(&mut world)
            .count();
        assert_eq!(count, 1);
    }
}
