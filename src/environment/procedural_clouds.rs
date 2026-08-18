//! Procedural volumetric cloud proxy spawn and sync (CLOUD-VOL-1).

use bevy::light::NotShadowCaster;
use bevy::math::primitives::Sphere;
use bevy::pbr::MeshMaterial3d;
use bevy::prelude::*;

use crate::camera::RtsCamera;

use super::cloud_material::{EnvironmentCloudMaterial, build_cloud_layer_uniform};
use super::cloud_settings::{CloudSettings, cloud_wind_displacement_world};
use super::visual_state::{DEFAULT_SKY_PRESENTATION, EnvironmentVisualState, SkyPresentation};

/// Marker for the single LOW volumetric cloud render proxy (no physical cloud meaning).
#[derive(Component, Debug)]
pub struct EnvironmentProceduralCloud;

/// Deprecated marker retained for compile-time compatibility; HIGH is not rendered in CLOUD-VOL-1.
#[derive(Component, Debug)]
pub struct EnvironmentProceduralCloudLow;

/// Deprecated marker retained for compile-time compatibility; HIGH is not rendered in CLOUD-VOL-1.
#[derive(Component, Debug)]
pub struct EnvironmentProceduralCloudHigh;

/// Prevents duplicate cloud spawns across repeated startup hooks.
#[derive(Resource, Debug, Default)]
pub struct ProceduralCloudSpawnState {
    pub spawned: bool,
}

/// Render-coverage proxy radius only; no atmospheric altitude meaning (CLOUD-VOL-1).
pub const CLOUD_RENDER_PROXY_RADIUS: f32 = 5_000.0;

/// Spawn one camera-centered cloud render proxy when procedural sky is active.
pub fn setup_procedural_clouds(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<EnvironmentCloudMaterial>>,
    settings: Res<CloudSettings>,
    visual: Res<EnvironmentVisualState>,
    time: Res<Time>,
    mut spawn_state: ResMut<ProceduralCloudSpawnState>,
) {
    if spawn_state.spawned
        || !settings.enabled
        || DEFAULT_SKY_PRESENTATION != SkyPresentation::Procedural
    {
        return;
    }

    let elapsed = time.elapsed_secs();
    let proxy_mesh = meshes.add(Sphere::new(CLOUD_RENDER_PROXY_RADIUS));
    let material = materials.add(super::cloud_material::build_cloud_material(
        &visual,
        &settings,
        cloud_wind_displacement_world(&settings.low, elapsed),
    ));

    commands.spawn((
        EnvironmentProceduralCloud,
        Mesh3d(proxy_mesh),
        MeshMaterial3d(material),
        Transform::IDENTITY,
        Visibility::Visible,
        NotShadowCaster,
    ));
    spawn_state.spawned = true;
}

