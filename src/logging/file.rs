//! Shared append-only log file helpers (used by terrain perf and dev runtime logs).
//!
//! High-volume callers should pass `flush: false` to avoid syncing every line to disk.

use std::collections::{HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::{Mutex, OnceLock};

/// Why a log write failed.
#[derive(Debug)]
pub struct FileLogError(pub std::io::Error);

impl std::fmt::Display for FileLogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for FileLogError {}

struct LogFileState {
    headers_written: HashSet<String>,
    writers: HashMap<String, BufWriter<std::fs::File>>,
}

impl LogFileState {
    fn new() -> Self {
        Self {
            headers_written: HashSet::new(),
            writers: HashMap::new(),
        }
    }
}

fn log_state() -> &'static Mutex<LogFileState> {
    static STATE: OnceLock<Mutex<LogFileState>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(LogFileState::new()))
}

fn ensure_parent_dir(path: &str) -> std::io::Result<()> {
    if let Some(parent) = Path::new(path).parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    Ok(())
}

fn append_inner(path: &str, session_header: &str, line: &str, flush: bool) {
    let result = (|| -> Result<(), FileLogError> {
        ensure_parent_dir(path).map_err(FileLogError)?;
        let mut state = log_state().lock().expect("log file mutex poisoned");
        if state.headers_written.insert(path.to_string()) {
            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .map_err(FileLogError)?;
            writeln!(file, "{session_header}").map_err(FileLogError)?;
        }
        let writer = match state.writers.entry(path.to_string()) {
            std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::hash_map::Entry::Vacant(entry) => {
                let file = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                    .map_err(FileLogError)?;
                entry.insert(BufWriter::new(file))
            }
        };
        writeln!(writer, "{line}").map_err(FileLogError)?;
        if flush {
            writer.flush().map_err(FileLogError)?;
        }
        Ok(())
    })();

    if let Err(err) = result {
        eprintln!("chasma log: failed to write {path}: {err}");
    }
}

/// Write a session header once per process for `path`.
pub fn write_session_header(path: &str, header_line: &str) -> Result<(), FileLogError> {
    ensure_parent_dir(path).map_err(FileLogError)?;
    let mut state = log_state().lock().expect("log file mutex poisoned");
    if !state.headers_written.insert(path.to_string()) {
        return Ok(());
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(FileLogError)?;
    writeln!(file, "{header_line}").map_err(FileLogError)?;
    Ok(())
}

/// Append a single line. Startup/low-volume logs should use the default `flush = true`.
pub fn append_log_line(path: &str, session_header: &str, line: &str) {
    append_inner(path, session_header, line, true);
}

/// Buffered append for high-volume streams (does not flush every line).
pub fn append_log_line_buffered(path: &str, session_header: &str, line: &str) {
    append_inner(path, session_header, line, false);
}

/// Append a multi-line block (blank line separator after the block).
pub fn append_log_block(path: &str, session_header: &str, block: &str) {
    let result = (|| -> Result<(), FileLogError> {
        ensure_parent_dir(path).map_err(FileLogError)?;
        let mut state = log_state().lock().expect("log file mutex poisoned");
        if state.headers_written.insert(path.to_string()) {
            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .map_err(FileLogError)?;
            writeln!(file, "{session_header}").map_err(FileLogError)?;
        }
        let writer = match state.writers.entry(path.to_string()) {
            std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::hash_map::Entry::Vacant(entry) => {
                let file = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                    .map_err(FileLogError)?;
                entry.insert(BufWriter::new(file))
            }
        };
        for line in block.lines() {
            writeln!(writer, "{line}").map_err(FileLogError)?;
        }
        writeln!(writer).map_err(FileLogError)?;
        writer.flush().map_err(FileLogError)?;
        Ok(())
    })();

    if let Err(err) = result {
        eprintln!("chasma log: failed to write {path}: {err}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_log(name: &str) -> String {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir()
            .join(format!("chasma_log_test_{name}_{stamp}.log"))
            .to_string_lossy()
            .into_owned()
    }

    #[test]
    fn append_log_line_writes_header_once() {
        let path = temp_log("line");
        append_log_line(&path, "# test session", "first");
        append_log_line(&path, "# test session", "second");
        let contents = std::fs::read_to_string(&path).unwrap();
        assert_eq!(contents.matches("# test session").count(), 1);
        assert!(contents.contains("first"));
        assert!(contents.contains("second"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn buffered_append_keeps_handle_open() {
        let path = temp_log("buffered");
        for i in 0..32 {
            append_log_line_buffered(&path, "# buffered", &format!("line {i}"));
        }
        append_log_line(&path, "# buffered", "flush");
        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains("line 31"));
        let _ = std::fs::remove_file(path);
    }
}
