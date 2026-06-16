//! Async chunk materialization pipeline (ADR-012 Phase 2B.5–2B.6).
//!
//! IO, decode, and mesh build run off the main thread. Apply inserts
//! [`ChunkData`], registers a prebuilt [`Mesh`], and spawns render entities.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, IoTaskPool, Task};

use crate::world::{ChunkCoord, ChunkData, ChunkId};

use super::albedo::{AlbedoFallback, ChunkAlbedoGrid};
use super::albedo_decode::load_albedo_sidecar_absolute;
use super::asset::TerrainAssetError;
use super::decode::decode_chunk;
use super::lod::{TerrainLodSettings, desired_lod};
use super::mesh::{ChunkLod, ChunkMeshSeamWeld, build_chunk_mesh_scaled};
use super::streaming::chunk_outside_residency_sets;
use super::residency::{ChunkDiscardKind, ChunkResidencyTracker, discard_chunk_residency};

/// Raw chunk file text read off the main thread.
pub type ChunkIoTask = Task<Result<String, TerrainAssetError>>;

/// Decoded chunk payload produced on the compute pool.
pub type ChunkDecodeTask = Task<Result<(ChunkId, ChunkData), TerrainAssetError>>;

/// Mesh output produced on the compute pool after decode.
#[derive(Debug)]
pub struct MeshBuildOutput {
    pub data: ChunkData,
    pub albedo: Option<ChunkAlbedoGrid>,
    pub mesh: Mesh,
    pub build_duration: Duration,
}

/// Async mesh-build task (compute pool).
pub type ChunkMeshBuildTask = Task<MeshBuildOutput>;

/// Chunk with decoded data and prebuilt mesh awaiting main-thread apply.
#[derive(Debug)]
pub struct MaterializedChunkPending {
    pub chunk_id: ChunkId,
    pub generation: u64,
    pub data: ChunkData,
    pub albedo: Option<ChunkAlbedoGrid>,
    pub mesh: Mesh,
    pub lod: ChunkLod,
    pub async_mesh_build_ms: f32,
}

/// Per-poll stats for async mesh build (surfaced in dev perf).
#[derive(Debug, Default, Clone, Copy)]
pub struct MaterializePollStats {
    pub async_mesh_build_ms: f32,
    pub async_mesh_builds_completed: usize,
}

enum MaterializeStage {
    Io(ChunkIoTask),
    IoReady { raw: String },
    Decode(ChunkDecodeTask),
    DecodeReady { data: ChunkData },
    MeshBuild(ChunkMeshBuildTask),
    MeshBuildReady {
        data: ChunkData,
        albedo: Option<ChunkAlbedoGrid>,
        mesh: Mesh,
        async_mesh_build_ms: f32,
    },
}

struct InFlightMaterialization {
    chunk_id: ChunkId,
    generation: u64,
    mesh_lod: Option<ChunkLod>,
    albedo_path: Option<PathBuf>,
    stage: MaterializeStage,
}

impl InFlightMaterialization {
    fn is_io_in_flight(&self) -> bool {
        matches!(
            self.stage,
            MaterializeStage::Io(_) | MaterializeStage::IoReady { .. }
        )
    }

    fn is_decode_in_flight(&self) -> bool {
        matches!(
            self.stage,
            MaterializeStage::Decode(_) | MaterializeStage::DecodeReady { .. }
        )
    }

    fn is_mesh_in_flight(&self) -> bool {
        matches!(
            self.stage,
            MaterializeStage::MeshBuild(_) | MaterializeStage::MeshBuildReady { .. }
        )
    }
}

/// Queue of in-flight IO/decode/mesh work and materialized results (terrain runtime only).
#[derive(Resource, Default)]
pub struct PendingChunkMaterializations {
    in_flight: Vec<InFlightMaterialization>,
    materialized: Vec<MaterializedChunkPending>,
}

impl PendingChunkMaterializations {
    pub fn materialized_chunks(&self) -> &[MaterializedChunkPending] {
        &self.materialized
    }

    pub fn materialized_len(&self) -> usize {
        self.materialized.len()
    }

    /// Alias for [`Self::materialized_len`] (perf / legacy naming).
    pub fn decoded_len(&self) -> usize {
        self.materialized.len()
    }

    pub fn in_flight_count(&self) -> usize {
        self.in_flight.len()
    }

    /// In-flight entries waiting on disk IO (includes IO-complete, decode-not-started).
    pub fn io_in_flight_count(&self) -> usize {
        self.in_flight
            .iter()
            .filter(|entry| entry.is_io_in_flight())
            .count()
    }

