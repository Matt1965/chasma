//! Water plane spawn and presentation sync (ADR-053 E11).

use bevy::mesh::VertexAttributeValues;
use bevy::prelude::*;

use crate::world::{ChunkCoord, ChunkExtent, ChunkLayout, WorldConfig, WorldData};

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

impl WaterPlaneLayout {
    /// Inclusive world-space XZ bounds of the horizontal rectangle.
    pub fn world_bounds(self) -> WaterWorldBounds {
        let half_w = self.width * 0.5;
        let half_d = self.depth * 0.5;
        WaterWorldBounds {
            min_x: self.center.x - half_w,
            max_x: self.center.x + half_w,
            min_z: self.center.z - half_d,
            max_z: self.center.z + half_d,
        }
    }
}

/// World-space XZ bounds for the water rectangle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaterWorldBounds {
    pub min_x: f32,
    pub max_x: f32,
    pub min_z: f32,
    pub max_z: f32,
}

/// Authored terrain extent expressed in world meters (testable).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AuthoredTerrainMeters {
    pub min_chunk: ChunkCoord,
    pub max_chunk: ChunkCoord,
    pub chunk_size: f32,
    pub min_x: f32,
    pub max_x: f32,
    pub min_z: f32,
    pub max_z: f32,
    pub center_x: f32,
    pub center_z: f32,
    pub width: f32,
    pub depth: f32,
}

impl AuthoredTerrainMeters {
    pub fn from_extent(extent: ChunkExtent, layout: ChunkLayout) -> Self {
        let chunk_size = layout.chunk_size_units();
        let min_x = extent.min.x as f32 * chunk_size;
        let min_z = extent.min.z as f32 * chunk_size;
        let width = (extent.max.x - extent.min.x + 1) as f32 * chunk_size;
        let depth = (extent.max.z - extent.min.z + 1) as f32 * chunk_size;
        Self {
            min_chunk: extent.min,
            max_chunk: extent.max,
            chunk_size,
            min_x,
            max_x: min_x + width,
            min_z,
            max_z: min_z + depth,
            center_x: min_x + width * 0.5,
            center_z: min_z + depth * 0.5,
            width,
            depth,
        }
    }
}

/// Tracks the spawned water entity and cached layout for resize detection.
#[derive(Resource, Debug, Default)]
pub struct WaterSpawnState {
    pub entity: Option<Entity>,
    pub material: Option<Handle<StandardMaterial>>,
    pub mesh: Option<Handle<Mesh>>,
    pub cached_width: f32,
    pub cached_depth: f32,
    /// Whether the current singleton was built from authored extent (vs fallback).
    pub built_from_authored_extent: bool,
    pub logged_configuration: bool,
    pub logged_runtime_diagnostic: bool,
}

impl WaterSpawnState {
    pub fn water_entity_count(&self) -> usize {
        usize::from(self.entity.is_some())
    }

    fn clear_spawned(&mut self) {
        self.entity = None;
        self.mesh = None;
        self.material = None;
        self.cached_width = 0.0;
        self.cached_depth = 0.0;
        self.built_from_authored_extent = false;
        self.logged_configuration = false;
        self.logged_runtime_diagnostic = false;
    }
}

