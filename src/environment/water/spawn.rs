//! Water plane spawn and presentation sync (ADR-053 E11).

use bevy::prelude::*;

use crate::world::{ChunkExtent, ChunkLayout, WorldConfig, WorldData};

use super::material::build_water_material;
use super::settings::WaterSettings;

/// Marker for the environment-owned water surface (at most one in E11).
#[derive(Component, Debug)]
pub struct EnvironmentWaterPlane;

/// Computed placement for the water plane (testable, no ECS).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaterPlaneLayout {
    pub center: Vec3,
    pub width: f32,
    pub depth: f32,
}

/// Tracks the spawned water entity and cached layout for resize detection.
#[derive(Resource, Debug, Default)]
pub struct WaterSpawnState {
    pub entity: Option<Entity>,
    pub material: Option<Handle<StandardMaterial>>,
    pub mesh: Option<Handle<Mesh>>,
    pub cached_width: f32,
    pub cached_depth: f32,
    pub logged_configuration: bool,
}

impl WaterSpawnState {
    pub fn water_entity_count(&self) -> usize {
        usize::from(self.entity.is_some())
    }
}

/// Derive plane center and size from authored extent or fallback settings.
pub fn water_plane_layout(
    settings: &WaterSettings,
    extent: Option<ChunkExtent>,
    layout: ChunkLayout,
) -> WaterPlaneLayout {
    if let Some(extent) = extent {
        layout_from_extent(settings.water_level, extent, layout)
    } else {
        let size = settings.plane_size_meters.max(1.0);
        WaterPlaneLayout {
            center: Vec3::new(size * 0.5, settings.water_level, size * 0.5),
            width: size,
            depth: size,
        }
    }
}

fn layout_from_extent(
    water_level: f32,
    extent: ChunkExtent,
    layout: ChunkLayout,
) -> WaterPlaneLayout {
    let chunk_size = layout.chunk_size_units();
    let origin_x = extent.min.x as f32 * chunk_size;
    let origin_z = extent.min.z as f32 * chunk_size;
    let width = (extent.max.x - extent.min.x + 1) as f32 * chunk_size;
    let depth = (extent.max.z - extent.min.z + 1) as f32 * chunk_size;
    WaterPlaneLayout {
        center: Vec3::new(origin_x + width * 0.5, water_level, origin_z + depth * 0.5),
        width,
        depth,
    }
}

fn horizontal_water_transform(layout: WaterPlaneLayout) -> Transform {
    Transform {
        translation: layout.center,
        rotation: Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2),
        scale: Vec3::ONE,
    }
}

/// Spawn, hide, or despawn the singleton water plane based on [`WaterSettings::enabled`].
pub fn ensure_environment_water(
    mut commands: Commands,
    settings: Res<WaterSettings>,
    world: Option<Res<WorldData>>,
    config: Res<WorldConfig>,
    mut state: ResMut<WaterSpawnState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut planes: Query<(Entity, &mut Visibility), With<EnvironmentWaterPlane>>,
) {
    if !settings.enabled {
        for (entity, _) in &planes {
            commands.entity(entity).despawn();
        }
        state.entity = None;
        state.mesh = None;
        state.material = None;
        state.cached_width = 0.0;
        state.cached_depth = 0.0;
        state.logged_configuration = false;
        return;
    }

    let extent = world.as_ref().and_then(|world| world.extent());
    let layout = water_plane_layout(&settings, extent, config.chunk_layout());

    if extent.is_none() {
        #[cfg(feature = "dev")]
        if !state.logged_configuration {
            bevy::log::warn!(
                target: "chasma::environment::water",
                "Authored world extent not set; using fallback plane size {:.0} m",
                settings.plane_size_meters
            );
        }
    }

    let needs_spawn = state.entity.is_none()
        || state.cached_width != layout.width
        || state.cached_depth != layout.depth;

    if needs_spawn {
        for (entity, _) in &planes {
            commands.entity(entity).despawn();
        }

        let mesh = meshes.add(Rectangle::new(layout.width, layout.depth));
        let material = materials.add(build_water_material(&settings));
        let entity = commands
            .spawn((
                EnvironmentWaterPlane,
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material.clone()),
                horizontal_water_transform(layout),
                Visibility::Visible,
            ))
            .id();

        state.entity = Some(entity);
        state.mesh = Some(mesh);
        state.material = Some(material);
        state.cached_width = layout.width;
        state.cached_depth = layout.depth;

        if !state.logged_configuration {
            log_water_configuration(&settings, &layout, 1);
            state.logged_configuration = true;
        }
    } else if let Some(entity) = state.entity {
        if let Ok((_, mut visibility)) = planes.get_mut(entity) {
            *visibility = Visibility::Visible;
        }
    }
}

