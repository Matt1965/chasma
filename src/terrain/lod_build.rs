//! Async resident LOD mesh builds and cache refresh (ADR-013 Phase 2C-c).
//!
//! Missing LOD meshes are built on [`AsyncComputeTaskPool`]. Cache hits swap
//! handles on the main thread without calling the mesh builder.

use std::time::Duration;

use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task};

use crate::view::PrimaryViewFocus;
use crate::world::{ChunkCoord, ChunkData, ChunkId, WorldConfig, WorldData};

use super::albedo::{
    AlbedoFallback, ChunkAlbedoGrid, TerrainChunkAlbedo, production_albedo_fallback,
};
use super::catalog::TerrainWorldCatalog;
use super::components::TerrainChunkMesh;
use super::lod::{LodPriority, TerrainLodSettings, desired_lod, predicted_lod_targets};
use super::lod_cache::TerrainChunkLodCache;
#[cfg(feature = "dev")]
use super::mesh::chunk_mesh_geometry;
use super::mesh::{ChunkLod, ChunkMeshSeamWeld, build_chunk_mesh_scaled};
use super::spawn::{TerrainRenderAssets, seam_weld_heights};
use super::streaming::{TerrainStreamingSettings, stable_focus_chunk};

/// Completed async LOD mesh build ready for main-thread cache registration.
#[derive(Debug)]
pub struct LodBuildOutput {
    pub chunk_id: ChunkId,
    pub lod: ChunkLod,
    pub mesh: Mesh,
    pub build_duration: Duration,
    pub from_prefetch: bool,
}

/// Async LOD mesh-build task (compute pool).
pub type ChunkLodBuildTask = Task<LodBuildOutput>;

struct InFlightLodBuild {
    chunk_id: ChunkId,
    lod: ChunkLod,
    from_prefetch: bool,
    task: ChunkLodBuildTask,
}

/// Per-frame predictive LOD warmup counters (dev perf).
#[derive(Debug, Default, Clone, Copy)]
pub struct LodPrefetchFrameStats {
    pub prefetch_requests: usize,
    pub prefetch_hits: usize,
    pub prefetch_misses: usize,
    pub builds_started_from_prefetch: usize,
    pub warmup_high: usize,
    pub warmup_mid: usize,
    pub warmup_low: usize,
}

/// Tracks in-flight async LOD mesh builds keyed by `(ChunkId, ChunkLod)`.
#[derive(Resource, Default)]
pub struct PendingChunkLodBuilds {
    in_flight: Vec<InFlightLodBuild>,
}

impl PendingChunkLodBuilds {
    pub fn in_flight_count(&self) -> usize {
        self.in_flight.len()
    }

    pub fn has_in_flight(&self, chunk_id: ChunkId, lod: ChunkLod) -> bool {
        self.in_flight
            .iter()
            .any(|entry| entry.chunk_id == chunk_id && entry.lod == lod)
    }

    /// Enqueue an immediate-priority async LOD build (display-driven cache miss).
    pub fn try_enqueue_immediate(
        &mut self,
        chunk_id: ChunkId,
        lod: ChunkLod,
        data: ChunkData,
        albedo: Option<ChunkAlbedoGrid>,
        vertical_scale: f32,
        seam_weld: ChunkMeshSeamWeld,
        fallback: AlbedoFallback,
    ) -> bool {
        self.try_enqueue_inner(
            chunk_id,
            lod,
            data,
            albedo,
            vertical_scale,
            seam_weld,
            fallback,
            false,
        )
    }

    /// Enqueue a predictive prefetch LOD build (lower scheduling priority than immediate).
    pub fn try_enqueue_prefetch(
        &mut self,
        chunk_id: ChunkId,
        lod: ChunkLod,
        data: ChunkData,
        albedo: Option<ChunkAlbedoGrid>,
        vertical_scale: f32,
        seam_weld: ChunkMeshSeamWeld,
        fallback: AlbedoFallback,
    ) -> bool {
        self.try_enqueue_inner(
            chunk_id,
            lod,
            data,
            albedo,
            vertical_scale,
            seam_weld,
            fallback,
            true,
        )
    }

