# Decisions — Phase 4

Decisions made during agent tools implementation.

| # | Decision | Rationale |
|---|----------|-----------|
| 1 | **Tool trait lives in `agent/mod.rs`** with 5 concrete implementations | Each tool wraps an existing backend module (fs::ops, shell::exec, etc.) |
| 2 | **`ChatMessage.tool_calls` field** added for multi-turn tool loop | Enables feeding tool call info back to the LLM for multi-step reasoning |
| 3 | **Oneshot channels for approve/reject** | Clean one-shot async signaling; no polling or busy-waiting |
| 4 | **Destructive tools require approval** (`write_file`, `run_terminal`) | File writes and shell commands can modify the system; user must explicitly approve |
| 5 | **Multi-turn loop with result feedback** | Tool results are injected as `ChatRole::Tool` messages and the LLM is re-invoked automatically |
| 6 | **OpenAI and Anthropic both supported** | Each provider serializes tool calls/results in its native API format |
| 7 | **`ToolRegistry` built at stream start** | No need for persistent registry; created fresh per `send_message` call |
