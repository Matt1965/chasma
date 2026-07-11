//! Append-only file logging under [`LOGS_DIR`] (dev/runtime diagnostics).

mod file;

pub use file::{
    FileLogError, append_log_block, append_log_line, append_log_line_buffered, write_session_header,
};

/// Directory for all runtime log files (relative to process working directory).
pub const LOGS_DIR: &str = "logs";

/// One-time dev startup messages (Excel import, biome load, preview spawns).
pub const DEV_STARTUP_LOG_PATH: &str = "logs/dev_startup.log";

/// Per-chunk procedural doodad materialization (high volume during streaming).
pub const DOODAD_PROCGEN_LOG_PATH: &str = "logs/doodad_procgen.log";

/// Terrain streaming performance samples (dev preview opt-in).
pub const TERRAIN_STREAMING_PERF_LOG_PATH: &str = "logs/terrain_streaming_perf.log";
