# Decisions

## Adapter interface and insertion seam (ac-1) — finalized 2026-07-02

Mapped against the real orchestration code in `src-tauri/src/commands/streaming_cmd.rs::send_message_impl` (lines 98-618).

### Trait

New module: `src-tauri/src/agent/context_adapter.rs`

```rust
#[async_trait]
pub trait ContextManagerAdapter: Send + Sync {
    /// Seam 1 — replaces lines 254-270 (history replay) and the tools
    /// passed into ChatRequest at line 277.
    async fn build_turn_input(
        &mut self,
        session: &Session,
        base_tools: &[ToolDefinition],
    ) -> Result<TurnInput, String>;

    /// Seam 2 — replaces the message-append tail of the per-tool-call
    /// body at lines 547-565 (inside the `for tc in &tool_calls` loop,
    /// lines 437-566).
    async fn handle_tool_loop_step(
        &mut self,
        call_id: &str,
        tool_name: &str,
        result: &agent::ToolResult,
        reasoning: Option<String>,
        messages: &mut Vec<ChatMessage>,
    ) -> Result<(), String>;

    /// Seam 3 — replaces the direct `store.append_message(...)` call at
    /// lines 589-596 (FinishReason::Stop | FinishReason::Length branch).
    async fn finalize_turn(
        &mut self,
        session_id: SessionId,
        assistant_content: Option<String>,
        thinking: Option<String>,
        store: &dyn ConversationStore,
    ) -> Result<(), String>;
}

pub struct TurnInput {
    pub messages: Vec<ChatMessage>,
    pub tools: Vec<ToolDefinition>,
}
```

### Why these three seams and not others

- Provider selection, streaming transport (lines 295-345), the approval/watchdog mechanism (lines 437-515), and MCP tool discovery (lines 173-249) stay in orchestration core per the planning-node boundary decision — they're identical in Legacy and Squire mode.
- `reasoning` is passed in explicitly rather than adapters reaching into `full_thinking` themselves, because current behavior only attaches reasoning to the *first* tool-call message per turn (`std::mem::take`, line 542) — keeping that take-logic at the call site preserves exact parity without adapters needing to replicate a stateful quirk.
- `build_turn_input` returns tools alongside messages (not just messages) because Squire's strict gateway-only tool exposure (Q5, resolved in `../planning/decisions.md`) needs to filter `base_tools` down to just the gateway tool — that filtering has to happen before `ChatRequest` is built, same seam as history assembly.

### LegacyContextAdapter mapping (ac-2 scope)

- `build_turn_input`: reproduces lines 254-270 verbatim (map `session.messages` to `ChatMessage`), returns `base_tools` unfiltered.
- `handle_tool_loop_step`: reproduces lines 547-565 verbatim (push assistant tool_call message, then tool-result message).
- `finalize_turn`: reproduces lines 589-596 verbatim (append assistant message only if `content` non-empty).

### Adapter selection for this node

`send_message_impl` will construct `LegacyContextAdapter` unconditionally for now — no `context_mode` field exists yet (that's `../session-mode`). This keeps ac-2 a pure extract-with-no-behavior-change refactor, per the planning node's incremental delivery plan step 1.

### Parity test strategy (ac-2)

Cannot unit-test `send_message_impl` directly (it's a Tauri command spawning a detached tokio task with `AppHandle`/`State` params). Parity tests target the adapter in isolation:
- `LegacyContextAdapter::build_turn_input` given a fixture `Session` with mixed User/Assistant/System messages produces the same `Vec<ChatMessage>` the old inline code would have.
- `handle_tool_loop_step` given a fixture `ToolResult` and `reasoning` produces the same two-message push sequence, including the `is_error` passthrough into `content`.
- `finalize_turn` given empty vs non-empty `assistant_content` matches the old empty-content skip behavior (line 583 `if !content.is_empty()`).
