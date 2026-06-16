//! Terrain chunk streaming lifecycle (ADR-012).
//!
//! Phase 2B.5: async IO + decode materialization with main-thread apply.

use std::collections::HashSet;
#[cfg(feature = "dev")]
use std::time::Instant;

use bevy::prelude::*;

use crate::view::PrimaryViewFocus;
use crate::world::{ChunkCoord, ChunkId, WorldConfig, WorldData};

use super::components::TerrainChunkMesh;
use super::catalog::TerrainWorldCatalog;
use super::grace::JustAppliedGrace;
use super::materialize::{
    DecodedChunkPending, PendingChunkMaterializations, decoded_result_may_apply,
};
use super::load::validate_loaded_chunk;
use super::residency::{
    ChunkDiscardKind, ChunkResidencyTracker, discard_chunk_residency,
};
use super::spawn::{
    TerrainRenderAssets, despawn_chunk_meshes, refresh_adjacent_chunk_meshes_inner,
    spawn_chunk_mesh_inner,
};
use super::streaming::{
    TerrainStreamingSettings, chunks_in_radius, diff_streaming_residency, stable_focus_chunk,
};
#[cfg(feature = "dev")]
use super::perf::{TerrainStreamingPerfRecorder, TerrainStreamingPerfSettings, duration_to_ms};

/// Systems that drive terrain chunk residency.
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct TerrainStreamingSystems;

/// Reset per-frame streaming perf counters (dev only).
#[cfg(feature = "dev")]
pub fn begin_terrain_streaming_perf_frame(
    #[cfg(feature = "dev")] settings: Res<TerrainStreamingPerfSettings>,
    #[cfg(feature = "dev")] mut perf_state: ResMut<super::perf::TerrainStreamingPerfState>,
) {
    #[cfg(feature = "dev")]
    if !settings.enabled {
        return;
    }
    #[cfg(feature = "dev")]
    perf_state.begin_frame();
}

/// Request async loads for chunks entering the desired load set.
pub fn stream_terrain_chunks(
    focus: Res<PrimaryViewFocus>,
    catalog: Res<TerrainWorldCatalog>,
    settings: Res<TerrainStreamingSettings>,
    config: Res<WorldConfig>,
    mut residency: ResMut<ChunkResidencyTracker>,
    mut pending: ResMut<PendingChunkMaterializations>,
    world: Res<WorldData>,
) {
    let layout = config.chunk_layout();
    let focus_coord = stable_focus_chunk(focus.position, layout);
    let desired_load =
        chunks_in_radius(focus_coord, settings.load_radius_chunks, &catalog);
    let keep_resident =
        chunks_in_radius(focus_coord, settings.unload_radius_chunks, &catalog);

    pending.discard_outside_residency_sets(
        &mut residency,
        &keep_resident,
        &desired_load,
    );

    let (to_load, _) = diff_streaming_residency(
        &focus,
        layout,
        &settings,
        &catalog,
        &world,
        &HashSet::new(),
    );

    for coord in to_load {
        if catalog.get(coord).is_none() {
            continue;
        };
        let Some(path) = catalog.chunk_path(coord) else {
            continue;
        };

        let chunk_id = ChunkId::new(coord);
        let Some(generation) = residency.begin_loading(chunk_id) else {
            continue;
        };

        if !pending.try_start_io(chunk_id, generation, path) {
            pending.discard_chunk_state(
                &mut residency,
                chunk_id,
                ChunkDiscardKind::RejectedCompletion { generation },
            );
        }
    }
}