    fn try_enqueue_inner(
        &mut self,
        chunk_id: ChunkId,
        lod: ChunkLod,
        data: ChunkData,
        albedo: Option<ChunkAlbedoGrid>,
        vertical_scale: f32,
        seam_weld: ChunkMeshSeamWeld,
        fallback: AlbedoFallback,
        from_prefetch: bool,
    ) -> bool {
        if self.has_in_flight(chunk_id, lod) {
            return false;
        }
        self.in_flight.push(InFlightLodBuild {
            chunk_id,
            lod,
            from_prefetch,
            task: spawn_chunk_lod_build_task(
                chunk_id,
                data,
                albedo,
                vertical_scale,
                lod,
                seam_weld,
                fallback,
            ),
        });
        true
    }

    /// Back-compat for tests: treated as immediate enqueue.
    pub fn try_enqueue(
        &mut self,
        chunk_id: ChunkId,
        lod: ChunkLod,
        data: ChunkData,
        vertical_scale: f32,
        seam_weld: ChunkMeshSeamWeld,
    ) -> bool {
        self.try_enqueue_immediate(
            chunk_id,
            lod,
            data,
            None,
            vertical_scale,
            seam_weld,
            production_albedo_fallback(),
        )
    }

    /// Drop all in-flight builds for `chunk_id` (chunk unload).
    pub fn cancel_for_chunk(&mut self, chunk_id: ChunkId) {
        self.in_flight.retain(|entry| entry.chunk_id != chunk_id);
    }

    /// Poll finished tasks and return completed outputs.
    pub fn poll(&mut self) -> Vec<LodBuildOutput> {
        let mut completed = Vec::new();
        self.in_flight.retain_mut(|entry| {
            if !entry.task.is_finished() {
                return true;
            }
            let mut output = bevy::tasks::block_on(&mut entry.task);
            output.from_prefetch = entry.from_prefetch;
            completed.push(output);
            false
        });
        completed
    }
}

/// Mesh-build stage for resident LOD cache misses.
pub fn spawn_chunk_lod_build_task(
    chunk_id: ChunkId,
    data: ChunkData,
    albedo: Option<ChunkAlbedoGrid>,
    vertical_scale: f32,
    lod: ChunkLod,
    seam_weld: ChunkMeshSeamWeld,
    fallback: AlbedoFallback,
) -> ChunkLodBuildTask {
    AsyncComputeTaskPool::get().spawn(async move {
        let start = std::time::Instant::now();
        let mesh = build_chunk_mesh_scaled(
            &data.heightfield,
            lod,
            vertical_scale,
            &seam_weld,
            albedo.as_ref(),
            fallback,
        );
        LodBuildOutput {
            chunk_id,
            lod,
            mesh,
            build_duration: start.elapsed(),
            from_prefetch: false,
        }
    })
}

/// Swap active mesh handles when the desired LOD is already cached.
pub fn apply_cached_lod_swaps(
    focus: Res<PrimaryViewFocus>,
    config: Res<WorldConfig>,
    lod_settings: Res<TerrainLodSettings>,
    query: Query<(&mut TerrainChunkMesh, &mut Mesh3d, &TerrainChunkLodCache)>,
) {
    let focus_coord = stable_focus_chunk(focus.position, config.chunk_layout());
    apply_cached_lod_swaps_inner(focus_coord, &lod_settings, query);
}

pub(crate) fn apply_cached_lod_swaps_inner(
    focus_coord: crate::world::ChunkCoord,
    lod_settings: &TerrainLodSettings,
    mut query: Query<(&mut TerrainChunkMesh, &mut Mesh3d, &TerrainChunkLodCache)>,
) {
    for (mut marker, mut mesh3d, cache) in &mut query {
        let desired = desired_lod(focus_coord, marker.chunk.coord(), lod_settings);
        if desired == marker.active_lod {
            continue;
        }
        let Some(handle) = cache.get(desired).cloned() else {
            continue;
        };
        mesh3d.0 = handle;
        marker.active_lod = desired;
    }
}