/// Keep water transform and material aligned with settings / extent changes.
pub fn sync_environment_water_presentation(
    settings: Res<WaterSettings>,
    world: Option<Res<WorldData>>,
    config: Res<WorldConfig>,
    state: Res<WaterSpawnState>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut planes: Query<
        (
            &mut Transform,
            &mut MeshMaterial3d<StandardMaterial>,
            &mut Visibility,
        ),
        With<EnvironmentWaterPlane>,
    >,
) {
    if !settings.enabled {
        return;
    }

    let Some(entity) = state.entity else {
        return;
    };

    let extent = world.as_ref().and_then(|world| world.extent());
    let layout = water_plane_layout(&settings, extent, config.chunk_layout());

    let Ok((mut transform, mesh_material, mut visibility)) = planes.get_mut(entity) else {
        return;
    };

    *transform = horizontal_water_transform(layout);
    *visibility = Visibility::Visible;

    if let Some(material) = materials.get_mut(&mesh_material.0) {
        *material = build_water_material(&settings);
    }
}

fn log_water_configuration(
    settings: &WaterSettings,
    layout: &WaterPlaneLayout,
    entity_count: usize,
) {
    bevy::log::info!(
        target: "chasma::environment::water",
        "Water configured: enabled={}, level={:.1}, plane={:.0}x{:.0} m, entities={}",
        settings.enabled,
        settings.water_level,
        layout.width,
        layout.depth,
        entity_count,
    );
}