/// Poll IO/decode tasks and store decoded [`ChunkData`].
pub fn poll_chunk_materializations(
    focus: Res<PrimaryViewFocus>,
    catalog: Res<TerrainWorldCatalog>,
    settings: Res<TerrainStreamingSettings>,
    config: Res<WorldConfig>,
    mut residency: ResMut<ChunkResidencyTracker>,
    mut pending: ResMut<PendingChunkMaterializations>,
    #[cfg(feature = "dev")] perf_settings: Res<TerrainStreamingPerfSettings>,
    #[cfg(feature = "dev")] mut perf_state: ResMut<super::perf::TerrainStreamingPerfState>,
) {
    #[cfg(feature = "dev")]
    let poll_start = perf_settings.enabled.then(Instant::now);

    let layout = config.chunk_layout();
    let focus_coord = stable_focus_chunk(focus.position, layout);
    let keep_resident: HashSet<_> =
        chunks_in_radius(focus_coord, settings.unload_radius_chunks, &catalog);

    pending.poll_in_flight(
        &mut residency,
        &keep_resident,
        settings.max_decode_per_frame,
    );

    #[cfg(feature = "dev")]
    if perf_settings.enabled {
        let frame = perf_state.frame_mut();
        frame.poll_ms = duration_to_ms(poll_start.unwrap().elapsed());
        frame.io_in_flight = pending.io_in_flight_count();
        frame.decode_in_flight = pending.decode_in_flight_count();
        frame.decoded_queue_len = pending.decoded_len();
    }
}

/// Apply decoded chunks to [`WorldData`] and spawn derived render entities.
pub fn apply_chunk_materializations(
    focus: Res<PrimaryViewFocus>,
    catalog: Res<TerrainWorldCatalog>,
    settings: Res<TerrainStreamingSettings>,
    config: Res<WorldConfig>,
    render_assets: Res<TerrainRenderAssets>,
    mut residency: ResMut<ChunkResidencyTracker>,
    mut pending: ResMut<PendingChunkMaterializations>,
    mut grace: ResMut<JustAppliedGrace>,
    mut world: ResMut<WorldData>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mesh_entities: Query<(Entity, &TerrainChunkMesh)>,
    #[cfg(feature = "dev")] perf_settings: Res<TerrainStreamingPerfSettings>,
    #[cfg(feature = "dev")] mut perf_state: ResMut<super::perf::TerrainStreamingPerfState>,
) {
    #[cfg(feature = "dev")]
    let apply_start = perf_settings.enabled.then(Instant::now);
    #[cfg(feature = "dev")]
    let mut mesh_perf = perf_settings.enabled.then(TerrainStreamingPerfRecorder::default);

    let layout = config.chunk_layout();
    let focus_coord = stable_focus_chunk(focus.position, layout);
    let keep_resident: HashSet<_> =
        chunks_in_radius(focus_coord, settings.unload_radius_chunks, &catalog);

    let decoded = pending.take_decoded();

    let (applied, deferred) = apply_decoded_batch(
        decoded,
        settings.max_apply_per_frame,
        &catalog,
        &config,
        &mut residency,
        &mut world,
        &keep_resident,
        &mesh_entities,
        &mut commands,
        &mut meshes,
        &render_assets,
        layout.chunk_size_units(),
        #[cfg(feature = "dev")]
        mesh_perf.as_mut(),
    );

    for chunk_id in &applied {
        grace.grant(*chunk_id);
    }

    pending.requeue_decoded(deferred);

    #[cfg(feature = "dev")]
    if perf_settings.enabled {
        let frame = perf_state.frame_mut();
        frame.apply_ms = duration_to_ms(apply_start.unwrap().elapsed());
        frame.chunks_applied = applied.len();
        frame.io_in_flight = pending.io_in_flight_count();
        frame.decode_in_flight = pending.decode_in_flight_count();
        frame.decoded_queue_len = pending.decoded_len();
        if let Some(recorder) = mesh_perf.as_ref() {
            recorder.finish_into(frame);
        }
    }
}

/// Unload resident chunks outside the keep band.
pub fn unload_terrain_chunks(
    focus: Res<PrimaryViewFocus>,
    catalog: Res<TerrainWorldCatalog>,
    settings: Res<TerrainStreamingSettings>,
    config: Res<WorldConfig>,
    mut residency: ResMut<ChunkResidencyTracker>,
    mut pending: ResMut<PendingChunkMaterializations>,
    mut grace: ResMut<JustAppliedGrace>,
    mut world: ResMut<WorldData>,
    mut commands: Commands,
    mesh_entities: Query<(Entity, &TerrainChunkMesh)>,
    #[cfg(feature = "dev")] perf_settings: Res<TerrainStreamingPerfSettings>,
    #[cfg(feature = "dev")] mut perf_state: ResMut<super::perf::TerrainStreamingPerfState>,
) {
    let layout = config.chunk_layout();
    let (_, to_unload) = diff_streaming_residency(
        &focus,
        layout,
        &settings,
        &catalog,
        &world,
        grace.as_set(),
    );

    for chunk_id in &to_unload {
        pending.discard_chunk_state(&mut residency, *chunk_id, ChunkDiscardKind::Revoked);
        despawn_chunk_meshes(&mut commands, *chunk_id, &mesh_entities);
        world.remove(*chunk_id);
    }

    #[cfg(feature = "dev")]
    if perf_settings.enabled {
        perf_state.frame_mut().chunks_unloaded = to_unload.len();
    }

    grace.clear();
}

