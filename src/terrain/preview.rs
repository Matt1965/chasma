//! Developer preview scene for the terrain runtime (dev-only).
//!
//! Gated behind the `dev` feature. This is composition/throwaway code: it wires
//! a camera, a light, the manifest load path, and derived render entities so the
//! Phase 2A vertical slice can be viewed end to end. It must not be depended on
//! by the core layers (ADR-007, ADR-010).

use std::path::Path;

use bevy::prelude::*;

use crate::world::{WorldConfig, WorldData};

use super::load::load_world_from_manifest;
use super::spawn::spawn_terrain_render_entities;

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
    spawn_terrain_render_entities(&mut commands, &world, size, &mut meshes, material);

    commands.spawn((
        DirectionalLight {
            illuminance: 12_000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(256.0, 200.0, 128.0)
            .looking_at(Vec3::new(256.0, 0.0, 128.0), Vec3::Y),
    ));

    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(256.0, 180.0, 420.0).looking_at(Vec3::new(256.0, 8.0, 128.0), Vec3::Y),
    ));
}
