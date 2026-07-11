//! Tauri-bound event emitter for the engine.
//!
//! Wraps `AppHandle` to implement `EventEmitter`, bridging the
//! engine's event system to Tauri's frontend event system.

use async_trait::async_trait;
use tauri::Emitter;

use super::traits::{EngineEvent, EventEmitter};

/// Wraps a Tauri `AppHandle` to implement `EventEmitter`.
///
/// This is how the real app connects the engine to the frontend.
/// Tests use `RecordingEventEmitter` instead.
pub struct TauriEventEmitter {
    app: tauri::AppHandle,
}

impl TauriEventEmitter {
    pub fn new(app: tauri::AppHandle) -> Self {
        Self { app }
    }
}

#[async_trait]
impl EventEmitter for TauriEventEmitter {
    async fn emit(&self, event: &EngineEvent) {
        match event {
            EngineEvent::Chunk(text) => {
                let _ = self.app.emit("stream-chunk", text);
            }
            EngineEvent::Thinking(text) => {
                let _ = self.app.emit("stream-thinking", text);
            }
            EngineEvent::ToolCall(tc) => {
                let _ = self.app.emit("stream-tool-call", tc);
            }
            EngineEvent::ToolResult(result) => {
                let _ = self.app.emit("stream-tool-result", result);
            }
            EngineEvent::ToolPending(payload) => {
                let _ = self.app.emit("stream-tool-pending", payload.to_string());
            }
            EngineEvent::Status(status) => {
                let _ = self.app.emit("stream-status", status);
            }
            EngineEvent::Output { source, line, timestamp } => {
                let _ = self.app.emit(
                    "output:append",
                    serde_json::json!({
                        "source": source,
                        "line": line,
                        "timestamp": timestamp,
                    }),
                );
            }
            EngineEvent::Error(err) => {
                let _ = self.app.emit("stream-error", err);
            }
            EngineEvent::Done => {
                let _ = self.app.emit("stream-done", "");
            }
            EngineEvent::Phase2Summary(summary) => {
                let _ = self.app.emit("stream-phase2-summary", summary.to_string());
            }
            EngineEvent::AskUserPending(payload) => {
                let _ = self.app.emit("stream-ask-user-pending", payload.to_string());
            }
        }
    }

    async fn emit_status(&self, status: &str) {
        let _ = self.app.emit("stream-status", status.to_string());
    }
}