/// Apply decoded entries to authoritative world data and spawn meshes.
pub(crate) fn apply_decoded_batch(
    mut decoded: Vec<DecodedChunkPending>,
    max_apply: usize,
    catalog: &TerrainWorldCatalog,
    config: &WorldConfig,
    residency: &mut ChunkResidencyTracker,
    world: &mut WorldData,
    keep_resident: &HashSet<ChunkCoord>,
    mesh_entities: &Query<(Entity, &TerrainChunkMesh)>,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    render_assets: &TerrainRenderAssets,
    chunk_size_units: f32,
    #[cfg(feature = "dev")] mut perf: Option<&mut TerrainStreamingPerfRecorder>,
) -> (Vec<ChunkId>, Vec<DecodedChunkPending>) {
    decoded.sort_by_key(|entry| (entry.chunk_id.coord().z, entry.chunk_id.coord().x));
    decoded.dedup_by_key(|entry| entry.chunk_id);

    let mut applied = Vec::new();
    let mut deferred = Vec::new();
    let mut applied_count = 0usize;

    for entry in decoded {
        if applied_count >= max_apply {
            deferred.push(entry);
            continue;
        }

        let chunk_id = entry.chunk_id;
        let generation = entry.generation;

        if !decoded_result_may_apply(residency, chunk_id, generation, keep_resident) {
            discard_chunk_residency(
                residency,
                chunk_id,
                ChunkDiscardKind::RejectedCompletion { generation },
            );
            continue;
        }

        if world.is_chunk_loaded(chunk_id) {
            if residency.loading_generation_matches(chunk_id, generation) {
                residency.mark_resident(chunk_id);
            } else {
                discard_chunk_residency(
                    residency,
                    chunk_id,
                    ChunkDiscardKind::RejectedCompletion { generation },
                );
            }
            continue;
        }

        if mesh_entities
            .iter()
            .any(|(_, marker)| marker.chunk == chunk_id)
        {
            if residency.loading_generation_matches(chunk_id, generation) {
                residency.mark_resident(chunk_id);
            } else {
                discard_chunk_residency(
                    residency,
                    chunk_id,
                    ChunkDiscardKind::RejectedCompletion { generation },
                );
            }
            continue;
        }

        let coord = chunk_id.coord();
        let Some(manifest_entry) = catalog.get(coord) else {
            discard_chunk_residency(
                residency,
                chunk_id,
                ChunkDiscardKind::RejectedCompletion { generation },
            );
            continue;
        };

        if let Err(err) = validate_loaded_chunk(manifest_entry, chunk_id, &entry.data, config) {
            bevy::log::error!(
                "chunk apply validation failed ({}, {}): {err}",
                coord.x,
                coord.z
            );
            discard_chunk_residency(
                residency,
                chunk_id,
                ChunkDiscardKind::RejectedCompletion { generation },
            );
            continue;
        }

        world.insert(chunk_id, entry.data);
        spawn_chunk_mesh_inner(
            commands,
            chunk_id,
            world,
            chunk_size_units,
            meshes,
            render_assets.material.clone(),
            render_assets.vertical_scale,
            #[cfg(feature = "dev")]
            perf.as_deref_mut(),
            #[cfg(feature = "dev")]
            super::perf::MeshBuildKind::NewChunk,
        );
        refresh_adjacent_chunk_meshes_inner(
            commands,
            chunk_id,
            world,
            chunk_size_units,
            meshes,
            render_assets.material.clone(),
            render_assets.vertical_scale,
            mesh_entities,
            #[cfg(feature = "dev")]
            perf.as_deref_mut(),
        );
        residency.mark_resident(chunk_id);
        applied.push(chunk_id);
        applied_count += 1;
    }

    (applied, deferred)
}

