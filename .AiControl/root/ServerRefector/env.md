# Env — ServerRefector

Inherits all env from `root`. Key areas of focus:

## Rust Crates to Modify

| Crate | Path | Role |
|-------|------|------|
| `provider-core` | `src-tauri/crates/provider-core/` | Add `ModelInstance`, `ModelOptions`, update `LlmProvider` trait |
| `provider-openai` | `src-tauri/crates/provider-openai/` | Update to use `ModelInstance` |
| `provider-anthropic` | `src-tauri/crates/provider-anthropic/` | Update to use `ModelInstance` |
| `provider-registry` | `src-tauri/crates/provider-registry/` | May need updates for `ModelInstance` lookup |
| `squire-store` | `src-tauri/crates/squire-store/` | No changes expected (already well-abstracted) |

## Source Files to Refactor

| File | Role |
|------|------|
| `src-tauri/src/commands/streaming_cmd.rs` | Engine loop — extract into `SquireEngine` |
| `src-tauri/src/commands/setup_cmd.rs` | RuntimeContext construction from AppState |
| `src-tauri/src/commands/mod.rs` | AppState may slim down |
| `src-tauri/src/llm/registry.rs` | Bridge between config and Registry |
| `src-tauri/src/agent/squire/adapter.rs` | SquireContextAdapter — may slim down |

## Test Files

| File | What to add |
|------|-------------|
| `src-tauri/src/agent/squire_test.rs` | Add tests using `RuntimeContext` |
| New: `src-tauri/crates/provider-core/tests/` | ModelInstance serialization tests |
| New: `src-tauri/src/engine/` | Headless engine integration tests |

## Commands

```powershell
# Build after changes
.\scripts\build.ps1

# Run tests
.\scripts\test.ps1
.\scripts\test-all.ps1
```

