//! Developer preview scene for the terrain runtime (dev-only).
//!
//! Gated behind the `dev` feature. Wires catalog init, synchronous streaming,
//! lighting, and terrain render assets. The permanent RTS camera comes from
//! [`crate::camera::CameraPlugin`] (ADR-014).

use std::path::Path;

use bevy::prelude::*;

use crate::world::{WorldConfig, WorldData};

use super::catalog::TerrainWorldCatalog;
use super::decode::decode_chunk;
use super::spawn::{vertical_scale_for_height_span, TerrainRenderAssets};
use super::streaming::TerrainStreamingSettings;
use super::perf::TerrainStreamingPerfSettings;

/// Default path for dev preview terrain perf logs (see [`TerrainStreamingPerfSettings`]).
pub const PREVIEW_PERF_LOG_PATH: &str = "logs/terrain_streaming_perf.log";

/// Fallback when chunk metadata cannot be sampled at preview startup.
const DEV_PREVIEW_VERTICAL_SCALE_FALLBACK: f32 = 5.0;

/// Dev preview relief target: visible relief without over-exaggerating stitch artifacts.
const PREVIEW_TARGET_HEIGHT_SPAN_UNITS: f32 = 5.0;

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

    let vertical_scale = preview_vertical_scale(&catalog);

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
        vertical_scale,
    });
    commands.insert_resource(TerrainStreamingPerfSettings {
        enabled: true,
        log_to_console: true,
        log_to_file: true,
        log_file_path: PREVIEW_PERF_LOG_PATH.to_string(),
        ..Default::default()
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

fn preview_vertical_scale(catalog: &TerrainWorldCatalog) -> f32 {
    let coord = catalog.authored_extent().min;
    let Some(path) = catalog.chunk_path(coord) else {
        return DEV_PREVIEW_VERTICAL_SCALE_FALLBACK;
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return DEV_PREVIEW_VERTICAL_SCALE_FALLBACK;
    };
    let Ok((_, data)) = decode_chunk(&text) else {
        return DEV_PREVIEW_VERTICAL_SCALE_FALLBACK;
    };
    vertical_scale_for_height_span(
        data.metadata.height_min,
        data.metadata.height_max,
        PREVIEW_TARGET_HEIGHT_SPAN_UNITS,
    )
}