#[cfg(test)]
mod apply_tests {
    use super::*;
    use crate::terrain::catalog::TerrainWorldCatalog;
    use crate::terrain::materialize::PendingChunkMaterializations;
    use crate::world::{ChunkData, Heightfield};
    use bevy::ecs::system::SystemState;

    fn sample_chunk_data() -> ChunkData {
        ChunkData::new(
            Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap(),
            Vec::new(),
        )
    }

    fn sample_render_assets(world: &mut World) -> TerrainRenderAssets {
        let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
        let material = materials.add(StandardMaterial::default());
        TerrainRenderAssets {
            material,
            vertical_scale: 1.0,
        }
    }

    fn setup_apply_world() -> World {
        let mut world = World::new();
        world.init_resource::<WorldConfig>();
        world.init_resource::<WorldData>();
        world.init_resource::<ChunkResidencyTracker>();
        world.init_resource::<Assets<Mesh>>();
        world.init_resource::<Assets<StandardMaterial>>();
        world.register_component::<TerrainChunkMesh>();
        world
    }

    fn test_catalog_for_coords(coords: &[(i32, i32)]) -> TerrainWorldCatalog {
        use crate::terrain::asset::{ManifestChunk, MANIFEST_FORMAT_VERSION};
        use std::sync::atomic::{AtomicU64, Ordering};

        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "chasma_apply_cat_{}_{}",
            std::process::id(),
            id
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let cfg = WorldConfig::default();
        let entries: Vec<_> = coords
            .iter()
            .map(|&(x, z)| ManifestChunk {
                x,
                z,
                path: format!("chunks/{x}_{z}.ron"),
            })
            .collect();
        let manifest = crate::terrain::asset::Manifest {
            version: MANIFEST_FORMAT_VERSION,
            config: crate::terrain::asset::ManifestConfig {
                chunk_size_meters: cfg.chunk_size_meters,
                units_per_meter: cfg.units_per_meter,
                meters_per_sample: cfg.meters_per_sample,
            },
            chunks: entries,
        };
        std::fs::write(
            dir.join("manifest.ron"),
            ron::to_string(&manifest).unwrap(),
        )
        .unwrap();
        TerrainWorldCatalog::from_manifest(&dir.join("manifest.ron"), &cfg).unwrap()
    }

    fn apply_entries(
        world: &mut World,
        decoded: Vec<DecodedChunkPending>,
        keep_resident: &HashSet<ChunkCoord>,
        catalog: &TerrainWorldCatalog,
    ) {
        let render_assets = sample_render_assets(world);
        let config = world.resource::<WorldConfig>().clone();
        let chunk_size_units = config.chunk_layout().chunk_size_units();

        let mut system_state = SystemState::<(
            Commands,
            ResMut<WorldData>,
            ResMut<ChunkResidencyTracker>,
            ResMut<Assets<Mesh>>,
            Query<(Entity, &TerrainChunkMesh)>,
        )>::new(world);

        {
            let (mut commands, mut world_data, mut residency, mut meshes, mesh_entities) =
                system_state.get_mut(world);
            let (_, _) = apply_decoded_batch(
                decoded,
                usize::MAX,
                catalog,
                &config,
                &mut residency,
                &mut world_data,
                keep_resident,
                &mesh_entities,
                &mut commands,
                &mut meshes,
                &render_assets,
                chunk_size_units,
                #[cfg(feature = "dev")]
                None,
            );
        }

        system_state.apply(world);
    }

    #[test]
    fn decoded_chunk_becomes_world_data_entry_after_apply() {
        let chunk_id = ChunkId::new(ChunkCoord::new(1, 1));
        let catalog = test_catalog_for_coords(&[(1, 1)]);
        let mut world = setup_apply_world();

        {
            let mut residency = world.resource_mut::<ChunkResidencyTracker>();
            let generation = residency.begin_loading(chunk_id).unwrap();
            let mut keep = HashSet::new();
            keep.insert(chunk_id.coord());

            apply_entries(
                &mut world,
                vec![DecodedChunkPending {
                    chunk_id,
                    generation,
                    data: sample_chunk_data(),
                }],
                &keep,
                &catalog,
            );
        }

        let world_data = world.resource::<WorldData>();
        assert!(world_data.is_chunk_loaded(chunk_id));
        assert!(world.resource::<ChunkResidencyTracker>().is_resident(chunk_id));
    }

