//! Dev-only terrain streaming performance instrumentation.
//!
//! Measures poll/apply/mesh/spawn costs without changing streaming architecture.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::time::{Duration, Instant, SystemTime};

use bevy::prelude::*;

use crate::world::ChunkCoord;

use super::mesh::{ChunkLod, ChunkMeshGeometry};

/// Default path for terrain streaming perf logs (relative to the process working directory).
pub const DEFAULT_PERF_LOG_PATH: &str = "logs/terrain_streaming_perf.log";

/// Tunable reporting thresholds and output targets (dev preview opts in at startup).
#[derive(Debug, Clone, Resource, Reflect)]
#[reflect(Resource)]
pub struct TerrainStreamingPerfSettings {
    /// Master switch: when false, samples are not collected or written anywhere.
    pub enabled: bool,
    /// Emit matching samples to the Bevy console (`info!`).
    pub log_to_console: bool,
    /// Append matching samples to [`Self::log_file_path`].
    pub log_to_file: bool,
    /// Log file path, relative to the process working directory (usually project root).
    pub log_file_path: String,
    /// Log when `poll_ms + apply_ms` exceeds this value.
    pub frame_time_threshold_ms: f32,
    /// Log when mesh build time in an apply tick exceeds this value.
    pub mesh_build_threshold_ms: f32,
    /// Periodic summary interval while streaming work is active.
    pub summary_interval_secs: f32,
}

impl Default for TerrainStreamingPerfSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            log_to_console: false,
            log_to_file: false,
            log_file_path: DEFAULT_PERF_LOG_PATH.to_string(),
            frame_time_threshold_ms: 4.0,
            mesh_build_threshold_ms: 2.0,
            summary_interval_secs: 1.0,
        }
    }
}

/// Why a terrain mesh was built or applied this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeshBuildReason {
    /// Async initial materialization (`spawn_chunk_mesh_build_task`).
    InitialMaterialize,
    /// Async resident LOD rebuild (display-driven cache miss).
    LodImmediate,
    /// Async predictive LOD prefetch.
    LodPrefetch,
    /// Main-thread apply of a prebuilt materialization mesh.
    AppliedPrebuilt,
}

/// Per-mesh record for detailed hitch logs.
#[derive(Debug, Clone)]
pub struct MeshBuildLogEntry {
    pub coord: ChunkCoord,
    pub lod: ChunkLod,
    pub reason: MeshBuildReason,
    pub build_ms: f32,
    pub geometry: ChunkMeshGeometry,
}

/// Completed async mesh builds by LOD level.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LodBuildCounts {
    pub full: usize,
    pub half: usize,
    pub quarter: usize,
    pub eighth: usize,
}

impl LodBuildCounts {
    pub fn record(&mut self, lod: ChunkLod) {
        match lod {
            ChunkLod::Full => self.full += 1,
            ChunkLod::Half => self.half += 1,
            ChunkLod::Quarter => self.quarter += 1,
            ChunkLod::Eighth => self.eighth += 1,
        }
    }

    pub fn total(&self) -> usize {
        self.full + self.half + self.quarter + self.eighth
    }
}

/// Per-frame streaming measurements (one terrain streaming chain tick).
#[derive(Debug, Clone, Default, Reflect)]
pub struct TerrainStreamingFrameSample {
    pub io_in_flight: usize,
    pub decode_in_flight: usize,
    pub mesh_build_in_flight: usize,
    pub materialized_queue_len: usize,
    pub chunks_applied: usize,
    pub chunks_unloaded: usize,
    pub mesh_build_count: usize,
    pub mesh_build_avg_ms: f32,
    pub mesh_build_max_ms: f32,
    pub total_vertices: usize,
    pub total_indices: usize,
    pub total_triangles: usize,
    pub avg_vertices_per_mesh: usize,
    pub avg_triangles_per_mesh: usize,
    pub lod_prefetch_requests: usize,
    pub lod_prefetch_hits: usize,
    pub lod_prefetch_misses: usize,
    pub lod_builds_started_from_prefetch: usize,
    /// Async materialization mesh builds completed this frame.
    pub materialize_async_build_ms: f32,
    pub materialize_async_builds_completed: usize,
    /// Async resident LOD mesh builds completed this frame.
    pub lod_async_build_ms: f32,
    pub lod_async_builds_completed: usize,
    #[reflect(ignore)]
    pub lod_build_counts: LodBuildCounts,
    pub poll_ms: f32,
    pub apply_ms: f32,
    pub prebuilt_meshes_applied: usize,
    pub mesh_assets_ms: f32,
    pub spawn_ms: f32,
    #[reflect(ignore)]
    pub mesh_build_log: Vec<MeshBuildLogEntry>,
}

