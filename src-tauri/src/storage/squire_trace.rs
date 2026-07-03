//! Structured, JSONL debug tracing for the Squire semantic retrieval loop
//! (node `root/Squire/squire-observability`, todo obs-1).
//!
//! This module is **pure observation**: it never changes retrieval or scoring
//! behavior, it only records what happened. Each event is appended as one JSON
//! object per line to `squire-trace.log`, sitting next to `provider-wire.log`
//! in the app config dir. It intentionally mirrors the resilience approach of
//! `append_wire_log` in `llm::openai` — all file IO errors are swallowed so a
//! tracing failure can never break the retrieval path.
//!
//! Gating: tracing is enabled when the build is a debug build
//! (`cfg!(debug_assertions)`, so it's ON by default in dev for immediate use)
//! OR the `SQUIRE_TRACE` env var is set to a truthy value (`1`, `true`, `yes`,
//! `on`, case-insensitive). Setting `SQUIRE_TRACE=0` (or `false`/`off`) does
//! NOT disable a debug build — the debug default wins; the env var is an
//! additive override for release builds. If that turns out to be surprising we
//! can revisit, but "on in dev, opt-in in release" matches the todo's intent.
//!
//! Only the RETRIEVAL trace (`event = "explore"`) is implemented here. The
//! other planned event types (token lifecycle, funnel, snapshot, timing,
//! query-probe) are deliberately deferred to later observability todos.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use serde::Serialize;
use serde_json::Value;

/// Path to the JSONL trace log, alongside `provider-wire.log`.
fn trace_log_path() -> PathBuf {
    crate::state::config::config_dir().join("squire-trace.log")
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
///
/// `payload` is a flexible `serde_json::Value` so each event type can carry its
/// own shape without bloating this struct; the RETRIEVAL trace payload shape is
/// documented on `trace_explore`.
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
///
/// `payload` is expected to be a JSON object with the retrieval shape:
/// `branch`, `resource_type`, `query`, `num_hops`, `max_results`,
/// `embedding_backend`, `results` (included candidates) and `near_misses`
/// (candidates dropped by the score<=0 cut or top-N truncation). Building the
/// payload is left to the caller so it can be assembled where the candidate
/// detail actually lives (see `explore_memory` and `SquireExploreTool`).
pub fn trace_explore(turn: u64, tool_call_id: Option<String>, payload: Value) {
    append_event(TraceEvent {
        ts: chrono::Utc::now().to_rfc3339(),
        turn,
        tool_call_id,
        event: "explore".to_string(),
        payload,
    });
}