    #[test]
    fn entity_exists_after_apply() {
        let chunk_id = ChunkId::new(ChunkCoord::new(2, 0));
        let catalog = test_catalog_for_coords(&[(2, 0)]);
        let mut world = setup_apply_world();

        {
            let mut residency = world.resource_mut::<ChunkResidencyTracker>();
            let generation = residency.begin_loading(chunk_id).unwrap();
            let mut keep = HashSet::new();
            keep.insert(chunk_id.coord());

            apply_entries(
                &mut world,
                vec![DecodedChunkPending {
                    chunk_id,
                    generation,
                    data: sample_chunk_data(),
                }],
                &keep,
                &catalog,
            );
        }

        let mut query = world.query::<&TerrainChunkMesh>();
        let markers: Vec<_> = query.iter(&world).collect();
        assert_eq!(markers.len(), 1);
        assert_eq!(markers[0].chunk, chunk_id);
    }

    #[test]
    fn stale_generation_chunk_is_not_applied() {
        let chunk_id = ChunkId::new(ChunkCoord::new(0, 3));
        let catalog = test_catalog_for_coords(&[(0, 3)]);
        let mut world = setup_apply_world();

        let stale_generation = {
            let mut residency = world.resource_mut::<ChunkResidencyTracker>();
            let first = residency.begin_loading(chunk_id).unwrap();
            residency.cancel(chunk_id);
            let _second = residency.begin_loading(chunk_id).unwrap();
            first
        };

        let mut keep = HashSet::new();
        keep.insert(chunk_id.coord());

        apply_entries(
            &mut world,
            vec![DecodedChunkPending {
                chunk_id,
                generation: stale_generation,
                data: sample_chunk_data(),
            }],
            &keep,
            &catalog,
        );

        let world_data = world.resource::<WorldData>();
        assert!(!world_data.is_chunk_loaded(chunk_id));
        assert!(!world.resource::<ChunkResidencyTracker>().is_resident(chunk_id));
        assert!(world.resource::<ChunkResidencyTracker>().is_loading(chunk_id));

        let mut query = world.query::<&TerrainChunkMesh>();
        assert_eq!(query.iter(&world).count(), 0);
    }

    #[test]
    fn removed_chunk_does_not_spawn_entity() {
        let chunk_id = ChunkId::new(ChunkCoord::new(4, 4));
        let catalog = test_catalog_for_coords(&[(4, 4)]);
        let mut world = setup_apply_world();

        let generation = {
            let mut residency = world.resource_mut::<ChunkResidencyTracker>();
            let generation = residency.begin_loading(chunk_id).unwrap();
            residency.cancel(chunk_id);
            generation
        };

        let keep = HashSet::new();

        apply_entries(
            &mut world,
            vec![DecodedChunkPending {
                chunk_id,
                generation,
                data: sample_chunk_data(),
            }],
            &keep,
            &catalog,
        );

        assert!(!world.resource::<WorldData>().is_chunk_loaded(chunk_id));
        assert!(!world.resource::<ChunkResidencyTracker>().is_loading(chunk_id));
        let mut query = world.query::<&TerrainChunkMesh>();
        assert_eq!(query.iter(&world).count(), 0);
    }

    #[test]
    fn no_duplicate_entity_per_chunk_id() {
        let chunk_id = ChunkId::new(ChunkCoord::new(3, 1));
        let catalog = test_catalog_for_coords(&[(3, 1)]);
        let mut world = setup_apply_world();

        let generation = {
            let mut residency = world.resource_mut::<ChunkResidencyTracker>();
            residency.begin_loading(chunk_id).unwrap()
        };

        let mut keep = HashSet::new();
        keep.insert(chunk_id.coord());

        let data = sample_chunk_data();
        apply_entries(
            &mut world,
            vec![DecodedChunkPending {
                chunk_id,
                generation,
                data: data.clone(),
            }],
            &keep,
            &catalog,
        );
        apply_entries(
            &mut world,
            vec![DecodedChunkPending {
                chunk_id,
                generation,
                data,
            }],
            &keep,
            &catalog,
        );

        let mut query = world.query::<&TerrainChunkMesh>();
        assert_eq!(query.iter(&world).count(), 1);
        assert!(world.resource::<WorldData>().is_chunk_loaded(chunk_id));
    }