pub fn lod_label(lod: ChunkLod) -> &'static str {
    match lod {
        ChunkLod::Full => "Full",
        ChunkLod::Half => "Half",
        ChunkLod::Quarter => "Quarter",
        ChunkLod::Eighth => "Eighth",
    }
}

pub fn mesh_build_reason_label(reason: MeshBuildReason) -> &'static str {
    match reason {
        MeshBuildReason::InitialMaterialize => "InitialMaterialize",
        MeshBuildReason::LodImmediate => "LodImmediate",
        MeshBuildReason::LodPrefetch => "LodPrefetch",
        MeshBuildReason::AppliedPrebuilt => "AppliedPrebuilt",
    }
}

/// Record one mesh build/apply event on the current frame sample.
pub fn record_mesh_build_event(
    frame: &mut TerrainStreamingFrameSample,
    coord: ChunkCoord,
    lod: ChunkLod,
    reason: MeshBuildReason,
    build_ms: f32,
    geometry: ChunkMeshGeometry,
) {
    frame.lod_build_counts.record(lod);

    match reason {
        MeshBuildReason::InitialMaterialize => {
            frame.materialize_async_builds_completed += 1;
            frame.materialize_async_build_ms += build_ms;
        }
        MeshBuildReason::LodImmediate | MeshBuildReason::LodPrefetch => {
            frame.lod_async_builds_completed += 1;
            frame.lod_async_build_ms += build_ms;
        }
        MeshBuildReason::AppliedPrebuilt => {}
    }

    frame.mesh_build_log.push(MeshBuildLogEntry {
        coord,
        lod,
        reason,
        build_ms,
        geometry,
    });
}

impl TerrainStreamingFrameSample {
    pub fn total_streaming_ms(&self) -> f32 {
        self.poll_ms + self.apply_ms
    }

    pub fn has_activity(&self) -> bool {
        self.chunks_applied > 0
            || self.chunks_unloaded > 0
            || self.io_in_flight > 0
            || self.decode_in_flight > 0
            || self.mesh_build_in_flight > 0
            || self.materialized_queue_len > 0
            || self.materialize_async_builds_completed > 0
            || self.lod_async_builds_completed > 0
            || !self.mesh_build_log.is_empty()
    }

    pub fn refresh_geometry_averages(&mut self) {
        let built = self.mesh_build_log.len();
        self.prebuilt_meshes_applied = self
            .mesh_build_log
            .iter()
            .filter(|e| e.reason == MeshBuildReason::AppliedPrebuilt)
            .count();
        self.total_vertices = self
            .mesh_build_log
            .iter()
            .map(|e| e.geometry.vertices)
            .sum();
        self.total_indices = self
            .mesh_build_log
            .iter()
            .map(|e| e.geometry.indices)
            .sum();
        self.total_triangles = self
            .mesh_build_log
            .iter()
            .map(|e| e.geometry.triangles)
            .sum();
        self.avg_vertices_per_mesh = if built > 0 {
            self.total_vertices / built
        } else {
            0
        };
        self.avg_triangles_per_mesh = if built > 0 {
            self.total_triangles / built
        } else {
            0
        };
        self.mesh_build_count = built;
        if built > 0 {
            let async_ms: f32 = self
                .mesh_build_log
                .iter()
                .filter(|e| e.reason != MeshBuildReason::AppliedPrebuilt)
                .map(|e| e.build_ms)
                .sum();
            let async_count = self
                .mesh_build_log
                .iter()
                .filter(|e| e.reason != MeshBuildReason::AppliedPrebuilt)
                .count();
            if async_count > 0 {
                self.mesh_build_avg_ms = async_ms / async_count as f32;
                self.mesh_build_max_ms = self
                    .mesh_build_log
                    .iter()
                    .filter(|e| e.reason != MeshBuildReason::AppliedPrebuilt)
                    .map(|e| e.build_ms)
                    .fold(0.0_f32, f32::max);
            }
        }
    }
}

