//! Dev-only terrain streaming performance instrumentation.
//!
//! Measures poll/apply/mesh/spawn costs without changing streaming architecture.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::time::{Duration, Instant, SystemTime};

use bevy::prelude::*;

use crate::world::ChunkCoord;

use super::mesh::ChunkMeshGeometry;

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

/// Why a chunk mesh was built during apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeshBuildKind {
    NewChunk,
    NeighborRebuild,
}

/// Per-mesh build record for detailed hitch logs.
#[derive(Debug, Clone)]
pub struct BuiltMeshLogEntry {
    pub coord: ChunkCoord,
    pub kind: MeshBuildKind,
    pub build_ms: f32,
    pub geometry: ChunkMeshGeometry,
}

/// Per-frame streaming measurements (one terrain streaming chain tick).
#[derive(Debug, Clone, Default, Reflect)]
pub struct TerrainStreamingFrameSample {
    pub io_in_flight: usize,
    pub decode_in_flight: usize,
    pub mesh_build_in_flight: usize,
    pub decoded_queue_len: usize,
    pub chunks_applied: usize,
    pub chunks_unloaded: usize,
    pub meshes_built: usize,
    pub new_chunk_meshes: usize,
    pub neighbor_meshes_rebuilt: usize,
    pub mesh_build_count: usize,
    pub mesh_build_avg_ms: f32,
    pub mesh_build_max_ms: f32,
    pub neighbors_considered: usize,
    pub neighbors_rebuilt: usize,
    pub neighbors_skipped: usize,
    pub total_vertices: usize,
    pub total_indices: usize,
    pub total_triangles: usize,
    pub avg_vertices_per_mesh: usize,
    pub avg_triangles_per_mesh: usize,
    pub lod_prefetch_requests: usize,
    pub lod_prefetch_hits: usize,
    pub lod_prefetch_misses: usize,
    pub lod_builds_started_from_prefetch: usize,
    pub poll_ms: f32,
    pub apply_ms: f32,
    pub async_mesh_build_ms: f32,
    pub async_mesh_builds_completed: usize,
    pub main_thread_mesh_build_ms: f32,
    pub prebuilt_meshes_applied: usize,
    pub mesh_build_ms: f32,
    pub mesh_assets_ms: f32,
    pub spawn_ms: f32,
    #[reflect(ignore)]
    pub mesh_details: Vec<BuiltMeshLogEntry>,
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
            || self.decoded_queue_len > 0
    }
}

/// Latest completed sample (inspectable in the editor).
#[derive(Debug, Clone, Resource, Reflect, Default)]
#[reflect(Resource)]
pub struct TerrainStreamingPerfLatest(pub TerrainStreamingFrameSample);

/// Accumulates mesh/spawn timings during one apply pass.
#[derive(Debug, Default)]
pub struct TerrainStreamingPerfRecorder {
    mesh_build: Duration,
    mesh_assets: Duration,
    spawn: Duration,
    mesh_build_durations: Vec<Duration>,
    new_chunk_meshes: usize,
    neighbor_meshes_rebuilt: usize,
    neighbors_considered: usize,
    neighbors_rebuilt: usize,
    neighbors_skipped: usize,
    total_vertices: usize,
    total_indices: usize,
    total_triangles: usize,
    mesh_details: Vec<BuiltMeshLogEntry>,
    prebuilt_meshes_applied: usize,
}

impl TerrainStreamingPerfRecorder {
    pub fn record_neighbor_considered(&mut self) {
        self.neighbors_considered += 1;
    }

    pub fn record_neighbor_skipped(&mut self) {
        self.neighbors_skipped += 1;
    }

    pub fn record_neighbor_rebuilt(&mut self) {
        self.neighbors_rebuilt += 1;
    }

    pub fn record_prebuilt_mesh_applied(&mut self) {
        self.prebuilt_meshes_applied += 1;
    }

    pub fn record_mesh_build(
        &mut self,
        kind: MeshBuildKind,
        coord: ChunkCoord,
        elapsed: Duration,
        geometry: ChunkMeshGeometry,
    ) {
        self.mesh_build += elapsed;
        self.mesh_build_durations.push(elapsed);
        match kind {
            MeshBuildKind::NewChunk => self.new_chunk_meshes += 1,
            MeshBuildKind::NeighborRebuild => self.neighbor_meshes_rebuilt += 1,
        }
        self.total_vertices += geometry.vertices;
        self.total_indices += geometry.indices;
        self.total_triangles += geometry.triangles;
        self.mesh_details.push(BuiltMeshLogEntry {
            coord,
            kind,
            build_ms: duration_to_ms(elapsed),
            geometry,
        });
    }

    pub fn record_mesh_assets(&mut self, elapsed: Duration) {
        self.mesh_assets += elapsed;
    }

    pub fn record_spawn(&mut self, elapsed: Duration) {
        self.spawn += elapsed;
    }