    #[test]
    fn io_and_decode_pipeline_unchanged_after_apply_step() {
        use crate::terrain::materialize::{
            decode_chunk_text, read_chunk_file_text, spawn_chunk_decode_task, spawn_chunk_io_task,
        };
        use crate::terrain::asset::{CHUNK_FORMAT_VERSION, ChunkFile};
        use bevy::tasks::TaskPoolBuilder;
        use std::sync::Once;

        static TASK_POOLS: Once = Once::new();
        TASK_POOLS.call_once(|| {
            bevy::tasks::IoTaskPool::get_or_init(|| TaskPoolBuilder::new().num_threads(1).build());
            bevy::tasks::AsyncComputeTaskPool::get_or_init(|| {
                TaskPoolBuilder::new().num_threads(1).build()
            });
        });

        let mut samples = Vec::new();
        for row in 0..3 {
            for col in 0..3 {
                samples.push((row * 10 + col) as f32);
            }
        }
        let file = ChunkFile {
            version: CHUNK_FORMAT_VERSION,
            x: 7,
            z: 8,
            samples_per_edge: 3,
            spacing_meters: 128.0,
            samples,
            height_min: 0.0,
            height_max: 22.0,
        };
        let path = std::env::temp_dir().join(format!(
            "chasma_apply_reg_{}_7_8.ron",
            std::process::id(),
        ));
        std::fs::write(&path, ron::to_string(&file).unwrap()).unwrap();

        let raw = read_chunk_file_text(&path).unwrap();
        let mut io_task = spawn_chunk_io_task(path.clone());
        assert_eq!(bevy::tasks::block_on(&mut io_task).unwrap(), raw);

        let mut decode_task = spawn_chunk_decode_task(raw);
        let (id, data) = bevy::tasks::block_on(&mut decode_task).unwrap();
        assert_eq!(id, ChunkId::new(ChunkCoord::new(7, 8)));
        assert_eq!(data.heightfield.samples_per_edge(), 3);
        assert_eq!(decode_chunk_text(&std::fs::read_to_string(&path).unwrap()).unwrap().0, id);

        let mut pending = PendingChunkMaterializations::default();
        assert!(pending.try_start_io(id, 1, path.clone()));
        assert!(!pending.try_start_io(id, 2, path.clone()));

        std::fs::remove_file(path).ok();
    }

    #[test]
    fn apply_budget_limits_number_of_chunks_per_frame() {
        let mut world = setup_apply_world();

        let mut keep = HashSet::new();
        let mut entries = Vec::new();
        {
            let mut residency = world.resource_mut::<ChunkResidencyTracker>();
            for i in 0..5 {
                let chunk_id = ChunkId::new(ChunkCoord::new(i, 0));
                keep.insert(chunk_id.coord());
                let generation = residency.begin_loading(chunk_id).unwrap();
                entries.push(DecodedChunkPending {
                    chunk_id,
                    generation,
                    data: sample_chunk_data(),
                });
            }
        }

        let catalog = test_catalog_for_coords(&[(0, 0), (1, 0), (2, 0), (3, 0), (4, 0)]);
        let decoded = entries;
        let budget = 2;

        let render_assets = sample_render_assets(&mut world);
        let chunk_size_units = world.resource::<WorldConfig>().chunk_layout().chunk_size_units();
        let config = world.resource::<WorldConfig>().clone();
        let mut system_state = SystemState::<(
            Commands,
            ResMut<WorldData>,
            ResMut<ChunkResidencyTracker>,
            ResMut<Assets<Mesh>>,
            Query<(Entity, &TerrainChunkMesh)>,
        )>::new(&mut world);

        let (applied, deferred) = {
            let (mut commands, mut world_data, mut residency, mut meshes, mesh_entities) =
                system_state.get_mut(&mut world);
            apply_decoded_batch(
                decoded,
                budget,
                &catalog,
                &config,
                &mut residency,
                &mut world_data,
                &keep,
                &mesh_entities,
                &mut commands,
                &mut meshes,
                &render_assets,
                chunk_size_units,
                #[cfg(feature = "dev")]
                None,
            )
        };
        system_state.apply(&mut world);

        assert_eq!(applied.len(), budget);
        assert_eq!(deferred.len(), 3);
        assert_eq!(world.resource::<WorldData>().len(), budget);
    }

