//! Async chunk materialization pipeline (ADR-012 Phase 2B.5).
//!
//! Step 2: IO + decode off the main thread. Mesh build and apply (WorldData +
//! ECS spawn) land in later steps.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, IoTaskPool, Task};

use crate::world::{ChunkCoord, ChunkData, ChunkId};

use super::asset::TerrainAssetError;
use super::decode::decode_chunk;
use super::streaming::chunk_outside_residency_sets;
use super::residency::{ChunkDiscardKind, ChunkResidencyTracker, discard_chunk_residency};

/// Raw chunk file text read off the main thread.
pub type ChunkIoTask = Task<Result<String, TerrainAssetError>>;

/// Decoded chunk payload produced on the compute pool.
pub type ChunkDecodeTask = Task<Result<(ChunkId, ChunkData), TerrainAssetError>>;

/// Mesh-build phase (step 3+ — not wired yet).
#[allow(dead_code)]
pub type ChunkMeshBuildTask = Task<()>;

/// Decoded chunk awaiting apply on the main thread (step 3+).
#[derive(Debug)]
pub struct DecodedChunkPending {
    pub chunk_id: ChunkId,
    pub generation: u64,
    pub data: ChunkData,
}

enum MaterializeStage {
    Io(ChunkIoTask),
    IoReady { raw: String },
    Decode(ChunkDecodeTask),
    DecodeReady { data: ChunkData },
}

struct InFlightMaterialization {
    chunk_id: ChunkId,
    generation: u64,
    stage: MaterializeStage,
}

/// Queue of in-flight IO/decode work and decoded results (terrain runtime only).
#[derive(Resource, Default)]
pub struct PendingChunkMaterializations {
    in_flight: Vec<InFlightMaterialization>,
    decoded: Vec<DecodedChunkPending>,
}

impl PendingChunkMaterializations {
    pub fn decoded_chunks(&self) -> &[DecodedChunkPending] {
        &self.decoded
    }

    pub fn decoded_len(&self) -> usize {
        self.decoded.len()
    }

    pub fn in_flight_count(&self) -> usize {
        self.in_flight.len()
    }

    pub fn unique_pipeline_chunk_count(&self) -> usize {
        let mut ids = HashSet::new();
        for entry in &self.in_flight {
            ids.insert(entry.chunk_id);
        }
        for entry in &self.decoded {
            ids.insert(entry.chunk_id);
        }
        ids.len()
    }

    pub fn has_pipeline_for(&self, chunk_id: ChunkId) -> bool {
        self.in_flight
            .iter()
            .any(|entry| entry.chunk_id == chunk_id)
            || self
                .decoded
                .iter()
                .any(|entry| entry.chunk_id == chunk_id)
    }