/// Derive plane center and size from authored extent (plus padding) or fallback settings.
pub fn water_plane_layout(
    settings: &WaterSettings,
    extent: Option<ChunkExtent>,
    layout: ChunkLayout,
) -> WaterPlaneLayout {
    if let Some(extent) = extent {
        layout_from_extent(settings, extent, layout)
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
    settings: &WaterSettings,
    extent: ChunkExtent,
    layout: ChunkLayout,
) -> WaterPlaneLayout {
    let authored = AuthoredTerrainMeters::from_extent(extent, layout);
    let padding = finite_non_negative(settings.extent_padding_meters);
    WaterPlaneLayout {
        center: Vec3::new(authored.center_x, settings.water_level, authored.center_z),
        width: authored.width + padding * 2.0,
        depth: authored.depth + padding * 2.0,
    }
}

fn finite_non_negative(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

fn horizontal_water_transform(layout: WaterPlaneLayout) -> Transform {
    Transform {
        translation: layout.center,
        rotation: Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2),
        scale: Vec3::ONE,
    }
}

/// Full width/depth of a Bevy XY [`Rectangle`] mesh (before water yaw/pitch rotation).
pub fn rectangle_mesh_xy_size(mesh: &Mesh) -> Option<(f32, f32)> {
    let VertexAttributeValues::Float32x3(positions) = mesh.attribute(Mesh::ATTRIBUTE_POSITION)?
    else {
        return None;
    };
    let mut max_x = 0.0_f32;
    let mut max_y = 0.0_f32;
    for p in positions {
        max_x = max_x.max(p[0].abs());
        max_y = max_y.max(p[1].abs());
    }
    Some((max_x * 2.0, max_y * 2.0))
}

fn approx_dim(a: f32, b: f32) -> bool {
    (a - b).abs() <= 1e-3_f32.max(b.abs() * 1e-5)
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
        state.clear_spawned();
        return;
    }

    let extent = world.as_ref().and_then(|world| world.extent());
    let chunk_layout = config.chunk_layout();
    let layout = water_plane_layout(&settings, extent, chunk_layout);
    let want_authored = extent.is_some();

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

    let marked: Vec<Entity> = planes.iter().map(|(entity, _)| entity).collect();
    let mesh_matches = state
        .mesh
        .as_ref()
        .and_then(|handle| meshes.get(handle))
        .and_then(rectangle_mesh_xy_size)
        .is_some_and(|(width, depth)| {
            approx_dim(width, layout.width) && approx_dim(depth, layout.depth)
        });

    let needs_spawn = state.entity.is_none()
        || marked.len() != 1
        || state.entity != marked.first().copied()
        || state.cached_width != layout.width
        || state.cached_depth != layout.depth
        || state.built_from_authored_extent != want_authored
        || !mesh_matches;

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

        let rebuilt_from_fallback = state.built_from_authored_extent != want_authored
            && want_authored
            && state.cached_width > 0.0;

        state.entity = Some(entity);
        state.mesh = Some(mesh);
        state.material = Some(material);
        state.cached_width = layout.width;
        state.cached_depth = layout.depth;
        state.built_from_authored_extent = want_authored;

        // Re-log when first configuring, or when replacing fallback with authored.
        if !state.logged_configuration || rebuilt_from_fallback {
            log_water_configuration(&settings, extent, chunk_layout, &layout, 1);
            state.logged_configuration = true;
        }
        // Re-run authored diagnostic after replacing fallback or resizing.
        if want_authored {
            state.logged_runtime_diagnostic = false;
        }
    } else if let Some(entity) = state.entity {
        if let Ok((_, mut visibility)) = planes.get_mut(entity) {
            *visibility = Visibility::Visible;
        }
    }
}