    /// In-flight entries waiting on decode (includes decode-complete, mesh-not-started).
    pub fn decode_in_flight_count(&self) -> usize {
        self.in_flight
            .iter()
            .filter(|entry| entry.is_decode_in_flight())
            .count()
    }

    /// In-flight entries waiting on async mesh build (includes build-complete, not yet queued).
    pub fn mesh_build_in_flight_count(&self) -> usize {
        self.in_flight
            .iter()
            .filter(|entry| entry.is_mesh_in_flight())
            .count()
    }

    pub fn unique_pipeline_chunk_count(&self) -> usize {
        let mut ids = HashSet::new();
        for entry in &self.in_flight {
            ids.insert(entry.chunk_id);
        }
        for entry in &self.materialized {
            ids.insert(entry.chunk_id);
        }
        ids.len()
    }

    pub fn has_pipeline_for(&self, chunk_id: ChunkId) -> bool {
        self.in_flight
            .iter()
            .any(|entry| entry.chunk_id == chunk_id)
            || self
                .materialized
                .iter()
                .any(|entry| entry.chunk_id == chunk_id)
    }

    /// Start IO for `chunk_id` if no pipeline entry exists yet.
    pub fn try_start_io(
        &mut self,
        chunk_id: ChunkId,
        generation: u64,
        path: PathBuf,
        albedo_path: Option<PathBuf>,
    ) -> bool {
        if self.has_pipeline_for(chunk_id) {
            warn!(
                "duplicate IO request blocked for chunk ({}, {})",
                chunk_id.coord().x,
                chunk_id.coord().z
            );
            return false;
        }
        self.in_flight.push(InFlightMaterialization {
            chunk_id,
            generation,
            mesh_lod: None,
            albedo_path,
            stage: MaterializeStage::Io(spawn_chunk_io_task(path)),
        });
        true
    }

    pub fn remove(&mut self, chunk_id: ChunkId) {
        self.in_flight.retain(|entry| entry.chunk_id != chunk_id);
        self.materialized.retain(|entry| entry.chunk_id != chunk_id);
    }

    /// Canonical cleanup for residency tracker + pipeline queue.
    pub fn discard_chunk_state(
        &mut self,
        residency: &mut ChunkResidencyTracker,
        chunk_id: ChunkId,
        kind: ChunkDiscardKind,
    ) {
        discard_chunk_residency(residency, chunk_id, kind);
        if matches!(kind, ChunkDiscardKind::Revoked) {
            self.remove(chunk_id);
        }
    }

    /// Drop all loading and pipeline work outside residency rings.
    pub fn discard_outside_residency_sets(
        &mut self,
        residency: &mut ChunkResidencyTracker,
        keep_resident: &HashSet<ChunkCoord>,
        desired_load: &HashSet<ChunkCoord>,
    ) {
        let mut revoke = HashSet::new();

        for (chunk_id, _) in residency.loading_chunk_ids() {
            if chunk_outside_residency_sets(chunk_id.coord(), keep_resident, desired_load) {
                revoke.insert(chunk_id);
            }
        }
        for entry in &self.in_flight {
            if chunk_outside_residency_sets(entry.chunk_id.coord(), keep_resident, desired_load) {
                revoke.insert(entry.chunk_id);
            }
        }
        for entry in &self.materialized {
            if chunk_outside_residency_sets(entry.chunk_id.coord(), keep_resident, desired_load) {
                revoke.insert(entry.chunk_id);
            }
        }

        for chunk_id in revoke {
            self.discard_chunk_state(residency, chunk_id, ChunkDiscardKind::Revoked);
        }
    }

    fn reject_in_flight_completion(
        residency: &mut ChunkResidencyTracker,
        chunk_id: ChunkId,
        generation: u64,
    ) {
        discard_chunk_residency(
            residency,
            chunk_id,
            ChunkDiscardKind::RejectedCompletion { generation },
        );
    }

