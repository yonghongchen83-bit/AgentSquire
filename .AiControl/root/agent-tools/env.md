See `.AiControl/root/env.md` for full toolchain setup.

Phase-specific: all backend work — no new npm packages expected.

## Implementation Notes

- Tools wrap existing `fs::ops`, `shell::exec`, `search::grep`, `fs::git` functions
- OpenAI and Anthropic providers both handle tool message serialization
- OpenAI uses `tool_calls` array in assistant messages + `tool` role messages
- Anthropic uses `tool_use`/`tool_result` content blocks
- Approve/reject uses `tokio::sync::oneshot` channels stored in `PendingApprovals` managed state