/// One-shot runtime geometry dump once authored extent and water entity both exist.
pub fn log_runtime_water_diagnostic_once(
    settings: Res<WaterSettings>,
    world: Option<Res<WorldData>>,
    config: Res<WorldConfig>,
    mut state: ResMut<WaterSpawnState>,
    meshes: Res<Assets<Mesh>>,
    materials: Res<Assets<StandardMaterial>>,
    planes: Query<
        (
            Entity,
            &Transform,
            &Mesh3d,
            &MeshMaterial3d<StandardMaterial>,
        ),
        With<EnvironmentWaterPlane>,
    >,
    all_meshes: Query<(
        Entity,
        &MeshMaterial3d<StandardMaterial>,
        Option<&EnvironmentWaterPlane>,
    )>,
) {
    if !settings.enabled || state.logged_runtime_diagnostic {
        return;
    }
    let Some(world) = world.as_ref() else {
        return;
    };
    let Some(extent) = world.extent() else {
        return;
    };
    if !state.built_from_authored_extent || state.entity.is_none() {
        return;
    }

    let chunk_layout = config.chunk_layout();
    let layout = water_plane_layout(&settings, Some(extent), chunk_layout);
    let authored = AuthoredTerrainMeters::from_extent(extent, chunk_layout);
    let padding = finite_non_negative(settings.extent_padding_meters);
    let expected_bounds = WaterWorldBounds {
        min_x: authored.min_x - padding,
        max_x: authored.max_x + padding,
        min_z: authored.min_z - padding,
        max_z: authored.max_z + padding,
    };
    let actual_bounds = layout.world_bounds();

    let marked_count = planes.iter().count();
    let Some(entity) = state.entity else {
        return;
    };
    // Wait until command flush has applied the singleton spawn.
    let Ok((_, transform, mesh3d, _)) = planes.get(entity) else {
        return;
    };

    let transform_dump = format!(
        "t=({:.1},{:.1},{:.1}) rot=({:.4},{:.4},{:.4},{:.4}) scale=({:.3},{:.3},{:.3})",
        transform.translation.x,
        transform.translation.y,
        transform.translation.z,
        transform.rotation.x,
        transform.rotation.y,
        transform.rotation.z,
        transform.rotation.w,
        transform.scale.x,
        transform.scale.y,
        transform.scale.z,
    );

    let mut mesh_dims = String::from("(missing)");
    let mut mesh_matches_layout = false;
    let mut mesh_matches_cache = false;
    if let Some(mesh) = meshes.get(&mesh3d.0) {
        if let Some((w, d)) = rectangle_mesh_xy_size(mesh) {
            mesh_dims = format!("{w:.1}x{d:.1}");
            mesh_matches_layout = approx_dim(w, layout.width) && approx_dim(d, layout.depth);
            mesh_matches_cache =
                approx_dim(w, state.cached_width) && approx_dim(d, state.cached_depth);
        }
    }

    let water_material = state.material.clone();
    let mut unmarked_same_material = 0usize;
    if let Some(water_mat) = water_material {
        for (_entity, mesh_mat, marker) in &all_meshes {
            if marker.is_some() {
                continue;
            }
            if mesh_mat.0 == water_mat {
                unmarked_same_material += 1;
            } else if let (Some(expected), Some(other)) =
                (materials.get(&water_mat), materials.get(&mesh_mat.0))
            {
                if expected.base_color == other.base_color
                    && expected.alpha_mode == other.alpha_mode
                    && approx_dim(expected.perceptual_roughness, other.perceptual_roughness)
                {
                    unmarked_same_material += 1;
                }
            }
        }
    }

    let covers = actual_bounds.min_x <= expected_bounds.min_x + 1e-2
        && actual_bounds.max_x >= expected_bounds.max_x - 1e-2
        && actual_bounds.min_z <= expected_bounds.min_z + 1e-2
        && actual_bounds.max_z >= expected_bounds.max_z - 1e-2;

    let center_ok = approx_dim(layout.center.x, authored.center_x)
        && approx_dim(layout.center.z, authored.center_z)
        && approx_dim(layout.center.y, settings.water_level);

    bevy::log::info!(
        target: "chasma::environment::water",
        "WATER DIAGNOSTIC entities_marked={marked_count} state_entity={entity:?} \
         built_from_authored={authored_built} transform={transform_dump} \
         mesh_xy={mesh_dims} layout={layout_w:.1}x{layout_d:.1} \
         cache={cache_w:.1}x{cache_d:.1} mesh_matches_layout={mesh_matches_layout} \
         mesh_matches_cache={mesh_matches_cache} \
         authored_chunks=({min_cx},{min_cz})..({max_cx},{max_cz}) chunk_size={chunk_size:.1} \
         authored_world=[{amin_x:.1}..{amax_x:.1}]x[{amin_z:.1}..{amax_z:.1}] \
         center=({acx:.1},{acz:.1}) size={aw:.1}x{ad:.1} \
         padding={padding:.1} expected_water=[{emin_x:.1}..{emax_x:.1}]x[{emin_z:.1}..{emax_z:.1}] \
         actual_water=[{rmin_x:.1}..{rmax_x:.1}]x[{rmin_z:.1}..{rmax_z:.1}] \
         covers_expected={covers} center_ok={center_ok} \
         unmarked_similar_material_planes={unmarked_same_material} water_level={level:.1}",
        entity = entity,
        authored_built = state.built_from_authored_extent,
        layout_w = layout.width,
        layout_d = layout.depth,
        cache_w = state.cached_width,
        cache_d = state.cached_depth,
        min_cx = authored.min_chunk.x,
        min_cz = authored.min_chunk.z,
        max_cx = authored.max_chunk.x,
        max_cz = authored.max_chunk.z,
        chunk_size = authored.chunk_size,
        amin_x = authored.min_x,
        amax_x = authored.max_x,
        amin_z = authored.min_z,
        amax_z = authored.max_z,
        acx = authored.center_x,
        acz = authored.center_z,
        aw = authored.width,
        ad = authored.depth,
        emin_x = expected_bounds.min_x,
        emax_x = expected_bounds.max_x,
        emin_z = expected_bounds.min_z,
        emax_z = expected_bounds.max_z,
        rmin_x = actual_bounds.min_x,
        rmax_x = actual_bounds.max_x,
        rmin_z = actual_bounds.min_z,
        rmax_z = actual_bounds.max_z,
        level = settings.water_level,
    );

    state.logged_runtime_diagnostic = true;
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
    extent: Option<ChunkExtent>,
    chunk_layout: ChunkLayout,
    layout: &WaterPlaneLayout,
    entity_count: usize,
) {
    let padding = finite_non_negative(settings.extent_padding_meters);
    if let Some(extent) = extent {
        let authored = AuthoredTerrainMeters::from_extent(extent, chunk_layout);
        bevy::log::info!(
            target: "chasma::environment::water",
            "Water configured: terrain={:.0}x{:.0} m, padding={:.0} m/side, plane={:.0}x{:.0} m, level={:.1}, entities={}",
            authored.width,
            authored.depth,
            padding,
            layout.width,
            layout.depth,
            settings.water_level,
            entity_count,
        );
    } else {
        bevy::log::info!(
            target: "chasma::environment::water",
            "Water configured: terrain=(none), padding={:.0} m/side (unused), plane={:.0}x{:.0} m, level={:.1}, entities={}",
            padding,
            layout.width,
            layout.depth,
            settings.water_level,
            entity_count,
        );
    }
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
    use crate::world::{ChunkCoord, WorldConfig, WorldData};
    use bevy::app::App;
    use bevy::asset::AssetPlugin;
    use bevy::ecs::system::RunSystemOnce;

    fn chunk_layout_256() -> ChunkLayout {
        ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        }
    }

    fn water_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
        app.init_resource::<WaterSettings>();
        app.init_resource::<WaterSpawnState>();
        app.init_resource::<WorldConfig>();
        app.init_resource::<Assets<Mesh>>();
        app.init_resource::<Assets<StandardMaterial>>();
        app
    }

    #[test]
    fn plane_size_uses_padding_1024_on_two_chunks() {
        let settings = WaterSettings {
            extent_padding_meters: 1_024.0,
            ..Default::default()
        };
        let extent = ChunkExtent {
            min: ChunkCoord::new(0, 0),
            max: ChunkCoord::new(1, 1),
        };
        let plane = water_plane_layout(&settings, Some(extent), chunk_layout_256());
        assert_eq!(plane.width, 2_560.0);
        assert_eq!(plane.depth, 2_560.0);
        assert_eq!(plane.center.x, 256.0);
        assert_eq!(plane.center.z, 256.0);
    }

    #[test]
    fn padded_layout_preserves_authored_center() {
        let settings = WaterSettings {
            extent_padding_meters: 16_384.0,
            water_level: 56.0,
            ..Default::default()
        };
        let extent = ChunkExtent {
            min: ChunkCoord::new(0, 0),
            max: ChunkCoord::new(1, 1),
        };
        let authored = AuthoredTerrainMeters::from_extent(extent, chunk_layout_256());
        let plane = water_plane_layout(&settings, Some(extent), chunk_layout_256());
        assert_eq!(plane.center.x, authored.center_x);
        assert_eq!(plane.center.z, authored.center_z);
        assert_eq!(plane.center.y, 56.0);
        assert_eq!(plane.width, authored.width + 2.0 * 16_384.0);
        assert_eq!(plane.depth, authored.depth + 2.0 * 16_384.0);
    }

    #[test]
    fn world_bounds_fully_contain_authored_terrain() {
        let settings = WaterSettings {
            extent_padding_meters: 1_024.0,
            ..Default::default()
        };
        let extent = ChunkExtent {
            min: ChunkCoord::new(0, 0),
            max: ChunkCoord::new(1, 1),
        };
        let authored = AuthoredTerrainMeters::from_extent(extent, chunk_layout_256());
        let plane = water_plane_layout(&settings, Some(extent), chunk_layout_256());
        let bounds = plane.world_bounds();
        assert!(bounds.min_x <= authored.min_x);
        assert!(bounds.max_x >= authored.max_x);
        assert!(bounds.min_z <= authored.min_z);
        assert!(bounds.max_z >= authored.max_z);
        assert_eq!(bounds.min_x, authored.min_x - 1_024.0);
        assert_eq!(bounds.max_x, authored.max_x + 1_024.0);
        assert_eq!(bounds.min_z, authored.min_z - 1_024.0);
        assert_eq!(bounds.max_z, authored.max_z + 1_024.0);
    }

    #[test]
    fn non_zero_positive_extent_keeps_authored_center() {
        let settings = WaterSettings {
            extent_padding_meters: 512.0,
            ..Default::default()
        };
        let extent = ChunkExtent {
            min: ChunkCoord::new(2, 4),
            max: ChunkCoord::new(3, 5),
        };
        let plane = water_plane_layout(&settings, Some(extent), chunk_layout_256());
        assert_eq!(plane.width, 512.0 + 1_024.0);
        assert_eq!(plane.depth, 512.0 + 1_024.0);
        assert_eq!(plane.center.x, 768.0);
        assert_eq!(plane.center.z, 1_280.0);
    }

    #[test]
    fn negative_chunk_coordinates_pad_around_authored_center() {
        let settings = WaterSettings {
            extent_padding_meters: 100.0,
            ..Default::default()
        };
        let extent = ChunkExtent {
            min: ChunkCoord::new(-2, -1),
            max: ChunkCoord::new(-1, 0),
        };
        let plane = water_plane_layout(&settings, Some(extent), chunk_layout_256());
        assert_eq!(plane.width, 712.0);
        assert_eq!(plane.depth, 712.0);
        assert_eq!(plane.center.x, -256.0);
        assert_eq!(plane.center.z, 0.0);
        let authored = AuthoredTerrainMeters::from_extent(extent, chunk_layout_256());
        let bounds = plane.world_bounds();
        assert!(bounds.min_x <= authored.min_x);
        assert!(bounds.max_x >= authored.max_x);
    }

    #[test]
    fn rectangular_extent_pads_width_and_depth_independently() {
        let settings = WaterSettings {
            extent_padding_meters: 10.0,
            ..Default::default()
        };
        let extent = ChunkExtent {
            min: ChunkCoord::new(0, 0),
            max: ChunkCoord::new(3, 1),
        };
        let plane = water_plane_layout(&settings, Some(extent), chunk_layout_256());
        assert_eq!(plane.width, 1_024.0 + 20.0);
        assert_eq!(plane.depth, 512.0 + 20.0);
        assert_eq!(plane.center.x, 512.0);
        assert_eq!(plane.center.z, 256.0);
    }

    #[test]
    fn zero_padding_matches_authored_extent_exactly() {
        let settings = WaterSettings {
            extent_padding_meters: 0.0,
            ..Default::default()
        };
        let extent = ChunkExtent {
            min: ChunkCoord::new(0, 0),
            max: ChunkCoord::new(1, 1),
        };
        let plane = water_plane_layout(&settings, Some(extent), chunk_layout_256());
        assert_eq!(plane.width, 512.0);
        assert_eq!(plane.depth, 512.0);
        assert_eq!(plane.center.x, 256.0);
        assert_eq!(plane.center.z, 256.0);
    }

    #[test]
    fn negative_padding_is_clamped_to_zero() {
        let settings = WaterSettings {
            extent_padding_meters: -100.0,
            ..Default::default()
        };
        let extent = ChunkExtent {
            min: ChunkCoord::new(0, 0),
            max: ChunkCoord::new(0, 0),
        };
        let plane = water_plane_layout(&settings, Some(extent), chunk_layout_256());
        assert_eq!(plane.width, 256.0);
        assert_eq!(plane.depth, 256.0);
    }

    #[test]
    fn non_finite_padding_is_treated_as_zero() {
        let settings = WaterSettings {
            extent_padding_meters: f32::NAN,
            ..Default::default()
        };
        let extent = ChunkExtent {
            min: ChunkCoord::new(0, 0),
            max: ChunkCoord::new(0, 0),
        };
        let plane = water_plane_layout(&settings, Some(extent), chunk_layout_256());
        assert_eq!(plane.width, 256.0);
        assert_eq!(plane.depth, 256.0);
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
    fn default_fallback_plane_size_is_65536() {
        let settings = WaterSettings::default();
        let layout = water_plane_layout(&settings, None, WorldConfig::default().chunk_layout());
        assert_eq!(layout.width, 65_536.0);
        assert_eq!(layout.depth, 65_536.0);
    }

    #[test]
    fn water_transform_remains_horizontal_at_water_level() {
        let settings = WaterSettings {
            water_level: 42.0,
            ..Default::default()
        };
        let layout = water_plane_layout(&settings, None, WorldConfig::default().chunk_layout());
        let transform = horizontal_water_transform(layout);
        assert!((transform.translation.y - 42.0).abs() < f32::EPSILON);
        assert_eq!(transform.scale, Vec3::ONE);
        let expected = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
        assert!((transform.rotation.dot(expected)).abs() > 0.999);
    }

    #[test]
    fn rectangle_mesh_dimensions_match_layout() {
        let mesh: Mesh = Rectangle::new(2_560.0, 1_280.0).into();
        let (w, d) = rectangle_mesh_xy_size(&mesh).expect("mesh size");
        assert!(approx_dim(w, 2_560.0));
        assert!(approx_dim(d, 1_280.0));
    }

    #[test]
    fn spawned_mesh_matches_water_plane_layout() {
        let mut app = water_app();
        let chunk_layout = app.world().resource::<WorldConfig>().chunk_layout();
        let mut world_data = WorldData::new(chunk_layout);
        world_data.set_authored_extent(ChunkExtent {
            min: ChunkCoord::new(0, 0),
            max: ChunkCoord::new(1, 1),
        });
        app.insert_resource(world_data);
        {
            let mut settings = app.world_mut().resource_mut::<WaterSettings>();
            settings.extent_padding_meters = 1_024.0;
        }

        app.world_mut()
            .run_system_once(ensure_environment_water)
            .unwrap();

        let state = app.world().resource::<WaterSpawnState>();
        let mesh_handle = state.mesh.clone().expect("mesh");
        let layout = WaterPlaneLayout {
            center: Vec3::new(256.0, 56.0, 256.0),
            width: 2_560.0,
            depth: 2_560.0,
        };
        assert_eq!(state.cached_width, layout.width);
        assert_eq!(state.cached_depth, layout.depth);
        let mesh = app
            .world()
            .resource::<Assets<Mesh>>()
            .get(&mesh_handle)
            .expect("mesh asset");
        let (w, d) = rectangle_mesh_xy_size(mesh).expect("size");
        assert!(approx_dim(w, layout.width));
        assert!(approx_dim(d, layout.depth));
    }

    #[test]
    fn fallback_to_authored_extent_replaces_mesh() {
        let mut app = water_app();
        {
            let mut settings = app.world_mut().resource_mut::<WaterSettings>();
            settings.plane_size_meters = 2_048.0;
            settings.extent_padding_meters = 1_024.0;
        }

        app.world_mut()
            .run_system_once(ensure_environment_water)
            .unwrap();
        assert!(
            !app.world()
                .resource::<WaterSpawnState>()
                .built_from_authored_extent
        );
        assert_eq!(
            app.world().resource::<WaterSpawnState>().cached_width,
            2_048.0
        );
        let fallback_entity = app.world().resource::<WaterSpawnState>().entity;

        let chunk_layout = app.world().resource::<WorldConfig>().chunk_layout();
        let mut world_data = WorldData::new(chunk_layout);
        world_data.set_authored_extent(ChunkExtent {
            min: ChunkCoord::new(0, 0),
            max: ChunkCoord::new(1, 1),
        });
        app.insert_resource(world_data);

        app.world_mut()
            .run_system_once(ensure_environment_water)
            .unwrap();

        let state = app.world().resource::<WaterSpawnState>();
        assert!(state.built_from_authored_extent);
        assert_eq!(state.cached_width, 2_560.0);
        assert_eq!(state.cached_depth, 2_560.0);
        assert_ne!(state.entity, fallback_entity);

        let mesh_handle = state.mesh.clone().expect("mesh");
        let mesh = app
            .world()
            .resource::<Assets<Mesh>>()
            .get(&mesh_handle)
            .expect("mesh asset");
        let (w, d) = rectangle_mesh_xy_size(mesh).expect("size");
        assert!(approx_dim(w, 2_560.0));
        assert!(approx_dim(d, 2_560.0));

        let mut world = app.world_mut();
        let count = world
            .query::<&EnvironmentWaterPlane>()
            .iter(&mut world)
            .count();
        assert_eq!(count, 1);
    }

    #[test]
    fn repeated_unchanged_frames_do_not_recreate_entity() {
        let mut app = water_app();
        let chunk_layout = app.world().resource::<WorldConfig>().chunk_layout();
        let mut world_data = WorldData::new(chunk_layout);
        world_data.set_authored_extent(ChunkExtent {
            min: ChunkCoord::new(0, 0),
            max: ChunkCoord::new(1, 1),
        });
        app.insert_resource(world_data);

        app.world_mut()
            .run_system_once(ensure_environment_water)
            .unwrap();
        let first = app.world().resource::<WaterSpawnState>().entity;
        app.world_mut()
            .run_system_once(ensure_environment_water)
            .unwrap();
        app.world_mut()
            .run_system_once(ensure_environment_water)
            .unwrap();
        let second = app.world().resource::<WaterSpawnState>().entity;
        assert_eq!(first, second);

        let mut world = app.world_mut();
        let count = world
            .query::<&EnvironmentWaterPlane>()
            .iter(&mut world)
            .count();
        assert_eq!(count, 1);
    }

    #[test]
    fn disabled_water_does_not_leave_spawned_entity_in_state_after_ensure() {
        let mut app = water_app();
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
        let mut app = water_app();
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
    fn changing_padding_resizes_cached_water_mesh_dimensions() {
        let mut app = water_app();
        let chunk_layout = app.world().resource::<WorldConfig>().chunk_layout();
        let mut world_data = WorldData::new(chunk_layout);
        world_data.set_authored_extent(ChunkExtent {
            min: ChunkCoord::new(0, 0),
            max: ChunkCoord::new(1, 1),
        });
        app.insert_resource(world_data);

        {
            let mut settings = app.world_mut().resource_mut::<WaterSettings>();
            settings.extent_padding_meters = 0.0;
        }

        app.world_mut()
            .run_system_once(ensure_environment_water)
            .unwrap();
        assert_eq!(
            app.world().resource::<WaterSpawnState>().cached_width,
            512.0
        );

        {
            let mut settings = app.world_mut().resource_mut::<WaterSettings>();
            settings.extent_padding_meters = 1_024.0;
        }

        app.world_mut()
            .run_system_once(ensure_environment_water)
            .unwrap();
        assert_eq!(
            app.world().resource::<WaterSpawnState>().cached_width,
            2_560.0
        );
        assert_eq!(
            app.world().resource::<WaterSpawnState>().cached_depth,
            2_560.0
        );

        let mut world = app.world_mut();
        let count = world
            .query::<&EnvironmentWaterPlane>()
            .iter(&mut world)
            .count();
        assert_eq!(count, 1);
    }

    #[test]
    fn settings_change_updates_water_transform() {
        let mut app = water_app();
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