    #[test]
    fn no_duplicate_chunk_spawn_under_high_io_load() {
        use bevy::tasks::TaskPoolBuilder;
        use std::sync::Once;

        static TASK_POOLS: Once = Once::new();
        TASK_POOLS.call_once(|| {
            bevy::tasks::IoTaskPool::get_or_init(|| TaskPoolBuilder::new().num_threads(2).build());
            bevy::tasks::AsyncComputeTaskPool::get_or_init(|| {
                TaskPoolBuilder::new().num_threads(2).build()
            });
        });

        let catalog = test_catalog_for_coords(
            &(0..8)
                .map(|i| (i, 0))
                .collect::<Vec<_>>(),
        );
        let mut pending = PendingChunkMaterializations::default();
        let mut world = setup_apply_world();

        let mut keep = HashSet::new();
        let mut paths = Vec::new();
        {
            let mut residency = world.resource_mut::<ChunkResidencyTracker>();
            for i in 0..8 {
                let chunk_id = ChunkId::new(ChunkCoord::new(i, 0));
                keep.insert(chunk_id.coord());
                let generation = residency.begin_loading(chunk_id).unwrap();
                let path = std::env::temp_dir().join(format!(
                    "chasma_high_io_{}_{}_{i}.ron",
                    std::process::id(),
                    i
                ));
                let file = crate::terrain::asset::ChunkFile {
                    version: crate::terrain::asset::CHUNK_FORMAT_VERSION,
                    x: i,
                    z: 0,
                    samples_per_edge: 3,
                    spacing_meters: 128.0,
                    samples: vec![0.0; 9],
                    height_min: 0.0,
                    height_max: 0.0,
                };
                std::fs::write(&path, ron::to_string(&file).unwrap()).unwrap();
                paths.push(path.clone());
                assert!(pending.try_start_io(chunk_id, generation, path));
            }
        }

        assert_eq!(pending.in_flight_count(), 8);

        assert_eq!(pending.unique_pipeline_chunk_count(), 8);

        for _ in 0..16 {
            let mut residency = world.resource_mut::<ChunkResidencyTracker>();
            pending.poll_in_flight(&mut residency, &keep, 2);
        }

        let decoded = pending.take_decoded();
        let render_assets = sample_render_assets(&mut world);
        let chunk_size_units = world.resource::<WorldConfig>().chunk_layout().chunk_size_units();
        let config = world.resource::<WorldConfig>().clone();
        let mut system_state = SystemState::<(
            Commands,
            ResMut<WorldData>,
            ResMut<ChunkResidencyTracker>,
            ResMut<Assets<Mesh>>,
            Query<(Entity, &TerrainChunkMesh)>,
        )>::new(&mut world);

        let (applied, deferred) = {
            let (mut commands, mut world_data, mut residency, mut meshes, mesh_entities) =
                system_state.get_mut(&mut world);
            apply_decoded_batch(
                decoded,
                4,
                &catalog,
                &config,
                &mut residency,
                &mut world_data,
                &keep,
                &mesh_entities,
                &mut commands,
                &mut meshes,
                &render_assets,
                chunk_size_units,
                #[cfg(feature = "dev")]
                None,
            )
        };
        system_state.apply(&mut world);
        pending.requeue_decoded(deferred);

        let mut query = world.query::<&TerrainChunkMesh>();
        let markers: Vec<_> = query.iter(&world).collect();
        assert_eq!(markers.len(), applied.len());
        assert!(markers.len() <= 4);
        assert_eq!(
            markers.len(),
            world.resource::<WorldData>().len(),
            "one entity per resident chunk"
        );

        for path in paths {
            std::fs::remove_file(path).ok();
        }
    }