/// Latest completed sample (inspectable in the editor).
#[derive(Debug, Clone, Resource, Reflect, Default)]
#[reflect(Resource)]
pub struct TerrainStreamingPerfLatest(pub TerrainStreamingFrameSample);

/// Accumulates asset insert and spawn timings during one apply pass.
#[derive(Debug, Default)]
pub struct TerrainStreamingPerfRecorder {
    mesh_assets: Duration,
    spawn: Duration,
    prebuilt_apply_log: Vec<(ChunkCoord, ChunkLod, ChunkMeshGeometry)>,
}

impl TerrainStreamingPerfRecorder {
    pub fn record_prebuilt_mesh_applied(
        &mut self,
        coord: ChunkCoord,
        lod: ChunkLod,
        geometry: ChunkMeshGeometry,
    ) {
        self.prebuilt_apply_log.push((coord, lod, geometry));
    }

    pub fn record_mesh_assets(&mut self, elapsed: Duration) {
        self.mesh_assets += elapsed;
    }

    pub fn record_spawn(&mut self, elapsed: Duration) {
        self.spawn += elapsed;
    }

    pub fn finish_into(&self, frame: &mut TerrainStreamingFrameSample) {
        frame.mesh_assets_ms = duration_to_ms(self.mesh_assets);
        frame.spawn_ms = duration_to_ms(self.spawn);
        for (coord, lod, geometry) in &self.prebuilt_apply_log {
            record_mesh_build_event(
                frame,
                *coord,
                *lod,
                MeshBuildReason::AppliedPrebuilt,
                0.0,
                *geometry,
            );
        }
        frame.refresh_geometry_averages();
    }
}

#[derive(Resource, Default)]
pub struct TerrainStreamingPerfState {
    frame: TerrainStreamingFrameSample,
    last_summary: Option<Instant>,
}

/// Tracks which on-disk log file has received the current session header.
#[derive(Resource, Default)]
pub struct TerrainStreamingPerfFileLog {
    active_path: Option<String>,
}

impl TerrainStreamingPerfFileLog {
    pub fn append_block(
        &mut self,
        settings: &TerrainStreamingPerfSettings,
        block: &str,
    ) {
        if !settings.log_to_file {
            return;
        }

        let path = settings.log_file_path.as_str();
        if self.active_path.as_deref() != Some(path) {
            self.active_path = Some(settings.log_file_path.clone());
            if let Err(err) = write_session_header(path) {
                warn!("terrain perf log: failed to open {path}: {err}");
                return;
            }
        }

        if let Err(err) = append_log_block(path, block) {
            warn!("terrain perf log: failed to write {path}: {err}");
        }
    }
}

fn write_session_header(path: &str) -> std::io::Result<()> {
    if let Some(parent) = Path::new(path).parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }

    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    let started = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    writeln!(file, "# terrain streaming perf session started_at={started}")?;
    writeln!(
        file,
        "# multi-line hitch summaries: poll/apply, mesh accounting, geometry, queues, per-mesh"
    )?;
    Ok(())
}

fn append_log_block(path: &str, block: &str) -> std::io::Result<()> {
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    for line in block.lines() {
        writeln!(file, "{line}")?;
    }
    writeln!(file)?;
    Ok(())
}