    /// Advance IO → decode → async mesh build; queue materialized results for apply.
    pub fn poll_in_flight(
        &mut self,
        residency: &mut ChunkResidencyTracker,
        keep_resident: &HashSet<ChunkCoord>,
        max_decode_per_frame: usize,
        vertical_scale: f32,
        focus_chunk: ChunkCoord,
        lod_settings: &TerrainLodSettings,
        stats: &mut MaterializePollStats,
    ) {
        self.in_flight
            .sort_by_key(|entry| (entry.chunk_id.coord().z, entry.chunk_id.coord().x));

        let mut next = Vec::with_capacity(self.in_flight.len());
        let mut completed: Vec<(ChunkId, u64, ChunkData, Option<ChunkAlbedoGrid>, Mesh, ChunkLod, f32)> =
            Vec::new();
        let budget = if max_decode_per_frame == 0 {
            usize::MAX
        } else {
            max_decode_per_frame
        };
        let mut decode_starts = 0usize;
        let mut mesh_starts = 0usize;
        let mut mesh_stores = 0usize;

        for mut entry in self.in_flight.drain(..) {
            match entry.stage {
                MaterializeStage::MeshBuildReady {
                    data,
                    albedo,
                    mesh,
                    async_mesh_build_ms,
                } => {
                    if !materialized_result_may_store(
                        residency,
                        entry.chunk_id,
                        entry.generation,
                        keep_resident,
                    ) {
                        Self::reject_in_flight_completion(
                            residency,
                            entry.chunk_id,
                            entry.generation,
                        );
                        continue;
                    }

                    if mesh_stores < budget {
                        mesh_stores += 1;
                        let lod = entry.mesh_lod.expect("mesh_lod set before store");
                        completed.push((
                            entry.chunk_id,
                            entry.generation,
                            data,
                            albedo,
                            mesh,
                            lod,
                            async_mesh_build_ms,
                        ));
                    } else {
                        entry.stage = MaterializeStage::MeshBuildReady {
                            data,
                            albedo,
                            mesh,
                            async_mesh_build_ms,
                        };
                        next.push(entry);
                    }
                }
                MaterializeStage::DecodeReady { data } => {
                    if !materialized_result_may_store(
                        residency,
                        entry.chunk_id,
                        entry.generation,
                        keep_resident,
                    ) {
                        Self::reject_in_flight_completion(
                            residency,
                            entry.chunk_id,
                            entry.generation,
                        );
                        continue;
                    }

                    if mesh_starts < budget {
                        mesh_starts += 1;
                        let lod =
                            desired_lod(focus_chunk, entry.chunk_id.coord(), lod_settings);
                        entry.mesh_lod = Some(lod);
                        entry.stage = MaterializeStage::MeshBuild(spawn_chunk_mesh_build_task(
                            data,
                            entry.albedo_path.clone(),
                            vertical_scale,
                            lod,
                            AlbedoFallback::default(),
                        ));
                        next.push(entry);
                    } else {
                        entry.stage = MaterializeStage::DecodeReady { data };
                        next.push(entry);
                    }
                }
                MaterializeStage::Io(mut task) => {
                    if !task.is_finished() {
                        entry.stage = MaterializeStage::Io(task);
                        next.push(entry);
                        continue;
                    }

                    let raw = match bevy::tasks::block_on(&mut task) {
                        Ok(text) => text,
                        Err(err) => {
                            bevy::log::error!(
                                "chunk IO failed ({}, {}): {err}",
                                entry.chunk_id.coord().x,
                                entry.chunk_id.coord().z
                            );
                            Self::reject_in_flight_completion(
                                residency,
                                entry.chunk_id,
                                entry.generation,
                            );
                            continue;
                        }
                    };

                    if !materialized_result_may_store(
                        residency,
                        entry.chunk_id,
                        entry.generation,
                        keep_resident,
                    ) {
                        Self::reject_in_flight_completion(
                            residency,
                            entry.chunk_id,
                            entry.generation,
                        );
                        continue;
                    }

                    if decode_starts < budget {
                        decode_starts += 1;
                        entry.stage =
                            MaterializeStage::Decode(spawn_chunk_decode_task(raw));
                    } else {
                        entry.stage = MaterializeStage::IoReady { raw };
                    }
                    next.push(entry);
                }
                MaterializeStage::IoReady { raw } => {
                    if !materialized_result_may_store(
                        residency,
                        entry.chunk_id,
                        entry.generation,
                        keep_resident,
                    ) {
                        Self::reject_in_flight_completion(
                            residency,
                            entry.chunk_id,
                            entry.generation,
                        );
                        continue;
                    }

                    if decode_starts < budget {
                        decode_starts += 1;
                        entry.stage =
                            MaterializeStage::Decode(spawn_chunk_decode_task(raw));
                        next.push(entry);
                    } else {
                        entry.stage = MaterializeStage::IoReady { raw };
                        next.push(entry);
                    }
                }
                MaterializeStage::Decode(mut task) => {
                    if !task.is_finished() {
                        entry.stage = MaterializeStage::Decode(task);
                        next.push(entry);
                        continue;
                    }

                    match bevy::tasks::block_on(&mut task) {
                        Ok((id, data)) => {
                            if id != entry.chunk_id {
                                bevy::log::error!(
                                    "chunk decode id mismatch: expected ({}, {}), got ({}, {})",
                                    entry.chunk_id.coord().x,
                                    entry.chunk_id.coord().z,
                                    id.coord().x,
                                    id.coord().z
                                );
                                Self::reject_in_flight_completion(
                                    residency,
                                    entry.chunk_id,
                                    entry.generation,
                                );
                                continue;
                            }
                            if !materialized_result_may_store(
                                residency,
                                entry.chunk_id,
                                entry.generation,
                                keep_resident,
                            ) {
                                Self::reject_in_flight_completion(
                                    residency,
                                    entry.chunk_id,
                                    entry.generation,
                                );
                                continue;
                            }

                            if mesh_starts < budget {
                                mesh_starts += 1;
                                let lod = desired_lod(
                                    focus_chunk,
                                    entry.chunk_id.coord(),
                                    lod_settings,
                                );
                                entry.mesh_lod = Some(lod);
                                entry.stage = MaterializeStage::MeshBuild(
                                    spawn_chunk_mesh_build_task(
                                        data,
                                        entry.albedo_path.clone(),
                                        vertical_scale,
                                        lod,
                                        AlbedoFallback::default(),
                                    ),
                                );
                                next.push(entry);
                            } else {
                                entry.stage = MaterializeStage::DecodeReady { data };
                                next.push(entry);
                            }
                        }
                        Err(err) => {
                            bevy::log::error!(
                                "chunk decode failed ({}, {}): {err}",
                                entry.chunk_id.coord().x,
                                entry.chunk_id.coord().z
                            );
                            Self::reject_in_flight_completion(
                                residency,
                                entry.chunk_id,
                                entry.generation,
                            );
                        }
                    }
                }
                MaterializeStage::MeshBuild(mut task) => {
                    if !task.is_finished() {
                        entry.stage = MaterializeStage::MeshBuild(task);
                        next.push(entry);
                        continue;
                    }

                    let output = bevy::tasks::block_on(&mut task);
                    let async_mesh_build_ms = duration_to_ms(output.build_duration);
                    stats.async_mesh_build_ms += async_mesh_build_ms;
                    stats.async_mesh_builds_completed += 1;

                    if !materialized_result_may_store(
                        residency,
                        entry.chunk_id,
                        entry.generation,
                        keep_resident,
                    ) {
                        Self::reject_in_flight_completion(
                            residency,
                            entry.chunk_id,
                            entry.generation,
                        );
                        continue;
                    }

                    if mesh_stores < budget {
                        mesh_stores += 1;
                        let lod = entry.mesh_lod.expect("mesh_lod set before store");
                        completed.push((
                            entry.chunk_id,
                            entry.generation,
                            output.data,
                            output.albedo,
                            output.mesh,
                            lod,
                            async_mesh_build_ms,
                        ));
                    } else {
                        entry.stage = MaterializeStage::MeshBuildReady {
                            data: output.data,
                            albedo: output.albedo,
                            mesh: output.mesh,
                            async_mesh_build_ms,
                        };
                        next.push(entry);
                    }
                }
            }
        }

        self.in_flight = next;
        for (chunk_id, generation, data, albedo, mesh, lod, async_mesh_build_ms) in completed {
            self.store_materialized(
                chunk_id,
                generation,
                data,
                albedo,
                mesh,
                lod,
                async_mesh_build_ms,
            );
        }
        self.assert_pipeline_chunk_uniqueness();
    }

