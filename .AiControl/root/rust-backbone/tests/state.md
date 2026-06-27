# Tests — Phase 1: Rust Backbone

**Status:** ✅ 34/34 tests passing

## Test Coverage

| Module | Tests | What's tested |
|--------|-------|---------------|
| `state::config` | 4 | Defaults, TOML round-trip, provider config, save/load |
| `storage::conversation_store` | 4 | MessageRole round-trip, session creation, UUID parsing, error display |
| `llm::provider` | 5 | ChatMessage, ToolDefinition, FinishReason, ToolCall, Error display |
| `fs::ops` | 6 | Read/write, nonexistent, create/delete dir, rename, list directory |
| `fs::git` | 5 | Status (clean/modified), log, branches, non-repo error |
| `search::grep` | 3 | Options builder, replace options, error display |
| `shell::exec` | 5 | echo, exit code, stdin, command not found, working directory |

## Run

```
cargo test
```
