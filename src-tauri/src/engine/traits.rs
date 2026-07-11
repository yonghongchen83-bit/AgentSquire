use async_trait::async_trait;
use serde::Serialize;
use squire_store::SessionId;

use super::runtime::RuntimeContext;

// ── EventEmitter ──────────────────────────────────────────────────────

/// Events that the engine can emit to the frontend.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum EngineEvent {
    /// A text chunk for the streaming response.
    Chunk(String),
    /// Thinking/reasoning content from the model.
    Thinking(String),
    /// A tool call detected in the model's response.
    ToolCall(serde_json::Value),
    /// A tool execution result.
    ToolResult(serde_json::Value),
    /// A tool is pending user approval.
    ToolPending(serde_json::Value),
    /// A status update message.
    Status(String),
    /// A general-purpose output line (for debug/timing logs).
    Output { source: String, line: String, timestamp: String },
    /// An error message.
    Error(String),
    /// The stream is complete.
    Done,
    /// Phase 2 completed with a summary.
    Phase2Summary(serde_json::Value),
    /// The model is asking the user a question (Squire ask-user loop).
    AskUserPending(serde_json::Value),
}

/// Abstraction over Tauri's event emission system.
///
/// The real app provides a `TauriEventEmitter` that wraps `AppHandle::emit()`.
/// Tests provide a `RecordingEventEmitter` that captures events for
/// assertions.
#[async_trait]
pub trait EventEmitter: Send + Sync {
    /// Emit an engine event.
    async fn emit(&self, event: &EngineEvent);
    /// Emit a status update string.
    async fn emit_status(&self, status: &str);
}

// ── Engine trait ──────────────────────────────────────────────────────

/// The core abstraction for running a chat turn.
///
/// An engine takes a `RuntimeContext` and produces streaming events via
/// the context's `EventEmitter`. The engine is completely decoupled from
/// Tauri — it never imports `AppHandle`, `State`, or any Tauri type.
///
/// # Lifecycle
///
/// 1. Construct a `RuntimeContext` with all dependencies.
/// 2. Call `Engine::run()`.
/// 3. The engine emits events via `RuntimeContext.event_emitter`.
/// 4. When complete, the engine returns `Ok(())`.
///
/// # Implementations
///
/// - `SquireEngine` — the current production engine supporting both Legacy
///   and Squire context modes with two-phase protocol.
#[async_trait]
pub trait Engine: Send {
    /// Execute a chat turn.
    ///
    /// The engine will:
    /// - Append the user message to the store
    /// - Build the turn context via `ContextManagerAdapter`
    /// - Call the LLM provider
    /// - Handle tool calls (approval, execution, feedback)
    /// - Finalize the turn
    /// - Handle Phase 2 if applicable (Squire mode)
    async fn run(
        self: Box<Self>,
        ctx: RuntimeContext,
        session_id: SessionId,
    ) -> Result<(), String>;
}