    #[cfg(test)]
    pub(crate) fn push_materialized_test_only(
        &mut self,
        chunk_id: ChunkId,
        generation: u64,
        data: ChunkData,
        vertical_scale: f32,
        lod: ChunkLod,
    ) {
        let mesh = build_materialized_mesh(&data, None, AlbedoFallback::default(), vertical_scale, lod);
        self.store_materialized(chunk_id, generation, data, None, mesh, lod, 0.0);
    }

    #[cfg(test)]
    pub(crate) fn push_decoded_test_only(
        &mut self,
        chunk_id: ChunkId,
        generation: u64,
        data: ChunkData,
    ) {
        self.push_materialized_test_only(chunk_id, generation, data, 1.0, ChunkLod::Full);
    }

    fn pipeline_has_unique_chunk_ids(&self) -> bool {
        let mut seen = HashSet::new();
        for entry in &self.in_flight {
            if !seen.insert(entry.chunk_id) {
                return false;
            }
        }
        for entry in &self.materialized {
            if !seen.insert(entry.chunk_id) {
                return false;
            }
        }
        true
    }

    fn assert_pipeline_chunk_uniqueness(&self) {
        if self.pipeline_has_unique_chunk_ids() {
            return;
        }

        let mut seen = HashSet::new();
        for entry in &self.in_flight {
            if !seen.insert(entry.chunk_id) {
                warn!(
                    "terrain pipeline duplicate chunk ({}, {}) in in_flight",
                    entry.chunk_id.coord().x,
                    entry.chunk_id.coord().z
                );
            }
        }
        for entry in &self.materialized {
            if !seen.insert(entry.chunk_id) {
                warn!(
                    "terrain pipeline duplicate chunk ({}, {}) in materialized queue",
                    entry.chunk_id.coord().x,
                    entry.chunk_id.coord().z
                );
            }
        }
        debug_assert!(self.pipeline_has_unique_chunk_ids());
    }

