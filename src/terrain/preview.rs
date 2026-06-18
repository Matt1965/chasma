//! Developer preview scene for the terrain runtime (dev-only).
//!
//! Gated behind the `dev` feature. Wires catalog init, synchronous streaming,
//! lighting, and terrain render assets. The permanent RTS camera comes from
//! [`crate::camera::CameraPlugin`] (ADR-014).
//!
//! **Streaming vs LOD (Phase 2C):** [`TerrainStreamingSettings`] controls how far
//! terrain is loaded and kept resident (`load_radius_chunks` /
//! `unload_radius_chunks`). [`super::lod::TerrainLodSettings`] only picks mesh
//! resolution among chunks that are already resident — it does not load more
//! chunks or extend visible distance.

use std::path::Path;

use bevy::prelude::*;

use crate::world::{WorldConfig, WorldData};

use super::catalog::TerrainWorldCatalog;
use super::decode::decode_chunk;
use super::lod::TerrainLodSettings;
use super::spawn::{vertical_scale_for_height_span, TerrainRenderAssets};
use super::streaming::TerrainStreamingSettings;
use super::perf::TerrainStreamingPerfSettings;

/// Default path for dev preview terrain perf logs (see [`TerrainStreamingPerfSettings`]).
pub const PREVIEW_PERF_LOG_PATH: &str = "logs/terrain_streaming_perf.log";

/// Fallback when chunk metadata cannot be sampled at preview startup.
const DEV_PREVIEW_VERTICAL_SCALE_FALLBACK: f32 = 3.0;

/// Dev preview relief target: visible relief without over-exaggerating stitch artifacts.
const PREVIEW_TARGET_HEIGHT_SPAN_UNITS: f32 = 3.0;

/// Chebyshev radius (chunks) within which the preview **requests** new loads.
///
/// This is the streaming **existence** radius — terrain beyond this is not loaded.
/// The sample world is 32×32 chunks (~8 km); radius 14 loads most of the map from
/// a central focus. Must be `<=` [`PREVIEW_UNLOAD_RADIUS_CHUNKS`].
const PREVIEW_LOAD_RADIUS_CHUNKS: i32 = 14;

/// Chebyshev radius (chunks) within which resident preview chunks are **kept**.
///
/// Outer retention ring; must be `>` [`PREVIEW_LOAD_RADIUS_CHUNKS`]. Radius 16
/// covers the full authored extent from a roughly central focus.
const PREVIEW_UNLOAD_RADIUS_CHUNKS: i32 = 16;

/// LOD detail rings (Chebyshev distance from focus). Only affect resident chunks.
///
/// Near focus stays sharp; mid-distance uses Quarter; outer resident band → Eighth.
const PREVIEW_LOD_FULL_MAX_DISTANCE: i32 = 0;
const PREVIEW_LOD_HALF_MAX_DISTANCE: i32 = 1;
const PREVIEW_LOD_QUARTER_MAX_DISTANCE: i32 = 4;

/// Dev-only throughput knobs — higher than runtime defaults for faster map fill-in.
/// Mesh work stays async; raise these if pop-in feels slow, lower if frames hitch.
const PREVIEW_MAX_LOADS_PER_FRAME: usize = 32;
const PREVIEW_MAX_UNLOADS_PER_FRAME: usize = 24;
const PREVIEW_MAX_APPLY_PER_FRAME: usize = 32;
const PREVIEW_MAX_DECODE_STARTS_PER_FRAME: usize = 32;
const PREVIEW_MAX_MESH_STARTS_PER_FRAME: usize = 32;
const PREVIEW_MAX_MESH_STORES_PER_FRAME: usize = 32;
const PREVIEW_MAX_LOD_BUILDS_PER_FRAME: usize = 24;
const PREVIEW_MAX_LOD_PREFETCH_PER_FRAME: usize = 12;
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
        // Vertex colors multiply with base_color; white passes albedo through unchanged.
        base_color: Color::WHITE,
        unlit: true,
        ..default()
    });

    let vertical_scale = preview_vertical_scale(&catalog);

    commands.insert_resource(catalog);
    commands.insert_resource(TerrainStreamingSettings {
        // Streaming radius = how far terrain exists (ADR-012). LOD does not extend this.
        load_radius_chunks: PREVIEW_LOAD_RADIUS_CHUNKS,
        unload_radius_chunks: PREVIEW_UNLOAD_RADIUS_CHUNKS,
        max_loads_per_frame: PREVIEW_MAX_LOADS_PER_FRAME,
        max_unloads_per_frame: PREVIEW_MAX_UNLOADS_PER_FRAME,
        max_apply_per_frame: PREVIEW_MAX_APPLY_PER_FRAME,
        max_decode_starts_per_frame: PREVIEW_MAX_DECODE_STARTS_PER_FRAME,
        max_mesh_starts_per_frame: PREVIEW_MAX_MESH_STARTS_PER_FRAME,
        max_mesh_stores_per_frame: PREVIEW_MAX_MESH_STORES_PER_FRAME,
    });
    commands.insert_resource(TerrainLodSettings {
        // LOD radius = mesh resolution among already-resident chunks (ADR-013).
        full_max_distance: PREVIEW_LOD_FULL_MAX_DISTANCE,
        half_max_distance: PREVIEW_LOD_HALF_MAX_DISTANCE,
        quarter_max_distance: PREVIEW_LOD_QUARTER_MAX_DISTANCE,
        max_lod_builds_per_frame: PREVIEW_MAX_LOD_BUILDS_PER_FRAME,
        max_lod_prefetch_per_frame: PREVIEW_MAX_LOD_PREFETCH_PER_FRAME,
    });
    commands.insert_resource(TerrainRenderAssets {
        material,
        vertical_scale,
    });
    commands.insert_resource(TerrainStreamingPerfSettings {
        enabled: true,
        log_to_console: false,
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