pub fn format_terrain_streaming_sample(
    sample: &TerrainStreamingFrameSample,
    settings: &TerrainStreamingPerfSettings,
) -> String {
    let hitch = sample.total_streaming_ms() > settings.frame_time_threshold_ms
        || sample.mesh_build_max_ms > settings.mesh_build_threshold_ms;
    let level = if hitch { "HITCH" } else { "terrain streaming" };

    let mut out = String::new();
    out.push_str(&format!(
        "{level}\npoll={:.2}ms apply={:.2}ms mat_async={:.2}ms lod_async={:.2}ms mesh_assets={:.2}ms spawn={:.2}ms",
        sample.poll_ms,
        sample.apply_ms,
        sample.materialize_async_build_ms,
        sample.lod_async_build_ms,
        sample.mesh_assets_ms,
        sample.spawn_ms,
    ));
    out.push_str("\n\nMeshes:\n");
    out.push_str(&format!(
        "applied={} prebuilt={} mat_async_completed={} lod_async_completed={} logged={} async_avg={:.1}ms async_max={:.1}ms",
        sample.chunks_applied,
        sample.prebuilt_meshes_applied,
        sample.materialize_async_builds_completed,
        sample.lod_async_builds_completed,
        sample.mesh_build_log.len(),
        sample.mesh_build_avg_ms,
        sample.mesh_build_max_ms,
    ));
    out.push_str("\n\nLOD builds (logged this frame):\n");
    out.push_str(&format!(
        "Full={} Half={} Quarter={} Eighth={} total={}",
        sample.lod_build_counts.full,
        sample.lod_build_counts.half,
        sample.lod_build_counts.quarter,
        sample.lod_build_counts.eighth,
        sample.lod_build_counts.total(),
    ));
    out.push_str("\n\nGeometry:\n");
    out.push_str(&format!(
        "vertices={} indices={} triangles={} avg_vertices={} avg_triangles={}",
        sample.total_vertices,
        sample.total_indices,
        sample.total_triangles,
        sample.avg_vertices_per_mesh,
        sample.avg_triangles_per_mesh,
    ));
    out.push_str("\n\nLOD warmup:\n");
    out.push_str(&format!(
        "prefetch_req={} hits={} misses={} builds_from_prefetch={}",
        sample.lod_prefetch_requests,
        sample.lod_prefetch_hits,
        sample.lod_prefetch_misses,
        sample.lod_builds_started_from_prefetch,
    ));
    out.push_str("\n\nQueues:\n");
    out.push_str(&format!(
        "io={} decode={} mesh_build={} materialized_q={} applied={} unloaded={}",
        sample.io_in_flight,
        sample.decode_in_flight,
        sample.mesh_build_in_flight,
        sample.materialized_queue_len,
        sample.chunks_applied,
        sample.chunks_unloaded,
    ));

    if !sample.mesh_build_log.is_empty() {
        out.push_str("\n\nPer-mesh:");
        for entry in &sample.mesh_build_log {
            out.push_str(&format!(
                "\n  ({},{}) LOD={} reason={} verts={} tris={} build={:.2}ms",
                entry.coord.x,
                entry.coord.z,
                lod_label(entry.lod),
                mesh_build_reason_label(entry.reason),
                entry.geometry.vertices,
                entry.geometry.triangles,
                entry.build_ms,
            ));
        }
    }

    out
}

impl TerrainStreamingPerfState {
    pub fn begin_frame(&mut self) {
        self.frame = TerrainStreamingFrameSample::default();
    }

    pub fn frame_mut(&mut self) -> &mut TerrainStreamingFrameSample {
        &mut self.frame
    }

    pub fn finish_frame(
        &mut self,
        settings: &TerrainStreamingPerfSettings,
    ) -> Option<TerrainStreamingFrameSample> {
        if !settings.enabled {
            return None;
        }

        let sample = self.frame.clone();
        let now = Instant::now();

        let hitch = sample.total_streaming_ms() > settings.frame_time_threshold_ms
            || sample.mesh_build_max_ms > settings.mesh_build_threshold_ms;

        let due_summary = self
            .last_summary
            .map(|t| now.duration_since(t).as_secs_f32() >= settings.summary_interval_secs)
            .unwrap_or(true);

        let should_log = hitch || (due_summary && sample.has_activity());

        if due_summary && sample.has_activity() {
            self.last_summary = Some(now);
        }

        if should_log {
            Some(sample)
        } else {
            None
        }
    }
}