    fn store_materialized(
        &mut self,
        chunk_id: ChunkId,
        generation: u64,
        data: ChunkData,
        albedo: Option<ChunkAlbedoGrid>,
        mesh: Mesh,
        lod: ChunkLod,
        async_mesh_build_ms: f32,
    ) {
        self.materialized.retain(|entry| entry.chunk_id != chunk_id);
        self.materialized.push(MaterializedChunkPending {
            chunk_id,
            generation,
            data,
            albedo,
            mesh,
            lod,
            async_mesh_build_ms,
        });
    }

    /// Take materialized chunks ready for main-thread apply.
    pub fn take_materialized(&mut self) -> Vec<MaterializedChunkPending> {
        std::mem::take(&mut self.materialized)
    }

    /// Alias for [`Self::take_materialized`].
    pub fn take_decoded(&mut self) -> Vec<MaterializedChunkPending> {
        self.take_materialized()
    }

    /// Return materialized chunks that exceeded the apply budget to the queue.
    pub fn requeue_materialized(&mut self, entries: Vec<MaterializedChunkPending>) {
        if entries.is_empty() {
            return;
        }

        let mut sorted = entries;

        sorted.sort_by_key(|entry| (entry.chunk_id.coord().z, entry.chunk_id.coord().x));
        sorted.dedup_by_key(|entry| entry.chunk_id);

        for entry in sorted {
            if self
                .in_flight
                .iter()
                .any(|in_flight| in_flight.chunk_id == entry.chunk_id)
            {
                continue;
            }
            if self
                .materialized
                .iter()
                .any(|queued| queued.chunk_id == entry.chunk_id)
            {
                continue;
            }
            self.store_materialized(
                entry.chunk_id,
                entry.generation,
                entry.data,
                entry.albedo,
                entry.mesh,
                entry.lod,
                entry.async_mesh_build_ms,
            );
        }
        self.assert_pipeline_chunk_uniqueness();
    }

    /// Alias for [`Self::requeue_materialized`].
    pub fn requeue_decoded(&mut self, entries: Vec<MaterializedChunkPending>) {
        self.requeue_materialized(entries);
    }
}

fn duration_to_ms(d: Duration) -> f32 {
    d.as_secs_f32() * 1000.0
}

/// Returns true when a materialized result may be applied on the main thread.
pub fn materialized_result_may_apply(
    residency: &ChunkResidencyTracker,
    chunk_id: ChunkId,
    generation: u64,
    keep_resident: &HashSet<ChunkCoord>,
) -> bool {
    materialized_result_may_store(residency, chunk_id, generation, keep_resident)
}

/// Returns true when a pipeline result may be retained (generation + residency band).
pub(crate) fn materialized_result_may_store(
    residency: &ChunkResidencyTracker,
    chunk_id: ChunkId,
    generation: u64,
    keep_resident: &HashSet<ChunkCoord>,
) -> bool {
    residency.loading_generation_matches(chunk_id, generation)
        && keep_resident.contains(&chunk_id.coord())
}

/// Read chunk file text from disk (IO stage body).
pub(crate) fn read_chunk_file_text(path: &Path) -> Result<String, TerrainAssetError> {
    std::fs::read_to_string(path).map_err(|err| TerrainAssetError::Io {
        path: path.display().to_string(),
        message: err.to_string(),
    })
}

/// Decode chunk RON text (decode stage body; reuses [`decode_chunk`]).
pub(crate) fn decode_chunk_text(
    text: &str,
) -> Result<(ChunkId, ChunkData), TerrainAssetError> {
    decode_chunk(text)
}

/// IO stage: read chunk file from disk on [`IoTaskPool`].
pub fn spawn_chunk_io_task(path: PathBuf) -> ChunkIoTask {
    IoTaskPool::get().spawn(async move { read_chunk_file_text(&path) })
}

/// Decode stage: `decode_chunk` on [`AsyncComputeTaskPool`].
pub fn spawn_chunk_decode_task(raw: String) -> ChunkDecodeTask {
    AsyncComputeTaskPool::get().spawn(async move { decode_chunk_text(&raw) })
}