/// Enqueue async LOD builds for cache misses and predictive prefetch (mesh stays active).
pub fn request_missing_lod_builds(
    focus: Res<PrimaryViewFocus>,
    config: Res<WorldConfig>,
    catalog: Res<TerrainWorldCatalog>,
    streaming: Res<TerrainStreamingSettings>,
    lod_settings: Res<TerrainLodSettings>,
    render_assets: Res<TerrainRenderAssets>,
    chunk_albedo: Res<TerrainChunkAlbedo>,
    world: Res<WorldData>,
    mut pending_builds: ResMut<PendingChunkLodBuilds>,
    query: Query<(&TerrainChunkMesh, &TerrainChunkLodCache)>,
    #[cfg(feature = "dev")] perf_settings: Res<super::perf::TerrainStreamingPerfSettings>,
    #[cfg(feature = "dev")] mut perf_state: ResMut<super::perf::TerrainStreamingPerfState>,
) {
    let focus_coord = stable_focus_chunk(focus.position, config.chunk_layout());
    let stats = request_missing_lod_builds_inner(
        focus_coord,
        &catalog,
        streaming.load_radius_chunks,
        &lod_settings,
        render_assets.vertical_scale,
        &chunk_albedo,
        &world,
        &mut pending_builds,
        query,
    );
    #[cfg(not(feature = "dev"))]
    let _ = stats;

    #[cfg(feature = "dev")]
    if perf_settings.enabled {
        let frame = perf_state.frame_mut();
        frame.lod_prefetch_requests = stats.prefetch_requests;
        frame.lod_prefetch_hits = stats.prefetch_hits;
        frame.lod_prefetch_misses = stats.prefetch_misses;
        frame.lod_builds_started_from_prefetch = stats.builds_started_from_prefetch;
    }
}

pub(crate) fn request_missing_lod_builds_inner(
    focus_coord: crate::world::ChunkCoord,
    catalog: &TerrainWorldCatalog,
    load_radius_chunks: i32,
    lod_settings: &TerrainLodSettings,
    vertical_scale: f32,
    chunk_albedo: &TerrainChunkAlbedo,
    world: &WorldData,
    pending_builds: &mut PendingChunkLodBuilds,
    query: Query<(&TerrainChunkMesh, &TerrainChunkLodCache)>,
) -> LodPrefetchFrameStats {
    let mut stats = LodPrefetchFrameStats::default();
    let mut resident: std::collections::HashMap<ChunkId, ChunkCoord> =
        std::collections::HashMap::new();
    for (marker, _) in &query {
        resident.insert(marker.chunk, marker.chunk.coord());
    }

    let max_immediate = lod_settings.max_lod_builds_per_frame;
    let mut immediate_enqueued = 0usize;

    // Phase 1: immediate display-driven LOD (always wins over prefetch).
    for (marker, cache) in &query {
        let desired = desired_lod(focus_coord, marker.chunk.coord(), lod_settings);
        if desired == marker.active_lod || cache.has_lod(desired) {
            continue;
        }
        if pending_builds.has_in_flight(marker.chunk, desired) {
            continue;
        }
        if max_immediate != 0 && immediate_enqueued >= max_immediate {
            continue;
        }
        let Some(data) = world.get(marker.chunk).cloned() else {
            continue;
        };
        let seam_weld = seam_weld_heights(world, marker.chunk);
        let albedo = chunk_albedo.get(marker.chunk).cloned();
        if pending_builds.try_enqueue_immediate(
            marker.chunk,
            desired,
            data,
            albedo,
            vertical_scale,
            seam_weld,
            production_albedo_fallback(),
        ) {
            immediate_enqueued += 1;
        }
    }

    // Phase 2: predictive prefetch for resident chunks in load+1 / load+2 bands.
    let max_prefetch = lod_settings.max_lod_prefetch_per_frame;
    let mut prefetch_enqueued = 0usize;
    let targets = predicted_lod_targets(focus_coord, catalog, lod_settings, load_radius_chunks);

    for (coord, lod, priority) in targets {
        if max_prefetch != 0 && prefetch_enqueued >= max_prefetch {
            break;
        }
        let chunk_id = ChunkId::new(coord);
        if !resident.contains_key(&chunk_id) || !world.is_chunk_loaded(chunk_id) {
            continue;
        }
        let Some((marker, cache)) = query.iter().find(|(m, _)| m.chunk == chunk_id) else {
            continue;
        };

        // Skip if already displayed or cached at the prefetch target.
        if lod == marker.active_lod || cache.has_lod(lod) {
            if cache.has_lod(lod) {
                stats.prefetch_hits += 1;
            }
            continue;
        }
        if pending_builds.has_in_flight(chunk_id, lod) {
            continue;
        }

        stats.prefetch_misses += 1;
        let Some(data) = world.get(chunk_id).cloned() else {
            continue;
        };
        let seam_weld = seam_weld_heights(world, chunk_id);
        let albedo = chunk_albedo.get(chunk_id).cloned();
        if pending_builds.try_enqueue_prefetch(
            chunk_id,
            lod,
            data,
            albedo,
            vertical_scale,
            seam_weld,
            production_albedo_fallback(),
        ) {
            prefetch_enqueued += 1;
            stats.prefetch_requests += 1;
            stats.builds_started_from_prefetch += 1;
            match priority {
                LodPriority::High => stats.warmup_high += 1,
                LodPriority::Medium => stats.warmup_mid += 1,
                LodPriority::Low => stats.warmup_low += 1,
            }
        }
    }

    stats
}

