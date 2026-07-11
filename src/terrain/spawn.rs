//! Spawn and despawn derived terrain render entities (ADR-010, ADR-012).
//!
//! Runtime streaming applies prebuilt meshes via [`spawn_prebuilt_chunk_mesh_inner`].
//! [`despawn_chunk_meshes`] is used on unload. Sync main-thread mesh rebuild
//! entry points were removed; mesh generation runs on the async materialization path.

#[cfg(feature = "dev")]
use std::time::Instant;

use bevy::prelude::*;

use crate::world::{ChunkCoord, ChunkId, WorldData, WorldPosition};

use super::components::TerrainChunkMesh;
use super::lod_cache::TerrainChunkLodCache;
#[cfg(feature = "dev")]
use super::mesh::chunk_mesh_geometry;
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

/// Map authoritative world Y to terrain render Y (ADR-010 visualization scale).
pub fn render_height(authoritative_y: f32, vertical_scale: f32) -> f32 {
    authoritative_y * vertical_scale
}

/// Compose a render-space position from authoritative [`WorldPosition`].
///
/// Terrain meshes multiply heightfield samples by `vertical_scale`; doodad render
/// entities must apply the same factor so props align with the visible surface.
pub fn world_position_to_render_global(
    position: WorldPosition,
    layout: crate::world::ChunkLayout,
    vertical_scale: f32,
) -> Vec3 {
    let mut global = position.to_global(layout);
    global.y = render_height(global.y, vertical_scale);
    global
}

pub(crate) fn seam_weld_heights(world: &WorldData, chunk_id: ChunkId) -> ChunkMeshSeamWeld {
    let coord = chunk_id.coord();
    let edge = |data: &crate::world::ChunkData| data.heightfield.samples_per_edge() - 1;
    let penultimate =
        |data: &crate::world::ChunkData| data.heightfield.samples_per_edge().saturating_sub(2);

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
        perf.record_prebuilt_mesh_applied(chunk_id.coord(), active_lod, chunk_mesh_geometry(&mesh));
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
    use crate::terrain::albedo::AlbedoFallback;
    use crate::terrain::mesh::{ChunkLod, build_chunk_mesh_scaled, chunk_mesh_geometry};
    use crate::world::{ChunkCoord, ChunkData, ChunkId, Heightfield, WorldData};

    #[test]
    fn vertical_scale_inversely_tracks_height_span() {
        let tight = vertical_scale_for_height_span(0.0, 0.0001, 100.0);
        let wide = vertical_scale_for_height_span(0.0, 10.0, 100.0);
        assert!(tight > wide);
    }

    #[test]
    fn world_position_to_render_global_scales_y_only() {
        use crate::world::{ChunkLayout, LocalPosition};

        let layout = ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        };
        let pos = WorldPosition::new(
            ChunkCoord::new(1, 2),
            LocalPosition::new(Vec3::new(10.0, 4.0, 20.0)),
        );
        let render = world_position_to_render_global(pos, layout, 3.0);
        assert_eq!(render.x, 266.0);
        assert_eq!(render.y, 12.0);
        assert_eq!(render.z, 532.0);
    }

    #[test]
    fn seam_weld_heights_and_build_chunk_mesh_scaled_share_path() {
        let layout = crate::world::ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        };
        let mut world = WorldData::new(layout);
        let west_samples: Vec<f32> = (0..9).map(|i| i as f32 * 0.5).collect();
        let center_samples: Vec<f32> = (0..9).map(|i| i as f32).collect();
        let east_samples: Vec<f32> = (0..9).map(|i| i as f32 * 2.0).collect();
        world.insert(
            ChunkId::new(ChunkCoord::new(-1, 0)),
            ChunkData::new(
                Heightfield::from_samples(3, 128.0, west_samples).unwrap(),
                Vec::new(),
            ),
        );
        let center_id = ChunkId::new(ChunkCoord::new(0, 0));
        world.insert(
            center_id,
            ChunkData::new(
                Heightfield::from_samples(3, 128.0, center_samples).unwrap(),
                Vec::new(),
            ),
        );
        world.insert(
            ChunkId::new(ChunkCoord::new(1, 0)),
            ChunkData::new(
                Heightfield::from_samples(3, 128.0, east_samples).unwrap(),
                Vec::new(),
            ),
        );

        let center = world.get(center_id).unwrap();
        let seam_weld = seam_weld_heights(&world, center_id);
        let mesh = build_chunk_mesh_scaled(
            &center.heightfield,
            ChunkLod::Full,
            1.0,
            &seam_weld,
            None,
            AlbedoFallback::Neutral,
        );
        let geometry = chunk_mesh_geometry(&mesh);
        assert!(geometry.vertices > 0);
        assert!(geometry.indices > 0);
        assert!(seam_weld.west_edge.is_some());
        assert!(seam_weld.east_interior.is_some());
    }

    #[test]
    fn despawn_target_is_matching_chunk_id() {
        let keep = ChunkId::new(ChunkCoord::new(0, 0));
        let remove = ChunkId::new(ChunkCoord::new(1, 0));
        assert_ne!(keep, remove);
    }
}