    pub fn finish_into(&self, frame: &mut TerrainStreamingFrameSample) {
        let built = self.mesh_build_durations.len();
        frame.meshes_built = built;
        frame.new_chunk_meshes = self.new_chunk_meshes;
        frame.neighbor_meshes_rebuilt = self.neighbor_meshes_rebuilt;
        frame.mesh_build_count = built;
        frame.mesh_build_ms = duration_to_ms(self.mesh_build);
        frame.main_thread_mesh_build_ms = frame.mesh_build_ms;
        frame.prebuilt_meshes_applied = self.prebuilt_meshes_applied;
        frame.mesh_assets_ms = duration_to_ms(self.mesh_assets);
        frame.spawn_ms = duration_to_ms(self.spawn);
        frame.neighbors_considered = self.neighbors_considered;
        frame.neighbors_rebuilt = self.neighbors_rebuilt;
        frame.neighbors_skipped = self.neighbors_skipped;
        frame.total_vertices = self.total_vertices;
        frame.total_indices = self.total_indices;
        frame.total_triangles = self.total_triangles;
        frame.avg_vertices_per_mesh = if built > 0 {
            self.total_vertices / built
        } else {
            0
        };
        frame.avg_triangles_per_mesh = if built > 0 {
            self.total_triangles / built
        } else {
            0
        };
        if built > 0 {
            let total_ms: f32 = self
                .mesh_build_durations
                .iter()
                .map(|d| duration_to_ms(*d))
                .sum();
            frame.mesh_build_avg_ms = total_ms / built as f32;
            frame.mesh_build_max_ms = self
                .mesh_build_durations
                .iter()
                .map(|d| duration_to_ms(*d))
                .fold(0.0_f32, f32::max);
        }
        frame.mesh_details = self.mesh_details.clone();
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
        || sample.mesh_build_ms > settings.mesh_build_threshold_ms;
    let level = if hitch { "HITCH" } else { "terrain streaming" };

    let mut out = String::new();
    out.push_str(&format!(
        "{level}\npoll={:.2}ms apply={:.2}ms async_mesh={:.2}ms main_mesh={:.2}ms mesh_assets={:.2}ms spawn={:.2}ms",
        sample.poll_ms,
        sample.apply_ms,
        sample.async_mesh_build_ms,
        sample.main_thread_mesh_build_ms,
        sample.mesh_assets_ms,
        sample.spawn_ms,
    ));
    out.push_str("\n\nMeshes:\n");
    out.push_str(&format!(
        "applied={} prebuilt={} new={} neighbor={} built={} async_completed={} count={} avg={:.1}ms max={:.1}ms",
        sample.chunks_applied,
        sample.prebuilt_meshes_applied,
        sample.new_chunk_meshes,
        sample.neighbor_meshes_rebuilt,
        sample.meshes_built,
        sample.async_mesh_builds_completed,
        sample.mesh_build_count,
        sample.mesh_build_avg_ms,
        sample.mesh_build_max_ms,
    ));
    out.push_str("\n\nNeighbor refresh:\n");
    out.push_str(&format!(
        "considered={} rebuilt={} skipped={}",
        sample.neighbors_considered, sample.neighbors_rebuilt, sample.neighbors_skipped,
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
        sample.decoded_queue_len,
        sample.chunks_applied,
        sample.chunks_unloaded,
    ));

    if !sample.mesh_details.is_empty() {
        out.push_str("\n\nPer-mesh:");
        for entry in &sample.mesh_details {
            let kind = match entry.kind {
                MeshBuildKind::NewChunk => "new",
                MeshBuildKind::NeighborRebuild => "neighbor",
            };
            out.push_str(&format!(
                "\n  ({},{}) {kind} verts={} tris={} build={:.2}ms",
                entry.coord.x,
                entry.coord.z,
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
            || sample.mesh_build_ms > settings.mesh_build_threshold_ms;

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
    fn recorder_tracks_new_and_neighbor_mesh_counts() {
        let mut recorder = TerrainStreamingPerfRecorder::default();
        recorder.record_neighbor_considered();
        recorder.record_neighbor_considered();
        recorder.record_neighbor_skipped();
        recorder.record_neighbor_rebuilt();
        recorder.record_mesh_build(
            MeshBuildKind::NewChunk,
            ChunkCoord::new(1, 0),
            Duration::from_millis(40),
            ChunkMeshGeometry {
                vertices: 100,
                indices: 600,
                triangles: 200,
            },
        );
        recorder.record_mesh_build(
            MeshBuildKind::NeighborRebuild,
            ChunkCoord::new(0, 0),
            Duration::from_millis(60),
            ChunkMeshGeometry {
                vertices: 100,
                indices: 600,
                triangles: 200,
            },
        );

        let mut frame = TerrainStreamingFrameSample::default();
        recorder.finish_into(&mut frame);
        assert_eq!(frame.new_chunk_meshes, 1);
        assert_eq!(frame.neighbor_meshes_rebuilt, 1);
        assert_eq!(frame.meshes_built, 2);
        assert_eq!(frame.neighbors_considered, 2);
        assert_eq!(frame.neighbors_skipped, 1);
        assert_eq!(frame.neighbors_rebuilt, 1);
        assert_eq!(frame.total_vertices, 200);
        assert_eq!(frame.total_triangles, 400);
        assert!((frame.mesh_build_avg_ms - 50.0).abs() < 0.1);
        assert!((frame.mesh_build_max_ms - 60.0).abs() < 0.1);
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
            meshes_built: 3,
            new_chunk_meshes: 1,
            neighbor_meshes_rebuilt: 2,
            mesh_build_count: 3,
            mesh_build_avg_ms: 42.0,
            mesh_build_max_ms: 57.0,
            ..Default::default()
        };
        sample.mesh_details.push(BuiltMeshLogEntry {
            coord: ChunkCoord::new(0, 0),
            kind: MeshBuildKind::NewChunk,
            build_ms: 57.0,
            geometry: ChunkMeshGeometry {
                vertices: 66049,
                indices: 393216,
                triangles: 131072,
            },
        });
        let block = format_terrain_streaming_sample(&sample, &settings);

        let mut file_log = TerrainStreamingPerfFileLog::default();
        file_log.append_block(&settings, &block);

        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("# terrain streaming perf session"));
        assert!(text.contains("HITCH") || text.contains("terrain streaming"));
        assert!(text.contains("neighbor=2"));
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

    if let Some(sample) = state.finish_frame(&settings) {
        log_terrain_streaming_sample(&sample, &settings, &mut file_log);
    }
}