/// Follow the RTS camera and sync LOW volumetric uniforms from shared environment state.
pub fn sync_procedural_cloud_presentation(
    settings: Res<CloudSettings>,
    visual: Res<EnvironmentVisualState>,
    time: Res<Time>,
    camera: Query<&GlobalTransform, With<RtsCamera>>,
    mut proxy: Query<
        (&mut Transform, &MeshMaterial3d<EnvironmentCloudMaterial>),
        With<EnvironmentProceduralCloud>,
    >,
    mut materials: ResMut<Assets<EnvironmentCloudMaterial>>,
) {
    if !settings.enabled || DEFAULT_SKY_PRESENTATION != SkyPresentation::Procedural {
        return;
    }

    let Ok(camera_transform) = camera.single() else {
        return;
    };

    let elapsed = time.elapsed_secs();
    let camera_position = camera_transform.translation();

    for (mut transform, material_handle) in &mut proxy {
        transform.translation = camera_position;
        transform.rotation = Quat::IDENTITY;
        transform.scale = Vec3::ONE;
        if let Some(material) = materials.get_mut(&material_handle.0) {
            material.params = build_cloud_layer_uniform(
                &visual,
                &settings.low,
                &settings,
                cloud_wind_displacement_world(&settings.low, elapsed),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::RtsCamera;
    use crate::environment::{
        SkyColorPalette, TimeOfDaySettings, evaluate_environment_visual_state,
    };
    use bevy::app::App;
    use bevy::asset::AssetPlugin;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::mesh::{Mesh, MeshPlugin, VertexAttributeValues};
    use bevy::pbr::MaterialPlugin;

    use super::super::cloud_material::EnvironmentCloudMaterial;

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            MeshPlugin,
            MaterialPlugin::<EnvironmentCloudMaterial>::default(),
        ));
        app.init_resource::<ProceduralCloudSpawnState>();
        app.init_resource::<CloudSettings>();
        app.init_resource::<Time>();
        app.insert_resource(evaluate_environment_visual_state(
            &TimeOfDaySettings::default(),
            &SkyColorPalette::default(),
        ));
        app
    }

    fn mesh_max_radius(mesh: &Mesh) -> f32 {
        let positions = match mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap() {
            VertexAttributeValues::Float32x3(values) => values,
            _ => panic!("expected float positions"),
        };
        positions
            .iter()
            .map(|p| Vec3::from_array(*p).length())
            .fold(0.0_f32, f32::max)
    }

    #[test]
    fn procedural_mode_spawns_one_cloud_render_proxy() {
        let mut app = test_app();
        app.world_mut()
            .run_system_once(setup_procedural_clouds)
            .unwrap();
        app.world_mut()
            .run_system_once(setup_procedural_clouds)
            .unwrap();

        let mut world = app.world_mut();
        let proxies = world
            .query_filtered::<(), With<EnvironmentProceduralCloud>>()
            .iter(&mut world)
            .count();
        let legacy_low = world
            .query_filtered::<(), With<EnvironmentProceduralCloudLow>>()
            .iter(&mut world)
            .count();
        let legacy_high = world
            .query_filtered::<(), With<EnvironmentProceduralCloudHigh>>()
            .iter(&mut world)
            .count();
        assert_eq!(proxies, 1);
        assert_eq!(legacy_low, 0);
        assert_eq!(legacy_high, 0);
    }

    #[test]
    fn procedural_sky_mode_remains_default() {
        assert_eq!(DEFAULT_SKY_PRESENTATION, SkyPresentation::Procedural);
    }

    #[test]
    fn proxy_radius_is_render_coverage_not_cloud_altitude() {
        assert!((CLOUD_RENDER_PROXY_RADIUS - 5_000.0).abs() < f32::EPSILON);
        assert_ne!(CLOUD_RENDER_PROXY_RADIUS, 1800.0);
        assert_ne!(CLOUD_RENDER_PROXY_RADIUS, 3200.0);

        let mut app = test_app();
        app.world_mut()
            .run_system_once(setup_procedural_clouds)
            .unwrap();
        let mesh_handle = {
            let mut world = app.world_mut();
            world
                .query_filtered::<&Mesh3d, With<EnvironmentProceduralCloud>>()
                .single(&mut world)
                .expect("cloud proxy mesh")
                .0
                .clone()
        };
        let mesh = app
            .world()
            .resource::<Assets<Mesh>>()
            .get(&mesh_handle)
            .expect("proxy mesh asset");
        let radius = mesh_max_radius(mesh);
        assert!((radius - CLOUD_RENDER_PROXY_RADIUS).abs() < 1.0);
    }

    #[test]
    fn sync_follows_camera_xyz_for_render_proxy_only() {
        let mut app = test_app();
        app.world_mut()
            .run_system_once(setup_procedural_clouds)
            .unwrap();
        app.world_mut().spawn((
            RtsCamera,
            GlobalTransform::from_translation(Vec3::new(10.0, 900.0, 20.0)),
        ));
        app.world_mut()
            .run_system_once(sync_procedural_cloud_presentation)
            .unwrap();

        let mut world = app.world_mut();
        let transform = world
            .query_filtered::<&Transform, With<EnvironmentProceduralCloud>>()
            .single(&mut world)
            .unwrap();
        assert_eq!(transform.translation, Vec3::new(10.0, 900.0, 20.0));
    }

    #[test]
    fn cloud_one_f_low_defaults_remain_authored() {
        let settings = CloudSettings::default();
        assert!((settings.low.macro_scale - 0.0005).abs() < f32::EPSILON);
        assert!((settings.low.wind_speed - 3.5).abs() < f32::EPSILON);
        assert!((settings.low.coverage - 0.58).abs() < f32::EPSILON);
        assert!((settings.low_band.y_min - 1800.0).abs() < f32::EPSILON);
        assert!((settings.low_band.y_max - 3200.0).abs() < f32::EPSILON);
    }
}