    #[test]
    fn invalid_chunk_data_is_rejected_at_apply() {
        let chunk_id = ChunkId::new(ChunkCoord::new(6, 6));
        let catalog = test_catalog_for_coords(&[(6, 6)]);
        let mut world = setup_apply_world();

        let generation = {
            let mut residency = world.resource_mut::<ChunkResidencyTracker>();
            residency.begin_loading(chunk_id).unwrap()
        };

        let mut keep = HashSet::new();
        keep.insert(chunk_id.coord());

        let mut bad_data = sample_chunk_data();
        bad_data.heightfield = crate::world::Heightfield::from_samples(3, 64.0, vec![0.0; 9])
            .unwrap();

        apply_entries(
            &mut world,
            vec![DecodedChunkPending {
                chunk_id,
                generation,
                data: bad_data,
            }],
            &keep,
            &catalog,
        );

        assert!(!world.resource::<WorldData>().is_chunk_loaded(chunk_id));
        assert!(!world.resource::<ChunkResidencyTracker>().is_loading(chunk_id));
        let mut query = world.query::<&TerrainChunkMesh>();
        assert_eq!(query.iter(&world).count(), 0);
    }

    #[test]
    fn chunk_survives_apply_frame_without_immediate_unload() {
        use super::super::streaming::diff_streaming_residency;

        let catalog = {
            use crate::terrain::asset::{ManifestChunk, MANIFEST_FORMAT_VERSION};
            use crate::world::WorldConfig;
            use std::sync::atomic::{AtomicU64, Ordering};

            static NEXT_ID: AtomicU64 = AtomicU64::new(0);
            let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!(
                "chasma_grace_{}_{}",
                std::process::id(),
                id
            ));
            std::fs::create_dir_all(&dir).unwrap();
            let cfg = WorldConfig::default();
            let manifest = crate::terrain::asset::Manifest {
                version: MANIFEST_FORMAT_VERSION,
                config: crate::terrain::asset::ManifestConfig {
                    chunk_size_meters: cfg.chunk_size_meters,
                    units_per_meter: cfg.units_per_meter,
                    meters_per_sample: cfg.meters_per_sample,
                },
                chunks: vec![
                    ManifestChunk {
                        x: 0,
                        z: 0,
                        path: "chunks/0_0.ron".to_string(),
                    },
                    ManifestChunk {
                        x: 3,
                        z: 0,
                        path: "chunks/3_0.ron".to_string(),
                    },
                ],
            };
            std::fs::write(
                dir.join("manifest.ron"),
                ron::to_string(&manifest).unwrap(),
            )
            .unwrap();
            TerrainWorldCatalog::from_manifest(&dir.join("manifest.ron"), &cfg).unwrap()
        };

        let layout = WorldConfig::default().chunk_layout();
        let mut world_data = WorldData::new(layout);
        world_data.set_authored_extent(catalog.authored_extent());
        let chunk_id = ChunkId::new(ChunkCoord::new(3, 0));
        world_data.insert(chunk_id, sample_chunk_data());

        let focus = PrimaryViewFocus::new(Vec3::new(128.0, 0.0, 0.0));
        let settings = TerrainStreamingSettings {
            load_radius_chunks: 1,
            unload_radius_chunks: 2,
            max_loads_per_frame: 16,
            max_unloads_per_frame: 16,
            max_apply_per_frame: 16,
            max_decode_per_frame: 16,
        };

        let mut grace = JustAppliedGrace::default();
        grace.grant(chunk_id);

        let (_, to_unload_without_grace) = diff_streaming_residency(
            &focus,
            layout,
            &settings,
            &catalog,
            &world_data,
            &HashSet::new(),
        );
        assert!(to_unload_without_grace.contains(&chunk_id));

        let (_, to_unload) = diff_streaming_residency(
            &focus,
            layout,
            &settings,
            &catalog,
            &world_data,
            grace.as_set(),
        );
        assert!(!to_unload.contains(&chunk_id));
        assert!(world_data.is_chunk_loaded(chunk_id));
    }
}
