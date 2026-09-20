use std::collections::HashSet;

use bevy::prelude::*;

use crate::terrain::albedo::production_albedo_fallback;
use crate::terrain::components::TerrainChunkMesh;
use crate::terrain::lod_build::PendingChunkLodBuilds;
use crate::terrain::lod_cache::TerrainChunkLodCache;
use crate::terrain::mesh::{ChunkLod, build_chunk_mesh_scaled_with_delta};
use crate::terrain::spawn::{TerrainRenderAssets, seam_weld_heights_effective};
use crate::terrain::TerrainChunkAlbedo;
use crate::world::{ChunkId, WorldData};

/// Chunks queued for terrain mesh rebuild after road deformation changes.
#[derive(Resource, Debug, Default)]
pub struct RoadTerrainRebuildQueue {
    pub chunks: HashSet<ChunkId>,
}

pub fn queue_road_terrain_rebuilds(queue: &mut RoadTerrainRebuildQueue, chunk_ids: &HashSet<ChunkId>) {
    queue.chunks.extend(chunk_ids.iter().copied());
}

pub fn apply_road_terrain_rebuilds(
    mut queue: ResMut<RoadTerrainRebuildQueue>,
    world: Res<WorldData>,
    render_assets: Res<TerrainRenderAssets>,
    chunk_albedo: Res<TerrainChunkAlbedo>,
    mut pending: ResMut<PendingChunkLodBuilds>,
    mut meshes: Query<(
        &TerrainChunkMesh,
        &mut Mesh3d,
        &mut TerrainChunkLodCache,
    )>,
    mut assets: ResMut<Assets<Mesh>>,
) {
    if queue.chunks.is_empty() {
        return;
    }
    let chunk_ids = queue.chunks.clone();
    queue.chunks.clear();

    let vertical_scale = render_assets.vertical_scale;
    let fallback = production_albedo_fallback();

    for chunk_id in chunk_ids {
        let Some(chunk) = world.get(chunk_id) else {
            continue;
        };
        let seam_weld = seam_weld_heights_effective(&world, chunk_id);
        let albedo = chunk_albedo.get(chunk_id).cloned();
        let delta = chunk.road_height_delta.as_ref();

        for (marker, mut mesh3d, mut cache) in &mut meshes {
            if marker.chunk != chunk_id {
                continue;
            }
            *cache = TerrainChunkLodCache::default();
            let lod = marker.active_lod;
            let mesh = build_chunk_mesh_scaled_with_delta(
                &chunk.heightfield,
                delta,
                lod,
                vertical_scale,
                &seam_weld,
                albedo.as_ref(),
                fallback,
            );
            let handle = assets.add(mesh);
            mesh3d.0 = handle.clone();
            cache.set(lod, handle);

            for other_lod in [
                ChunkLod::Full,
                ChunkLod::Half,
                ChunkLod::Quarter,
                ChunkLod::Eighth,
            ] {
                if other_lod == lod {
                    continue;
                }
                if pending.has_in_flight(chunk_id, other_lod) {
                    continue;
                }
                pending.try_enqueue_immediate(
                    chunk_id,
                    other_lod,
                    chunk.clone(),
                    albedo.clone(),
                    vertical_scale,
                    seam_weld.clone(),
                    fallback,
                );
            }
        }
    }
}
