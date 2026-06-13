//! Synchronous terrain chunk streaming lifecycle (ADR-012).

use bevy::prelude::*;

use crate::view::PrimaryViewFocus;
use crate::world::{WorldConfig, WorldData};

use super::components::TerrainChunkMesh;
use super::catalog::TerrainWorldCatalog;
use super::load::load_chunk_from_path;
use super::spawn::{TerrainRenderAssets, despawn_chunk_meshes, spawn_chunk_mesh};
use super::streaming::{TerrainStreamingSettings, diff_streaming_residency};

/// Systems that drive synchronous terrain chunk residency.
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct TerrainStreamingSystems;

/// Apply synchronous load/unload for the current view focus.
pub fn stream_terrain_chunks(
    focus: Res<PrimaryViewFocus>,
    catalog: Res<TerrainWorldCatalog>,
    settings: Res<TerrainStreamingSettings>,
    config: Res<WorldConfig>,
    render_assets: Res<TerrainRenderAssets>,
    mut world: ResMut<WorldData>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mesh_entities: Query<(Entity, &TerrainChunkMesh)>,
) {
    let layout = config.chunk_layout();
    let chunk_size_units = layout.chunk_size_units();
    let (to_load, to_unload) =
        diff_streaming_residency(&focus, layout, &settings, &catalog, &world);

    for chunk_id in to_unload {
        despawn_chunk_meshes(&mut commands, chunk_id, &mesh_entities);
        world.remove(chunk_id);
    }

    for coord in to_load {
        let Some(entry) = catalog.get(coord) else {
            continue;
        };
        let Some(path) = catalog.chunk_path(coord) else {
            continue;
        };

        let chunk_id = match load_chunk_from_path(&path, entry, &config, &mut world) {
            Ok(id) => id,
            Err(err) => {
                bevy::log::error!("failed to load terrain chunk ({}, {}): {err}", coord.x, coord.z);
                continue;
            }
        };

        spawn_chunk_mesh(
            &mut commands,
            chunk_id,
            &world,
            chunk_size_units,
            &mut meshes,
            render_assets.material.clone(),
            render_assets.vertical_scale,
        );
    }
}
