//! Developer preview scene for the terrain runtime (dev-only).
//!
//! Gated behind the `dev` feature. Wires catalog init, synchronous streaming,
//! lighting, and terrain render assets. The permanent RTS camera comes from
//! [`crate::camera::CameraPlugin`] (ADR-014).

use std::path::Path;

use bevy::prelude::*;

use crate::world::{WorldConfig, WorldData};

use super::catalog::TerrainWorldCatalog;
use super::spawn::TerrainRenderAssets;
use super::streaming::TerrainStreamingSettings;

/// Multiplier applied to mesh Y only in the dev preview.
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
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let catalog = TerrainWorldCatalog::from_manifest(Path::new(PREVIEW_MANIFEST_PATH), &config)
        .unwrap_or_else(|err| {
            panic!(
                "dev preview failed to load catalog from {PREVIEW_MANIFEST_PATH}: {err}. \
                 Run from the project root so the assets path resolves."
            );
        });
    assert!(catalog.chunk_count() > 0, "dev preview manifest listed no chunks");

    world.set_authored_extent(catalog.authored_extent());

    let material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.35, 0.55, 0.30),
        perceptual_roughness: 0.95,
        ..default()
    });

    commands.insert_resource(catalog);
    commands.insert_resource(TerrainStreamingSettings {
        load_radius_chunks: 1,
        unload_radius_chunks: 2,
        max_loads_per_frame: 4,
        max_unloads_per_frame: 4,
        max_apply_per_frame: 2,
        max_decode_per_frame: 4,
    });
    commands.insert_resource(TerrainRenderAssets {
        material,
        vertical_scale: DEV_PREVIEW_VERTICAL_SCALE,
    });

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
