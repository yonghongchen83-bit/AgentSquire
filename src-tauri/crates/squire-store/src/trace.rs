//! Structured, JSONL debug tracing for the Squire semantic retrieval loop.
//!
//! This module is **pure observation**: it never changes retrieval or scoring
//! behavior, it only records what happened. Each event is appended as one JSON
//! object per line to `squire-trace.log`.
//!
//! Gating: tracing is enabled when the build is a debug build
//! (`cfg!(debug_assertions)`, so it's ON by default in dev for immediate use)
//! OR the `SQUIRE_TRACE` env var is set to a truthy value (`1`, `true`, `yes`,
//! `on`, case-insensitive). Setting `SQUIRE_TRACE=0` (or `false`/`off`) does
//! NOT disable a debug build — the debug default wins; the env var is an
//! additive override for release builds.
//!
//! The trace log directory is configured via `set_trace_dir`, which should be
//! called once at app startup (from the main crate's initialization).

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::RwLock;

use serde::Serialize;
use serde_json::Value;

/// Directory where `squire-trace.log` is written.
///
/// Initialised at app startup to `{config_dir}`.  Updated during workspace
/// bind to `{workspace}/.squire` and reverted on unbind, so the trace log
/// follows the active workspace.
static TRACE_DIR: RwLock<Option<PathBuf>> = RwLock::new(None);

/// Configure the trace output directory.  Can be called multiple times;
/// later calls replace the previous value.
pub fn set_trace_dir(dir: PathBuf) {
    if let Ok(mut guard) = TRACE_DIR.write() {
        *guard = Some(dir);
    }
}

fn trace_log_path() -> PathBuf {
    let dir = TRACE_DIR
        .read()
        .ok()
        .and_then(|g| g.clone())
        .unwrap_or_else(|| PathBuf::from("."));
    dir.join("squire-trace.log")
}

/// Whether tracing is currently enabled. Debug builds are on by default;
/// release builds opt in via `SQUIRE_TRACE`.
pub fn trace_enabled() -> bool {
    if cfg!(debug_assertions) {
        return true;
    }
    match std::env::var("SQUIRE_TRACE") {
        Ok(v) => {
            let v = v.trim().to_ascii_lowercase();
            matches!(v.as_str(), "1" | "true" | "yes" | "on")
        }
        Err(_) => false,
    }
}

/// One structured trace event, serialized as a single JSONL line.
#[derive(Debug, Serialize)]
pub struct TraceEvent {
    /// RFC3339 timestamp (UTC) of when the event was recorded.
    pub ts: String,
    /// Squire turn counter at the time of the event.
    pub turn: u64,
    /// Correlating tool-call id, when the event originates from a tool call.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// Event discriminator, e.g. `"explore"`.
    pub event: String,
    /// Event-specific structured detail.
    pub payload: Value,
}

/// Append one event to `squire-trace.log`. No-op when tracing is disabled.
/// All IO errors are swallowed — tracing must never break retrieval.
pub fn append_event(event: TraceEvent) {
    if !trace_enabled() {
        return;
    }
    let line = match serde_json::to_string(&event) {
        Ok(l) => l,
        Err(_) => return,
    };
    let path = trace_log_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(file, "{}", line);
    }
}

/// Emit a RETRIEVAL trace (`event = "explore"`).
pub fn trace_explore(turn: u64, tool_call_id: Option<String>, payload: Value) {
    append_event(TraceEvent {
        ts: chrono::Utc::now().to_rfc3339(),
        turn,
        tool_call_id,
        event: "explore".to_string(),
        payload,
    });
}