/// Poll completed LOD builds; register meshes and swap when still desired.
pub fn poll_lod_builds(
    focus: Res<PrimaryViewFocus>,
    config: Res<WorldConfig>,
    lod_settings: Res<TerrainLodSettings>,
    world: Res<WorldData>,
    mut pending_builds: ResMut<PendingChunkLodBuilds>,
    mut meshes: ResMut<Assets<Mesh>>,
    query: Query<(
        Entity,
        &mut TerrainChunkMesh,
        &mut Mesh3d,
        &mut TerrainChunkLodCache,
    )>,
    #[cfg(feature = "dev")] perf_settings: Res<super::perf::TerrainStreamingPerfSettings>,
    #[cfg(feature = "dev")] mut perf_state: ResMut<super::perf::TerrainStreamingPerfState>,
) {
    let focus_coord = stable_focus_chunk(focus.position, config.chunk_layout());
    poll_lod_builds_inner(
        focus_coord,
        &lod_settings,
        &world,
        &mut pending_builds,
        &mut meshes,
        query,
        #[cfg(feature = "dev")]
        perf_settings.enabled.then_some(&mut perf_state),
    );
}

pub(crate) fn poll_lod_builds_inner(
    focus_coord: crate::world::ChunkCoord,
    lod_settings: &TerrainLodSettings,
    world: &WorldData,
    pending_builds: &mut PendingChunkLodBuilds,
    meshes: &mut Assets<Mesh>,
    mut query: Query<(
        Entity,
        &mut TerrainChunkMesh,
        &mut Mesh3d,
        &mut TerrainChunkLodCache,
    )>,
    #[cfg(feature = "dev")] mut perf_state: Option<&mut super::perf::TerrainStreamingPerfState>,
) {
    let completed = pending_builds.poll();

    #[cfg(feature = "dev")]
    if let Some(perf_state) = perf_state.as_mut() {
        let frame = perf_state.frame_mut();
        for output in &completed {
            super::perf::record_mesh_build_event(
                frame,
                output.chunk_id.coord(),
                output.lod,
                if output.from_prefetch {
                    super::perf::MeshBuildReason::LodPrefetch
                } else {
                    super::perf::MeshBuildReason::LodImmediate
                },
                super::perf::duration_to_ms(output.build_duration),
                chunk_mesh_geometry(&output.mesh),
            );
        }
    }

    for output in completed {
        if !world.is_chunk_loaded(output.chunk_id) {
            continue;
        }

        let Some(entity) = query
            .iter()
            .find_map(|(entity, marker, _, _)| (marker.chunk == output.chunk_id).then_some(entity))
        else {
            continue;
        };

        let Ok((_, mut marker, mut mesh3d, mut cache)) = query.get_mut(entity) else {
            continue;
        };

        let handle = meshes.add(output.mesh);
        cache.set(output.lod, handle.clone());

        let desired = desired_lod(focus_coord, marker.chunk.coord(), lod_settings);
        if desired == output.lod {
            mesh3d.0 = handle;
            marker.active_lod = output.lod;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::catalog::TerrainWorldCatalog;
    use crate::terrain::mesh::{ChunkLod, test_build_mesh_call_count, test_reset_build_mesh_calls};
    use crate::terrain::streaming::TerrainStreamingSettings;
    use crate::view::PrimaryViewFocus;
    use crate::world::{ChunkCoord, Heightfield};
    use bevy::tasks::TaskPoolBuilder;
    use std::sync::Once;

    static TASK_POOLS: Once = Once::new();

    fn ensure_task_pools() {
        TASK_POOLS.call_once(|| {
            AsyncComputeTaskPool::get_or_init(|| TaskPoolBuilder::new().num_threads(1).build());
        });
    }

    fn sample_chunk_data() -> ChunkData {
        ChunkData::new(
            Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap(),
            Vec::new(),
        )
    }

    fn dummy_mesh(meshes: &mut Assets<Mesh>) -> Handle<Mesh> {
        use bevy::asset::RenderAssetUsages;
        use bevy::mesh::PrimitiveTopology;
        meshes.add(Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        ))
    }

    fn setup_lod_world(chunk_id: ChunkId) -> World {
        let mut world = World::new();
        world.init_resource::<WorldConfig>();
        world.init_resource::<WorldData>();
        world.init_resource::<TerrainChunkAlbedo>();
        world.init_resource::<PrimaryViewFocus>();
        world.init_resource::<TerrainLodSettings>();
        world.init_resource::<PendingChunkLodBuilds>();
        world.init_resource::<Assets<Mesh>>();
        world.init_resource::<Assets<StandardMaterial>>();
        let material = {
            let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
            materials.add(StandardMaterial::default())
        };
        world.insert_resource(TerrainRenderAssets {
            material,
            vertical_scale: 1.0,
        });
        world.register_component::<TerrainChunkMesh>();
        world.register_component::<TerrainChunkLodCache>();
        world
            .resource_mut::<WorldData>()
            .insert(chunk_id, sample_chunk_data());
        world
    }

    fn spawn_chunk_entity(
        world: &mut World,
        chunk_id: ChunkId,
        active_lod: ChunkLod,
        extra_cached: &[ChunkLod],
    ) -> (Entity, Handle<Mesh>, Vec<Handle<Mesh>>) {
        let mut meshes = world.resource_mut::<Assets<Mesh>>();
        let active_handle = dummy_mesh(&mut meshes);
        let mut cache = TerrainChunkLodCache::default();
        cache.set(active_lod, active_handle.clone());
        let mut extra_handles = Vec::new();
        for lod in extra_cached {
            let handle = dummy_mesh(&mut meshes);
            cache.set(*lod, handle.clone());
            extra_handles.push(handle);
        }
        let entity = world
            .spawn((
                Mesh3d(active_handle.clone()),
                TerrainChunkMesh::new(chunk_id, active_lod),
                cache,
            ))
            .id();
        (entity, active_handle, extra_handles)
    }

    fn apply_cached_swaps_in_world(world: &mut World) {
        use bevy::ecs::system::SystemState;

        let mut state = SystemState::<(
            Res<PrimaryViewFocus>,
            Res<WorldConfig>,
            Res<TerrainLodSettings>,
            Query<(&mut TerrainChunkMesh, &mut Mesh3d, &TerrainChunkLodCache)>,
        )>::new(world);
        let (focus, config, lod_settings, mut query) = state.get_mut(world);
        apply_cached_lod_swaps_inner(
            crate::terrain::streaming::stable_focus_chunk(focus.position, config.chunk_layout()),
            &lod_settings,
            query.reborrow(),
        );
        state.apply(world);
    }

    fn poll_lod_builds_in_world(world: &mut World) {
        use bevy::ecs::system::SystemState;

        let mut state = SystemState::<(
            Res<PrimaryViewFocus>,
            Res<WorldConfig>,
            Res<TerrainLodSettings>,
            Res<WorldData>,
            ResMut<PendingChunkLodBuilds>,
            ResMut<Assets<Mesh>>,
            Query<(
                Entity,
                &mut TerrainChunkMesh,
                &mut Mesh3d,
                &mut TerrainChunkLodCache,
            )>,
        )>::new(world);
        let (focus, config, lod_settings, world_data, mut pending, mut meshes, mut query) =
            state.get_mut(world);
        poll_lod_builds_inner(
            crate::terrain::streaming::stable_focus_chunk(focus.position, config.chunk_layout()),
            &lod_settings,
            &world_data,
            &mut pending,
            &mut meshes,
            query.reborrow(),
            #[cfg(feature = "dev")]
            None,
        );
        state.apply(world);
    }

    fn request_missing_in_world_stats(world: &mut World) -> LodPrefetchFrameStats {
        use bevy::ecs::system::SystemState;

        let mut state = SystemState::<(
            Res<PrimaryViewFocus>,
            Res<WorldConfig>,
            Res<TerrainWorldCatalog>,
            Res<TerrainStreamingSettings>,
            Res<TerrainLodSettings>,
            Res<TerrainRenderAssets>,
            Res<TerrainChunkAlbedo>,
            Res<WorldData>,
            ResMut<PendingChunkLodBuilds>,
            Query<(&TerrainChunkMesh, &TerrainChunkLodCache)>,
        )>::new(world);
        let (
            focus,
            config,
            catalog,
            streaming,
            lod_settings,
            render_assets,
            chunk_albedo,
            world_data,
            mut pending,
            query,
        ) = state.get_mut(world);
        let stats = request_missing_lod_builds_inner(
            crate::terrain::streaming::stable_focus_chunk(focus.position, config.chunk_layout()),
            &catalog,
            streaming.load_radius_chunks,
            &lod_settings,
            render_assets.vertical_scale,
            &chunk_albedo,
            &world_data,
            &mut pending,
            query,
        );
        state.apply(world);
        stats
    }

    fn request_missing_in_world(world: &mut World) {
        request_missing_in_world_stats(world);
    }

    fn test_catalog(coords: &[(i32, i32)]) -> TerrainWorldCatalog {
        use crate::terrain::asset::{MANIFEST_FORMAT_VERSION, Manifest, ManifestChunk};
        use crate::terrain::load::config_snapshot;
        use std::sync::atomic::{AtomicU64, Ordering};

        static NEXT: AtomicU64 = AtomicU64::new(0);
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("chasma_lod_build_cat_{id}"));
        std::fs::create_dir_all(&dir).unwrap();
        let config = WorldConfig::default();
        let chunks: Vec<ManifestChunk> = coords
            .iter()
            .map(|(x, z)| ManifestChunk::at(*x, *z, format!("{x}_{z}.ron")))
            .collect();
        let manifest = Manifest {
            version: MANIFEST_FORMAT_VERSION,
            config: config_snapshot(&config),
            chunks,
        };
        std::fs::write(dir.join("manifest.ron"), ron::to_string(&manifest).unwrap()).unwrap();
        let catalog =
            TerrainWorldCatalog::from_manifest(&dir.join("manifest.ron"), &config).unwrap();
        std::fs::remove_dir_all(&dir).ok();
        catalog
    }

    fn focus_on_chunk(world: &mut World, chunk_id: ChunkId) {
        let layout = world.resource::<WorldConfig>().chunk_layout();
        let coord = chunk_id.coord();
        let center = Vec3::new(
            coord.x as f32 * layout.chunk_size_units() + layout.chunk_size_units() * 0.5,
            0.0,
            coord.z as f32 * layout.chunk_size_units() + layout.chunk_size_units() * 0.5,
        );
        world.resource_mut::<PrimaryViewFocus>().position = center;
    }

    #[test]
    fn cache_hit_swaps_mesh_handle_and_active_lod() {
        let chunk_id = ChunkId::new(ChunkCoord::new(0, 0));
        let mut world = setup_lod_world(chunk_id);
        let (entity, half_handle, extra) =
            spawn_chunk_entity(&mut world, chunk_id, ChunkLod::Half, &[ChunkLod::Full]);
        let full_handle = extra[0].clone();
        focus_on_chunk(&mut world, chunk_id);

        apply_cached_swaps_in_world(&mut world);

        let marker = world.get::<TerrainChunkMesh>(entity).unwrap();
        let mesh3d = world.get::<Mesh3d>(entity).unwrap();
        assert_eq!(marker.active_lod, ChunkLod::Full);
        assert_eq!(mesh3d.0, full_handle);
        assert_ne!(mesh3d.0, half_handle);
    }

    #[test]
    fn cache_miss_enqueues_one_async_build() {
        ensure_task_pools();
        test_reset_build_mesh_calls();
        let chunk_id = ChunkId::new(ChunkCoord::new(1, 0));
        let mut world = setup_lod_world(chunk_id);
        world.insert_resource(TerrainStreamingSettings::default());
        world.insert_resource(test_catalog(&[(1, 0)]));
        spawn_chunk_entity(&mut world, chunk_id, ChunkLod::Half, &[]);
        focus_on_chunk(&mut world, chunk_id);

        request_missing_in_world(&mut world);
        assert_eq!(
            world.resource::<PendingChunkLodBuilds>().in_flight_count(),
            1
        );

        request_missing_in_world(&mut world);
        assert_eq!(
            world.resource::<PendingChunkLodBuilds>().in_flight_count(),
            1
        );
    }

    #[test]
    fn duplicate_build_request_for_same_chunk_lod_is_blocked() {
        ensure_task_pools();
        let chunk_id = ChunkId::new(ChunkCoord::new(2, 0));
        let data = sample_chunk_data();
        let mut pending = PendingChunkLodBuilds::default();
        let seam = ChunkMeshSeamWeld::default();

        assert!(pending.try_enqueue(chunk_id, ChunkLod::Full, data.clone(), 1.0, seam.clone()));
        assert!(!pending.try_enqueue(chunk_id, ChunkLod::Full, data, 1.0, seam));
        assert_eq!(pending.in_flight_count(), 1);
    }

    #[test]
    fn completed_lod_build_caches_handle() {
        ensure_task_pools();
        test_reset_build_mesh_calls();
        let chunk_id = ChunkId::new(ChunkCoord::new(3, 0));
        let mut world = setup_lod_world(chunk_id);
        let (entity, _, _) = spawn_chunk_entity(&mut world, chunk_id, ChunkLod::Half, &[]);
        focus_on_chunk(&mut world, chunk_id);

        {
            let data = sample_chunk_data();
            world.resource_mut::<PendingChunkLodBuilds>().try_enqueue(
                chunk_id,
                ChunkLod::Full,
                data,
                1.0,
                ChunkMeshSeamWeld::default(),
            );
        }

        for _ in 0..64 {
            if world.resource::<PendingChunkLodBuilds>().in_flight_count() == 0 {
                break;
            }
            poll_lod_builds_in_world(&mut world);
        }

        let cache = world.get::<TerrainChunkLodCache>(entity).unwrap();
        assert!(cache.has_lod(ChunkLod::Full));
        assert_eq!(cache.cached_lod_count(), 2);
    }

    #[test]
    fn completed_lod_build_swaps_only_when_desired_still_matches() {
        ensure_task_pools();
        let chunk_id = ChunkId::new(ChunkCoord::new(4, 0));
        let mut world = setup_lod_world(chunk_id);
        let (entity, half_handle, _) =
            spawn_chunk_entity(&mut world, chunk_id, ChunkLod::Half, &[]);

        {
            let data = sample_chunk_data();
            world.resource_mut::<PendingChunkLodBuilds>().try_enqueue(
                chunk_id,
                ChunkLod::Full,
                data,
                1.0,
                ChunkMeshSeamWeld::default(),
            );
        }

        // Move focus before build completes so Full is no longer desired (still Half).
        focus_on_chunk(&mut world, ChunkId::new(ChunkCoord::new(3, 0)));

        for _ in 0..64 {
            if world.resource::<PendingChunkLodBuilds>().in_flight_count() == 0 {
                break;
            }
            poll_lod_builds_in_world(&mut world);
        }

        let marker = world.get::<TerrainChunkMesh>(entity).unwrap();
        let mesh3d = world.get::<Mesh3d>(entity).unwrap();
        let cache = world.get::<TerrainChunkLodCache>(entity).unwrap();
        assert!(cache.has_lod(ChunkLod::Full));
        assert_eq!(marker.active_lod, ChunkLod::Half);
        assert_eq!(mesh3d.0, half_handle);
    }

    #[test]
    fn unload_cancels_in_flight_lod_build() {
        ensure_task_pools();
        let chunk_id = ChunkId::new(ChunkCoord::new(5, 0));
        let mut pending = PendingChunkLodBuilds::default();
        pending.try_enqueue(
            chunk_id,
            ChunkLod::Full,
            sample_chunk_data(),
            1.0,
            ChunkMeshSeamWeld::default(),
        );
        assert_eq!(pending.in_flight_count(), 1);
        pending.cancel_for_chunk(chunk_id);
        assert_eq!(pending.in_flight_count(), 0);
        assert!(pending.poll().is_empty());
    }

    #[test]
    fn cache_hit_swap_does_not_call_mesh_builder() {
        test_reset_build_mesh_calls();
        let chunk_id = ChunkId::new(ChunkCoord::new(0, 0));
        let mut world = setup_lod_world(chunk_id);
        spawn_chunk_entity(&mut world, chunk_id, ChunkLod::Half, &[ChunkLod::Full]);
        focus_on_chunk(&mut world, chunk_id);

        let builds_before = test_build_mesh_call_count();
        apply_cached_swaps_in_world(&mut world);

        assert_eq!(test_build_mesh_call_count(), builds_before);
    }

    #[test]
    fn prefetch_does_not_increase_resident_count() {
        ensure_task_pools();
        let chunk_id = ChunkId::new(ChunkCoord::new(0, 0));
        let mut world = setup_lod_world(chunk_id);
        world.insert_resource(test_catalog(&[(0, 0), (2, 0)]));
        world.insert_resource(TerrainStreamingSettings {
            load_radius_chunks: 0,
            ..Default::default()
        });
        spawn_chunk_entity(&mut world, chunk_id, ChunkLod::Eighth, &[]);
        focus_on_chunk(&mut world, chunk_id);

        let residents_before = world.resource::<WorldData>().len();
        request_missing_in_world(&mut world);
        assert_eq!(world.resource::<WorldData>().len(), residents_before);
    }

    #[test]
    fn cache_hit_prevents_prefetch_enqueue() {
        ensure_task_pools();
        let chunk_id = ChunkId::new(ChunkCoord::new(0, 0));
        let mut world = setup_lod_world(chunk_id);
        world.insert_resource(test_catalog(&[(0, 0)]));
        world.insert_resource(TerrainStreamingSettings {
            load_radius_chunks: 0,
            ..Default::default()
        });
        spawn_chunk_entity(
            &mut world,
            chunk_id,
            ChunkLod::Eighth,
            &[ChunkLod::Full, ChunkLod::Half, ChunkLod::Quarter],
        );
        focus_on_chunk(&mut world, chunk_id);

        let stats = request_missing_in_world_stats(&mut world);

        assert_eq!(stats.prefetch_requests, 0);
        assert_eq!(
            world.resource::<PendingChunkLodBuilds>().in_flight_count(),
            0
        );
    }

    #[test]
    fn high_priority_prefetch_wins_over_low_when_budget_limited() {
        ensure_task_pools();
        let high = ChunkId::new(ChunkCoord::new(2, 0));
        let low = ChunkId::new(ChunkCoord::new(3, 0));
        let mut world = World::new();
        world.init_resource::<WorldConfig>();
        world.init_resource::<WorldData>();
        world.init_resource::<TerrainChunkAlbedo>();
        world.init_resource::<PrimaryViewFocus>();
        world.init_resource::<Assets<Mesh>>();
        world.init_resource::<Assets<StandardMaterial>>();
        world.insert_resource(TerrainStreamingSettings {
            load_radius_chunks: 1,
            ..Default::default()
        });
        world.insert_resource(TerrainLodSettings {
            full_max_distance: 0,
            half_max_distance: 1,
            quarter_max_distance: 4,
            max_lod_prefetch_per_frame: 1,
            max_lod_builds_per_frame: 0,
        });
        world.insert_resource(test_catalog(&[(2, 0), (3, 0)]));
        world.insert_resource(PendingChunkLodBuilds::default());
        let material = {
            let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
            materials.add(StandardMaterial::default())
        };
        world.insert_resource(TerrainRenderAssets {
            material,
            vertical_scale: 1.0,
        });
        world.register_component::<TerrainChunkMesh>();
        world.register_component::<TerrainChunkLodCache>();
        world
            .resource_mut::<WorldData>()
            .insert(high, sample_chunk_data());
        world
            .resource_mut::<WorldData>()
            .insert(low, sample_chunk_data());
        spawn_chunk_entity(&mut world, high, ChunkLod::Quarter, &[ChunkLod::Quarter]);
        spawn_chunk_entity(&mut world, low, ChunkLod::Quarter, &[ChunkLod::Quarter]);
        focus_on_chunk(&mut world, ChunkId::new(ChunkCoord::new(0, 0)));

        request_missing_in_world(&mut world);

        assert_eq!(
            world.resource::<PendingChunkLodBuilds>().in_flight_count(),
            1
        );
        assert!(
            world
                .resource::<PendingChunkLodBuilds>()
                .has_in_flight(high, ChunkLod::Half)
        );
    }

    #[test]
    fn prefetch_does_not_change_streaming_load_radius() {
        let settings = TerrainStreamingSettings::default();
        assert_eq!(settings.load_radius_chunks, 1);
        assert_eq!(settings.unload_radius_chunks, 2);
    }

    #[test]
    fn stationary_prefetch_is_stable() {
        ensure_task_pools();
        let chunk_id = ChunkId::new(ChunkCoord::new(0, 0));
        let mut world = setup_lod_world(chunk_id);
        world.insert_resource(test_catalog(&[(0, 0)]));
        world.insert_resource(TerrainStreamingSettings::default());
        spawn_chunk_entity(&mut world, chunk_id, ChunkLod::Half, &[ChunkLod::Full]);
        focus_on_chunk(&mut world, chunk_id);

        for _ in 0..4 {
            request_missing_in_world(&mut world);
        }
        assert_eq!(
            world.resource::<PendingChunkLodBuilds>().in_flight_count(),
            0
        );
        assert_eq!(world.resource::<WorldData>().len(), 1);
    }
}
