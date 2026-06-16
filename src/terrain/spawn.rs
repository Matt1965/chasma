//! Spawn and despawn derived terrain render entities (ADR-010, ADR-012).
//!
//! Runtime streaming applies prebuilt meshes via [`spawn_prebuilt_chunk_mesh_inner`].
//! [`despawn_chunk_meshes`] is used on unload. Sync main-thread mesh rebuild
//! entry points were removed; mesh generation runs on the async materialization path.

#[cfg(feature = "dev")]
use std::time::Instant;

use bevy::prelude::*;

use crate::world::{ChunkCoord, ChunkId, WorldData};

use super::components::TerrainChunkMesh;
use super::lod_cache::TerrainChunkLodCache;
use super::mesh::{ChunkLod, ChunkMeshSeamWeld};
#[cfg(feature = "dev")]
use super::perf::TerrainStreamingPerfRecorder;

/// Shared render resources for terrain chunk meshes.
#[derive(Debug, Clone, Resource)]
pub struct TerrainRenderAssets {
    pub material: Handle<StandardMaterial>,
    pub vertical_scale: f32,
}

/// Target visible height span (world units) when auto-scaling subtle source heights.
pub const DEFAULT_TARGET_HEIGHT_SPAN_UNITS: f32 = 120.0;

/// Compute a mesh vertical scale from authored height range in meters/units.
pub fn vertical_scale_for_height_span(
    height_min: f32,
    height_max: f32,
    target_span_units: f32,
) -> f32 {
    let span = (height_max - height_min).max(1e-12);
    target_span_units / span
}

pub(crate) fn seam_weld_heights(world: &WorldData, chunk_id: ChunkId) -> ChunkMeshSeamWeld {
    let coord = chunk_id.coord();
    let edge = |data: &crate::world::ChunkData| data.heightfield.samples_per_edge() - 1;
    let penultimate = |data: &crate::world::ChunkData| {
        data.heightfield.samples_per_edge().saturating_sub(2)
    };

    let west = world
        .get(ChunkId::new(ChunkCoord::new(coord.x - 1, coord.z)))
        .map(|data| data.heightfield.column_heights(edge(data)));
    let south = world
        .get(ChunkId::new(ChunkCoord::new(coord.x, coord.z - 1)))
        .map(|data| data.heightfield.row_heights(edge(data)));
    let east_interior = world
        .get(ChunkId::new(ChunkCoord::new(coord.x + 1, coord.z)))
        .map(|data| data.heightfield.column_heights(1));
    let north_interior = world
        .get(ChunkId::new(ChunkCoord::new(coord.x, coord.z + 1)))
        .map(|data| data.heightfield.row_heights(1));
    let west_interior = world
        .get(ChunkId::new(ChunkCoord::new(coord.x - 1, coord.z)))
        .map(|data| data.heightfield.column_heights(penultimate(data)));
    let south_interior = world
        .get(ChunkId::new(ChunkCoord::new(coord.x, coord.z - 1)))
        .map(|data| data.heightfield.row_heights(penultimate(data)));

    ChunkMeshSeamWeld {
        west_edge: west,
        south_edge: south,
        east_interior,
        north_interior,
        west_interior,
        south_interior,
    }
}

pub(crate) fn spawn_prebuilt_chunk_mesh_inner(
    commands: &mut Commands,
    chunk_id: ChunkId,
    chunk_size_units: f32,
    meshes: &mut Assets<Mesh>,
    material: Handle<StandardMaterial>,
    mesh: Mesh,
    active_lod: ChunkLod,
    #[cfg(feature = "dev")] mut perf: Option<&mut TerrainStreamingPerfRecorder>,
) {
    #[cfg(feature = "dev")]
    if let Some(perf) = perf.as_mut() {
        perf.record_prebuilt_mesh_applied();
    }

    #[cfg(feature = "dev")]
    let assets_start = perf.is_some().then(Instant::now);
    let mesh_handle = meshes.add(mesh);
    #[cfg(feature = "dev")]
    if let (Some(perf), Some(start)) = (perf.as_mut(), assets_start) {
        perf.record_mesh_assets(start.elapsed());
    }

    let mut cache = TerrainChunkLodCache::default();
    cache.set(active_lod, mesh_handle.clone());

    let coord = chunk_id.coord();
    #[cfg(feature = "dev")]
    let spawn_start = perf.is_some().then(Instant::now);
    commands.spawn((
        Mesh3d(mesh_handle),
        MeshMaterial3d(material),
        Transform::from_xyz(
            coord.x as f32 * chunk_size_units,
            0.0,
            coord.z as f32 * chunk_size_units,
        ),
        TerrainChunkMesh::new(chunk_id, active_lod),
        cache,
    ));
    #[cfg(feature = "dev")]
    if let (Some(perf), Some(start)) = (perf.as_mut(), spawn_start) {
        perf.record_spawn(start.elapsed());
    }
}

/// Despawn all derived render entities for `chunk_id`.
pub fn despawn_chunk_meshes(
    commands: &mut Commands,
    chunk_id: ChunkId,
    mesh_entities: &Query<(Entity, &TerrainChunkMesh)>,
) {
    for (entity, marker) in mesh_entities {
        if marker.chunk == chunk_id {
            commands.entity(entity).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{ChunkCoord, ChunkId};

    #[test]
    fn vertical_scale_inversely_tracks_height_span() {
        let tight = vertical_scale_for_height_span(0.0, 0.0001, 100.0);
        let wide = vertical_scale_for_height_span(0.0, 10.0, 100.0);
        assert!(tight > wide);
    }

    #[test]
    fn despawn_target_is_matching_chunk_id() {
        let keep = ChunkId::new(ChunkCoord::new(0, 0));
        let remove = ChunkId::new(ChunkCoord::new(1, 0));
        assert_ne!(keep, remove);
    }
}