#[cfg(feature = "dev")]
pub fn water_dev_keyboard(
    keyboard: Res<ButtonInput<KeyCode>>,
    dev_state: Res<crate::dev::DevModeState>,
    mut settings: ResMut<WaterSettings>,
) {
    if !dev_state.enabled {
        return;
    }

    let shift = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);

    if shift && keyboard.just_pressed(KeyCode::KeyW) {
        settings.enabled = !settings.enabled;
        bevy::log::info!(
            target: "chasma::environment::water",
            "Water {}",
            if settings.enabled { "enabled" } else { "disabled" }
        );
    }
    if shift && keyboard.just_pressed(KeyCode::PageUp) {
        settings.water_level += 1.0;
    }
    if shift && keyboard.just_pressed(KeyCode::PageDown) {
        settings.water_level -= 1.0;
    }
    if shift && keyboard.just_pressed(KeyCode::Equal) {
        settings.alpha = (settings.alpha + 0.05).min(1.0);
    }
    if shift && keyboard.just_pressed(KeyCode::Minus) {
        settings.alpha = (settings.alpha - 0.05).max(0.05);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{ChunkCoord, WorldConfig};

    #[test]
    fn plane_size_derives_from_authored_extent_when_available() {
        let settings = WaterSettings::default();
        let layout = ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        };
        let extent = ChunkExtent {
            min: ChunkCoord::new(0, 0),
            max: ChunkCoord::new(1, 1),
        };
        let plane = water_plane_layout(&settings, Some(extent), layout);
        assert_eq!(plane.width, 512.0);
        assert_eq!(plane.depth, 512.0);
        assert_eq!(plane.center.x, 256.0);
        assert_eq!(plane.center.z, 256.0);
    }

    #[test]
    fn fallback_size_used_when_no_extent() {
        let settings = WaterSettings {
            plane_size_meters: 1024.0,
            water_level: 7.5,
            ..Default::default()
        };
        let layout = water_plane_layout(&settings, None, WorldConfig::default().chunk_layout());
        assert_eq!(layout.width, 1024.0);
        assert_eq!(layout.depth, 1024.0);
        assert_eq!(layout.center.y, 7.5);
    }

    #[test]
    fn water_transform_y_equals_water_level() {
        let settings = WaterSettings {
            water_level: 42.0,
            ..Default::default()
        };
        let layout = water_plane_layout(&settings, None, WorldConfig::default().chunk_layout());
        let transform = horizontal_water_transform(layout);
        assert!((transform.translation.y - 42.0).abs() < f32::EPSILON);
    }

    #[test]
    fn disabled_water_does_not_leave_spawned_entity_in_state_after_ensure() {
        use bevy::app::App;
        use bevy::asset::AssetPlugin;
        use bevy::ecs::system::RunSystemOnce;

        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
        app.init_resource::<WaterSettings>();
        app.init_resource::<WaterSpawnState>();
        app.init_resource::<WorldConfig>();
        app.init_resource::<Assets<Mesh>>();
        app.init_resource::<Assets<StandardMaterial>>();

        {
            let mut settings = app.world_mut().resource_mut::<WaterSettings>();
            settings.enabled = true;
        }

        app.world_mut()
            .run_system_once(ensure_environment_water)
            .unwrap();
        assert_eq!(
            app.world()
                .resource::<WaterSpawnState>()
                .water_entity_count(),
            1
        );

        {
            let mut settings = app.world_mut().resource_mut::<WaterSettings>();
            settings.enabled = false;
        }

        app.world_mut()
            .run_system_once(ensure_environment_water)
            .unwrap();
        assert_eq!(
            app.world()
                .resource::<WaterSpawnState>()
                .water_entity_count(),
            0
        );

        let mut world = app.world_mut();
        let count = world
            .query::<&EnvironmentWaterPlane>()
            .iter(&mut world)
            .count();
        assert_eq!(count, 0);
    }

    #[test]
    fn water_spawn_does_not_duplicate() {
        use bevy::app::App;
        use bevy::asset::AssetPlugin;
        use bevy::ecs::system::RunSystemOnce;

        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
        app.init_resource::<WaterSettings>();
        app.init_resource::<WaterSpawnState>();
        app.init_resource::<WorldConfig>();
        app.init_resource::<Assets<Mesh>>();
        app.init_resource::<Assets<StandardMaterial>>();

        app.world_mut()
            .run_system_once(ensure_environment_water)
            .unwrap();
        app.world_mut()
            .run_system_once(ensure_environment_water)
            .unwrap();

        let mut world = app.world_mut();
        let count = world
            .query::<&EnvironmentWaterPlane>()
            .iter(&mut world)
            .count();
        assert_eq!(count, 1);
    }

    #[test]
    fn settings_change_updates_water_transform() {
        use bevy::app::App;
        use bevy::asset::AssetPlugin;
        use bevy::ecs::system::RunSystemOnce;

        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
        app.init_resource::<WaterSettings>();
        app.init_resource::<WaterSpawnState>();
        app.init_resource::<WorldConfig>();
        app.init_resource::<Assets<Mesh>>();
        app.init_resource::<Assets<StandardMaterial>>();

        app.world_mut()
            .run_system_once(ensure_environment_water)
            .unwrap();

        {
            let mut settings = app.world_mut().resource_mut::<WaterSettings>();
            settings.water_level = 99.0;
        }

        app.world_mut()
            .run_system_once(sync_environment_water_presentation)
            .unwrap();

        let entity = app.world().resource::<WaterSpawnState>().entity.unwrap();
        let transform = app
            .world()
            .get::<Transform>(entity)
            .expect("water transform");
        assert!((transform.translation.y - 99.0).abs() < f32::EPSILON);
    }
}