/// Mesh-build stage: pure mesh generation on [`AsyncComputeTaskPool`].
pub fn spawn_chunk_mesh_build_task(
    data: ChunkData,
    albedo_path: Option<PathBuf>,
    vertical_scale: f32,
    lod: ChunkLod,
    fallback: AlbedoFallback,
) -> ChunkMeshBuildTask {
    AsyncComputeTaskPool::get().spawn(async move {
        let start = std::time::Instant::now();
        let spe = data.heightfield.samples_per_edge();
        let albedo = match albedo_path.as_deref() {
            Some(path) => match load_albedo_sidecar_absolute(path, spe) {
                Ok(albedo) => albedo,
                Err(err) => {
                    bevy::log::error!("albedo sidecar load failed for {path:?}: {err}");
                    None
                }
            },
            None => None,
        };
        let mesh = build_materialized_mesh(&data, albedo.as_ref(), fallback, vertical_scale, lod);
        MeshBuildOutput {
            data,
            albedo,
            mesh,
            build_duration: start.elapsed(),
        }
    })
}

fn build_materialized_mesh(
    data: &ChunkData,
    albedo: Option<&ChunkAlbedoGrid>,
    fallback: AlbedoFallback,
    vertical_scale: f32,
    lod: ChunkLod,
) -> Mesh {
    build_chunk_mesh_scaled(
        &data.heightfield,
        lod,
        vertical_scale,
        &ChunkMeshSeamWeld::default(),
        albedo,
        fallback,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::asset::{CHUNK_FORMAT_VERSION, ChunkFile};
    use crate::world::ChunkCoord;
    use bevy::tasks::TaskPoolBuilder;
    use std::sync::Once;

    static TASK_POOLS: Once = Once::new();

    fn ensure_task_pools() {
        TASK_POOLS.call_once(|| {
            IoTaskPool::get_or_init(|| TaskPoolBuilder::new().num_threads(1).build());
            AsyncComputeTaskPool::get_or_init(|| TaskPoolBuilder::new().num_threads(1).build());
        });
    }

    fn sample_chunk_file_spe(x: i32, z: i32, samples_per_edge: u32) -> ChunkFile {
        let spe = samples_per_edge as usize;
        let spacing = 256.0 / (samples_per_edge - 1) as f32;
        let mut samples = Vec::new();
        for row in 0..spe {
            for col in 0..spe {
                samples.push((row * 10 + col) as f32);
            }
        }
        ChunkFile {
            version: CHUNK_FORMAT_VERSION,
            x,
            z,
            samples_per_edge,
            spacing_meters: spacing,
            samples,
            height_min: 0.0,
            height_max: 22.0,
        }
    }

    fn temp_chunk_path(x: i32, z: i32) -> PathBuf {
        temp_chunk_path_spe(x, z, 3)
    }

    fn temp_chunk_path_spe(x: i32, z: i32, samples_per_edge: u32) -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};

        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "chasma_mat_{}_{}_{x}_{z}.ron",
            std::process::id(),
            id
        ));
        std::fs::write(
            &path,
            ron::to_string(&sample_chunk_file_spe(x, z, samples_per_edge)).unwrap(),
        )
        .unwrap();
        path
    }

    #[test]
    fn io_read_produces_file_contents() {
        let path = temp_chunk_path(1, 2);
        let expected = std::fs::read_to_string(&path).unwrap();
        let actual = read_chunk_file_text(&path).unwrap();
        assert_eq!(actual, expected);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn io_task_produces_correct_file_contents() {
        ensure_task_pools();
        let path = temp_chunk_path(4, 5);
        let expected = std::fs::read_to_string(&path).unwrap();
        let mut task = spawn_chunk_io_task(path.clone());
        let actual = bevy::tasks::block_on(&mut task).unwrap();
        assert_eq!(actual, expected);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn decode_task_produces_valid_chunk_data() {
        ensure_task_pools();
        let path = temp_chunk_path(1, 2);
        let raw = read_chunk_file_text(&path).unwrap();
        let mut task = spawn_chunk_decode_task(raw);
        let (id, data) = bevy::tasks::block_on(&mut task).unwrap();
        assert_eq!(id, ChunkId::new(ChunkCoord::new(1, 2)));
        assert_eq!(data.heightfield.samples_per_edge(), 3);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn no_duplicate_io_tasks_per_chunk_id() {
        ensure_task_pools();
        let mut pending = PendingChunkMaterializations::default();
        let chunk_id = ChunkId::new(ChunkCoord::new(0, 0));
        let path = temp_chunk_path(0, 0);

        assert!(pending.try_start_io(chunk_id, 1, path.clone(), None));
        assert!(!pending.try_start_io(chunk_id, 2, path.clone(), None));
        assert_eq!(pending.in_flight_count(), 1);

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn cancellation_prevents_storing_result() {
        let mut residency = ChunkResidencyTracker::default();
        let chunk_id = ChunkId::new(ChunkCoord::new(2, 2));
        let generation = residency.begin_loading(chunk_id).unwrap();

        let mut keep = HashSet::new();
        keep.insert(ChunkCoord::new(2, 2));

        assert!(materialized_result_may_store(
            &residency,
            chunk_id,
            generation,
            &keep
        ));

        residency.cancel(chunk_id);

        assert!(!materialized_result_may_store(
            &residency,
            chunk_id,
            generation,
            &keep
        ));
    }

    #[test]
    fn outside_keep_resident_prevents_storing_result() {
        let mut residency = ChunkResidencyTracker::default();
        let chunk_id = ChunkId::new(ChunkCoord::new(5, 5));
        let generation = residency.begin_loading(chunk_id).unwrap();
        let keep = HashSet::new();

        assert!(!materialized_result_may_store(
            &residency,
            chunk_id,
            generation,
            &keep
        ));
    }

    #[test]
    fn generation_mismatch_prevents_storing_result() {
        let mut residency = ChunkResidencyTracker::default();
        let chunk_id = ChunkId::new(ChunkCoord::new(1, 1));
        let _first = residency.begin_loading(chunk_id).unwrap();
        residency.cancel(chunk_id);
        let second = residency.begin_loading(chunk_id).unwrap();

        let mut keep = HashSet::new();
        keep.insert(ChunkCoord::new(1, 1));

        assert!(!materialized_result_may_store(&residency, chunk_id, 0, &keep));
        assert!(materialized_result_may_store(&residency, chunk_id, second, &keep));
    }

    #[test]
    fn decode_queue_does_not_overflow_apply_stage() {
        let mut pending = PendingChunkMaterializations::default();
        let mut residency = ChunkResidencyTracker::default();

        for i in 0..6 {
            let chunk_id = ChunkId::new(ChunkCoord::new(i, 0));
            let generation = residency.begin_loading(chunk_id).unwrap();
            pending.push_decoded_test_only(chunk_id, generation, sample_chunk_data(i));
        }

        let budget = 2;
        let mut batch = pending.take_decoded();
        batch.sort_by_key(|entry| (entry.chunk_id.coord().z, entry.chunk_id.coord().x));
        let remainder = if batch.len() > budget {
            batch.split_off(budget)
        } else {
            Vec::new()
        };

        assert_eq!(batch.len(), budget);
        pending.requeue_decoded(remainder);
        assert_eq!(pending.decoded_len(), 4);
        assert_eq!(pending.unique_pipeline_chunk_count(), 4);
    }

    fn sample_chunk_data(seed: i32) -> ChunkData {
        let base = seed as f32;
        ChunkData::new(
            crate::world::Heightfield::from_samples(3, 128.0, vec![base; 9]).unwrap(),
            Vec::new(),
        )
    }

    fn poll_until_materialized(
        pending: &mut PendingChunkMaterializations,
        residency: &mut ChunkResidencyTracker,
        keep: &HashSet<ChunkCoord>,
        focus: ChunkCoord,
    ) {
        let settings = TerrainLodSettings::default();
        let mut stats = MaterializePollStats::default();
        for _ in 0..64 {
            pending.poll_in_flight(
                residency,
                keep,
                16,
                1.0,
                focus,
                &settings,
                &mut stats,
            );
            if pending.materialized_len() > 0 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    #[test]
    fn poll_decode_failure_clears_loading_state() {
        ensure_task_pools();
        let mut pending = PendingChunkMaterializations::default();
        let mut residency = ChunkResidencyTracker::default();
        let chunk_id = ChunkId::new(ChunkCoord::new(9, 9));
        let generation = residency.begin_loading(chunk_id).unwrap();
        let path = std::env::temp_dir().join(format!(
            "chasma_bad_decode_{}.ron",
            std::process::id()
        ));
        std::fs::write(&path, "not valid chunk ron").unwrap();
        assert!(pending.try_start_io(chunk_id, generation, path.clone(), None));

        let mut keep = HashSet::new();
        keep.insert(chunk_id.coord());

        for _ in 0..32 {
            let mut stats = MaterializePollStats::default();
            let settings = TerrainLodSettings::default();
            pending.poll_in_flight(
                &mut residency,
                &keep,
                16,
                1.0,
                chunk_id.coord(),
                &settings,
                &mut stats,
            );
            if !residency.is_loading(chunk_id) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }

        assert_eq!(
            residency.state(chunk_id),
            super::super::residency::ChunkResidencyState::Absent
        );
        assert!(!pending.has_pipeline_for(chunk_id));
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn discard_outside_residency_sets_clears_pipeline_and_loading() {
        let mut pending = PendingChunkMaterializations::default();
        let mut residency = ChunkResidencyTracker::default();
        let chunk_id = ChunkId::new(ChunkCoord::new(3, 3));
        let generation = residency.begin_loading(chunk_id).unwrap();
        pending.push_decoded_test_only(chunk_id, generation, sample_chunk_data(0));

        let keep = HashSet::new();
        let desired = HashSet::new();
        pending.discard_outside_residency_sets(&mut residency, &keep, &desired);

        assert!(!residency.is_loading(chunk_id));
        assert_eq!(pending.decoded_len(), 0);
    }

    #[test]
    fn mesh_build_task_produces_chunk_data_and_mesh() {
        ensure_task_pools();
        let data = sample_chunk_data(0);
        let mut task = spawn_chunk_mesh_build_task(
            data.clone(),
            None,
            1.0,
            ChunkLod::Full,
            AlbedoFallback::default(),
        );
        let output = bevy::tasks::block_on(&mut task);
        assert_eq!(output.data.heightfield.samples_per_edge(), 3);
        assert!(output.mesh.contains_attribute(Mesh::ATTRIBUTE_POSITION));
        assert!(output.build_duration > Duration::ZERO);
    }

    #[test]
    fn async_pipeline_produces_materialized_chunk_with_mesh() {
        ensure_task_pools();
        let mut pending = PendingChunkMaterializations::default();
        let mut residency = ChunkResidencyTracker::default();
        let chunk_id = ChunkId::new(ChunkCoord::new(1, 2));
        let generation = residency.begin_loading(chunk_id).unwrap();
        let path = temp_chunk_path(1, 2);
        assert!(pending.try_start_io(chunk_id, generation, path.clone(), None));

        let mut keep = HashSet::new();
        keep.insert(chunk_id.coord());
        let focus = ChunkCoord::new(1, 2);
        poll_until_materialized(&mut pending, &mut residency, &keep, focus);

        assert_eq!(pending.materialized_len(), 1);
        let entry = &pending.materialized_chunks()[0];
        assert_eq!(entry.chunk_id, chunk_id);
        assert_eq!(entry.lod, ChunkLod::Full);
        assert_eq!(entry.data.heightfield.samples_per_edge(), 3);
        assert!(entry.mesh.contains_attribute(Mesh::ATTRIBUTE_POSITION));
        assert!(entry.async_mesh_build_ms >= 0.0);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn async_pipeline_materializes_lower_lod_when_far_from_focus() {
        ensure_task_pools();
        let mut pending = PendingChunkMaterializations::default();
        let mut residency = ChunkResidencyTracker::default();
        let chunk_id = ChunkId::new(ChunkCoord::new(1, 0));
        let generation = residency.begin_loading(chunk_id).unwrap();
        let path = temp_chunk_path(1, 0);
        assert!(pending.try_start_io(chunk_id, generation, path.clone(), None));

        let mut keep = HashSet::new();
        keep.insert(chunk_id.coord());
        let focus = ChunkCoord::new(0, 0);
        poll_until_materialized(&mut pending, &mut residency, &keep, focus);

        assert_eq!(pending.materialized_len(), 1);
        assert_eq!(pending.materialized_chunks()[0].lod, ChunkLod::Half);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn stale_mesh_result_is_discarded() {
        ensure_task_pools();
        let mut pending = PendingChunkMaterializations::default();
        let mut residency = ChunkResidencyTracker::default();
        let chunk_id = ChunkId::new(ChunkCoord::new(3, 4));
        let stale_generation = residency.begin_loading(chunk_id).unwrap();
        let path = temp_chunk_path(3, 4);
        assert!(pending.try_start_io(chunk_id, stale_generation, path.clone(), None));

        let mut keep = HashSet::new();
        keep.insert(chunk_id.coord());
        let focus = chunk_id.coord();
        let settings = TerrainLodSettings::default();

        for _ in 0..32 {
            let mut stats = MaterializePollStats::default();
            pending.poll_in_flight(
                &mut residency,
                &keep,
                16,
                1.0,
                focus,
                &settings,
                &mut stats,
            );
            if pending.mesh_build_in_flight_count() > 0 || pending.materialized_len() > 0 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        residency.cancel(chunk_id);
        let _current = residency.begin_loading(chunk_id).unwrap();

        for _ in 0..32 {
            let mut stats = MaterializePollStats::default();
            pending.poll_in_flight(
                &mut residency,
                &keep,
                16,
                1.0,
                focus,
                &settings,
                &mut stats,
            );
            if !pending.has_pipeline_for(chunk_id) || pending.materialized_len() > 0 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        assert!(
            pending
                .materialized_chunks()
                .iter()
                .all(|entry| entry.generation != stale_generation),
            "stale mesh result must not remain queued"
        );
        std::fs::remove_file(path).ok();
    }
}
