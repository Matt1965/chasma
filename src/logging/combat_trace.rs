//! File sink for high-volume combat/perception runtime diagnostics (dev).

use std::sync::Once;

use super::COMBAT_TRACE_LOG_PATH;
use super::file::{append_log_line_buffered, begin_fresh_session_log};

const SESSION_HEADER: &str = "# chasma combat trace";

static SESSION: Once = Once::new();

fn ensure_combat_trace_session() {
    SESSION.call_once(|| {
        if let Err(err) = begin_fresh_session_log(COMBAT_TRACE_LOG_PATH, SESSION_HEADER) {
            eprintln!(
                "chasma log: failed to begin combat trace session at {COMBAT_TRACE_LOG_PATH}: {err}"
            );
        }
    });
}

/// Append one `COMBAT_TRACE` line to [`COMBAT_TRACE_LOG_PATH`].
#[cfg(feature = "dev")]
pub fn write_combat_trace(line: impl AsRef<str>) {
    ensure_combat_trace_session();
    append_log_line_buffered(
        COMBAT_TRACE_LOG_PATH,
        SESSION_HEADER,
        &format!("COMBAT_TRACE {}", line.as_ref()),
    );
}

#[cfg(not(feature = "dev"))]
pub fn write_combat_trace(_line: impl AsRef<str>) {}

/// Append one `PERCEPTION_TRACE` line to [`COMBAT_TRACE_LOG_PATH`].
#[cfg(feature = "dev")]
pub fn write_perception_trace(line: impl AsRef<str>) {
    ensure_combat_trace_session();
    append_log_line_buffered(
        COMBAT_TRACE_LOG_PATH,
        SESSION_HEADER,
        &format!("PERCEPTION_TRACE {}", line.as_ref()),
    );
}

#[cfg(not(feature = "dev"))]
pub fn write_perception_trace(_line: impl AsRef<str>) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logging::append_log_line;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_log(name: &str) -> String {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir()
            .join(format!("chasma_combat_trace_{name}_{stamp}.log"))
            .to_string_lossy()
            .into_owned()
    }

    #[test]
    fn fresh_session_log_truncates_previous_content() {
        let path = temp_log("truncate");
        std::fs::write(&path, "# chasma combat trace\nCOMBAT_TRACE stale\n").unwrap();
        begin_fresh_session_log(&path, SESSION_HEADER).expect("fresh session");
        append_log_line(&path, SESSION_HEADER, "COMBAT_TRACE fresh");
        let contents = std::fs::read_to_string(&path).unwrap();
        assert_eq!(contents.matches(SESSION_HEADER).count(), 1);
        assert!(!contents.contains("stale"));
        assert!(contents.contains("fresh"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn combat_and_perception_traces_share_chronological_sink_format() {
        let path = temp_log("format");
        begin_fresh_session_log(&path, SESSION_HEADER).expect("fresh session");
        append_log_line(&path, SESSION_HEADER, "COMBAT_TRACE event=accepted");
        append_log_line(
            &path,
            SESSION_HEADER,
            "PERCEPTION_TRACE observer=1 sight_range_m=24.00 candidates=2",
        );
        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains("COMBAT_TRACE event=accepted"));
        assert!(contents.contains("PERCEPTION_TRACE observer=1"));
        let combat_pos = contents.find("COMBAT_TRACE").expect("combat line");
        let perception_pos = contents.find("PERCEPTION_TRACE").expect("perception line");
        assert!(combat_pos < perception_pos);
        let _ = std::fs::remove_file(path);
    }
}