    /// Start IO for `chunk_id` if no pipeline entry exists yet.
    pub fn try_start_io(
        &mut self,
        chunk_id: ChunkId,
        generation: u64,
        path: PathBuf,
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
            stage: MaterializeStage::Io(spawn_chunk_io_task(path)),
        });
        true
    }

    pub fn remove(&mut self, chunk_id: ChunkId) {
        self.in_flight.retain(|entry| entry.chunk_id != chunk_id);
        self.decoded.retain(|entry| entry.chunk_id != chunk_id);
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
        for entry in &self.decoded {
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

    /// Advance IO → decode and collect finished decode results (no apply).
    pub fn poll_in_flight(
        &mut self,
        residency: &mut ChunkResidencyTracker,
        keep_resident: &HashSet<ChunkCoord>,
        max_decode_per_frame: usize,
    ) {
        self.in_flight
            .sort_by_key(|entry| (entry.chunk_id.coord().z, entry.chunk_id.coord().x));

        let mut next = Vec::with_capacity(self.in_flight.len());
        let mut completed = Vec::new();
        let decode_budget = if max_decode_per_frame == 0 {
            usize::MAX
        } else {
            max_decode_per_frame
        };
        let mut decode_starts = 0usize;
        let mut decode_stores = 0usize;

        for mut entry in self.in_flight.drain(..) {
            match entry.stage {
                MaterializeStage::DecodeReady { data } => {
                    if !decoded_result_may_store(
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

                    if decode_stores < decode_budget {
                        decode_stores += 1;
                        completed.push((entry.chunk_id, entry.generation, data));
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

                    if !decoded_result_may_store(
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

                    if decode_starts < decode_budget {
                        decode_starts += 1;
                        entry.stage =
                            MaterializeStage::Decode(spawn_chunk_decode_task(raw));
                    } else {
                        entry.stage = MaterializeStage::IoReady { raw };
                    }
                    next.push(entry);
                }
                MaterializeStage::IoReady { raw } => {
                    if !decoded_result_may_store(
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

                    if decode_starts < decode_budget {
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
                            if decoded_result_may_store(
                                residency,
                                entry.chunk_id,
                                entry.generation,
                                keep_resident,
                            ) {
                                if decode_stores < decode_budget {
                                    decode_stores += 1;
                                    completed.push((
                                        entry.chunk_id,
                                        entry.generation,
                                        data,
                                    ));
                                } else {
                                    entry.stage = MaterializeStage::DecodeReady { data };
                                    next.push(entry);
                                }
                            } else {
                                Self::reject_in_flight_completion(
                                    residency,
                                    entry.chunk_id,
                                    entry.generation,
                                );
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
            }
        }

        self.in_flight = next;
        for (chunk_id, generation, data) in completed {
            self.store_decoded(chunk_id, generation, data);
        }
        self.assert_pipeline_chunk_uniqueness();
    }

    #[cfg(test)]
    pub(crate) fn push_decoded_test_only(
        &mut self,
        chunk_id: ChunkId,
        generation: u64,
        data: ChunkData,
    ) {
        self.store_decoded(chunk_id, generation, data);
    }

    fn pipeline_has_unique_chunk_ids(&self) -> bool {
        let mut seen = HashSet::new();
        for entry in &self.in_flight {
            if !seen.insert(entry.chunk_id) {
                return false;
            }
        }
        for entry in &self.decoded {
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
        for entry in &self.decoded {
            if !seen.insert(entry.chunk_id) {
                warn!(
                    "terrain pipeline duplicate chunk ({}, {}) in decoded queue",
                    entry.chunk_id.coord().x,
                    entry.chunk_id.coord().z
                );
            }
        }
        debug_assert!(self.pipeline_has_unique_chunk_ids());
    }

    fn store_decoded(&mut self, chunk_id: ChunkId, generation: u64, data: ChunkData) {
        self.decoded.retain(|entry| entry.chunk_id != chunk_id);
        self.decoded.push(DecodedChunkPending {
            chunk_id,
            generation,
            data,
        });
    }

    /// Take decoded chunks ready for apply (step 3).
    pub fn take_decoded(&mut self) -> Vec<DecodedChunkPending> {
        std::mem::take(&mut self.decoded)
    }

    /// Return decoded chunks that exceeded the apply budget to the queue.
    pub fn requeue_decoded(&mut self, entries: Vec<DecodedChunkPending>) {
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
                .decoded
                .iter()
                .any(|decoded| decoded.chunk_id == entry.chunk_id)
            {
                continue;
            }
            self.store_decoded(entry.chunk_id, entry.generation, entry.data);
        }
        self.assert_pipeline_chunk_uniqueness();
    }
}

/// Returns true when a decoded result may be applied on the main thread.
pub fn decoded_result_may_apply(
    residency: &ChunkResidencyTracker,
    chunk_id: ChunkId,
    generation: u64,
    keep_resident: &HashSet<ChunkCoord>,
) -> bool {
    decoded_result_may_store(residency, chunk_id, generation, keep_resident)
}

/// Returns true when a decoded result may be retained (generation + residency band).
pub(crate) fn decoded_result_may_store(
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

/// Mesh-build stage skeleton (step 3+).
#[allow(dead_code)]
pub fn spawn_chunk_mesh_build_task() -> ChunkMeshBuildTask {
    AsyncComputeTaskPool::get().spawn(async {
        unimplemented!("mesh build phase lands in step 3")
    })
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

    fn sample_chunk_file(x: i32, z: i32) -> ChunkFile {
        let mut samples = Vec::new();
        for row in 0..3 {
            for col in 0..3 {
                samples.push((row * 10 + col) as f32);
            }
        }
        ChunkFile {
            version: CHUNK_FORMAT_VERSION,
            x,
            z,
            samples_per_edge: 3,
            spacing_meters: 128.0,
            samples,
            height_min: 0.0,
            height_max: 22.0,
        }
    }

    fn temp_chunk_path(x: i32, z: i32) -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};

        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "chasma_mat_{}_{}_{x}_{z}.ron",
            std::process::id(),
            id
        ));
        std::fs::write(&path, ron::to_string(&sample_chunk_file(x, z)).unwrap()).unwrap();
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

        assert!(pending.try_start_io(chunk_id, 1, path.clone()));
        assert!(!pending.try_start_io(chunk_id, 2, path.clone()));
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

        assert!(decoded_result_may_store(
            &residency,
            chunk_id,
            generation,
            &keep
        ));

        residency.cancel(chunk_id);

        assert!(!decoded_result_may_store(
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

        assert!(!decoded_result_may_store(
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

        assert!(!decoded_result_may_store(&residency, chunk_id, 0, &keep));
        assert!(decoded_result_may_store(&residency, chunk_id, second, &keep));
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
        assert!(pending.try_start_io(chunk_id, generation, path.clone()));

        let mut keep = HashSet::new();
        keep.insert(chunk_id.coord());

        for _ in 0..32 {
            pending.poll_in_flight(&mut residency, &keep, 16);
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
}
