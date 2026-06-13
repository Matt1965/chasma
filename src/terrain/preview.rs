//! Developer preview scene for the terrain runtime (dev-only).
//!
//! Gated behind the `dev` feature. This is composition/throwaway code: it wires
//! a light, the manifest load path, and derived render entities so the Phase 2A
//! vertical slice can be viewed end to end. The permanent RTS camera comes from
//! [`crate::camera::CameraPlugin`] (ADR-014). This plugin must not be depended
//! on by the core layers (ADR-007, ADR-010).

use std::path::Path;

use bevy::prelude::*;

use crate::world::{WorldConfig, WorldData};

use super::load::load_world_from_manifest;
use super::spawn::spawn_terrain_render_entities_scaled;

/// Multiplier applied to mesh Y only in the dev preview. Source Gaea tiles in
/// `source_data/test` carry ~0.27 m of relief over 512 m — invisible at RTS
/// camera distance without exaggeration or a taller Gaea export scale.
const DEV_PREVIEW_VERTICAL_SCALE: f32 = 250.0;

/// On-disk sample world exercised by the dev preview (ADR-011).
pub const PREVIEW_MANIFEST_PATH: &str = "assets/worlds/main/manifest.ron";

/// Adds a minimal viewable terrain scene. Dev-only composition (ADR-007).
pub struct TerrainPreviewPlugin;

impl Plugin for TerrainPreviewPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_preview);
    }
}

fn setup_preview(
    mut commands: Commands,
    config: Res<WorldConfig>,
    mut world: ResMut<WorldData>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let loaded = load_world_from_manifest(Path::new(PREVIEW_MANIFEST_PATH), &config, &mut world)
        .unwrap_or_else(|err| {
            panic!(
                "dev preview failed to load {PREVIEW_MANIFEST_PATH}: {err}. \
                 Run from the project root so the assets path resolves."
            );
        });
    assert!(loaded > 0, "dev preview manifest listed no chunks");

    let material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.35, 0.55, 0.30),
        perceptual_roughness: 0.95,
        ..default()
    });
    let size = config.chunk_layout().chunk_size_units();
    spawn_terrain_render_entities_scaled(
        &mut commands,
        &world,
        size,
        &mut meshes,
        material,
        DEV_PREVIEW_VERTICAL_SCALE,
    );

    commands.spawn((
        DirectionalLight {
            illuminance: 12_000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(256.0, 200.0, 128.0)
            .looking_at(Vec3::new(256.0, 0.0, 128.0), Vec3::Y),
    ));

}