pub fn duration_to_ms(d: Duration) -> f32 {
    d.as_secs_f32() * 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::mesh::ChunkLod;

    #[test]
    fn finish_frame_logs_on_hitch_without_waiting_for_summary() {
        let settings = TerrainStreamingPerfSettings {
            enabled: true,
            summary_interval_secs: 60.0,
            ..Default::default()
        };
        let mut state = TerrainStreamingPerfState::default();
        state.begin_frame();
        state.frame_mut().apply_ms = 10.0;
        assert!(state.finish_frame(&settings).is_some());
    }

    #[test]
    fn finish_frame_suppresses_idle_frames_between_summaries() {
        let settings = TerrainStreamingPerfSettings {
            enabled: true,
            ..Default::default()
        };
        let mut state = TerrainStreamingPerfState::default();
        state.begin_frame();
        assert!(state.finish_frame(&settings).is_none());
    }

    #[test]
    fn recorder_tracks_prebuilt_apply() {
        let mut recorder = TerrainStreamingPerfRecorder::default();
        recorder.record_prebuilt_mesh_applied(
            ChunkCoord::new(2, 0),
            ChunkLod::Half,
            ChunkMeshGeometry {
                vertices: 16641,
                indices: 98304,
                triangles: 32768,
            },
        );

        let mut frame = TerrainStreamingFrameSample::default();
        recorder.finish_into(&mut frame);
        assert_eq!(frame.prebuilt_meshes_applied, 1);
        assert_eq!(frame.lod_build_counts.half, 1);
        assert_eq!(frame.mesh_build_log.len(), 1);
        assert_eq!(frame.mesh_build_log[0].reason, MeshBuildReason::AppliedPrebuilt);
    }

    #[test]
    fn file_log_writes_header_and_block() {
        let path = std::env::temp_dir().join(format!(
            "chasma_perf_log_{}.txt",
            std::process::id()
        ));
        let path_str = path.display().to_string();
        let settings = TerrainStreamingPerfSettings {
            enabled: true,
            log_to_file: true,
            log_file_path: path_str.clone(),
            ..Default::default()
        };
        let mut sample = TerrainStreamingFrameSample {
            apply_ms: 12.0,
            ..Default::default()
        };
        sample.mesh_build_log.push(MeshBuildLogEntry {
            coord: ChunkCoord::new(0, 0),
            lod: ChunkLod::Full,
            reason: MeshBuildReason::InitialMaterialize,
            build_ms: 57.0,
            geometry: ChunkMeshGeometry {
                vertices: 66049,
                indices: 393216,
                triangles: 131072,
            },
        });
        sample.refresh_geometry_averages();
        let block = format_terrain_streaming_sample(&sample, &settings);

        let mut file_log = TerrainStreamingPerfFileLog::default();
        file_log.append_block(&settings, &block);

        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("# terrain streaming perf session"));
        assert!(text.contains("HITCH") || text.contains("terrain streaming"));
        assert!(text.contains("LOD=Full"));
        assert!(text.contains("reason=InitialMaterialize"));
        assert!(text.contains("Per-mesh:"));
        std::fs::remove_file(path).ok();
    }
}

pub fn log_terrain_streaming_sample(
    sample: &TerrainStreamingFrameSample,
    settings: &TerrainStreamingPerfSettings,
    file_log: &mut TerrainStreamingPerfFileLog,
) {
    let block = format_terrain_streaming_sample(sample, settings);
    if settings.log_to_console {
        for line in block.lines() {
            info!("{line}");
        }
    }
    file_log.append_block(settings, &block);
}

/// Runs after the streaming chain; publishes the latest sample and logs sparingly.
pub fn report_terrain_streaming_perf(
    settings: Res<TerrainStreamingPerfSettings>,
    mut state: ResMut<TerrainStreamingPerfState>,
    mut latest: ResMut<TerrainStreamingPerfLatest>,
    mut file_log: ResMut<TerrainStreamingPerfFileLog>,
) {
    if !settings.enabled {
        return;
    }

    latest.0 = state.frame.clone();

    state.frame_mut().refresh_geometry_averages();

    if let Some(sample) = state.finish_frame(&settings) {
        log_terrain_streaming_sample(&sample, &settings, &mut file_log);
    }
}
