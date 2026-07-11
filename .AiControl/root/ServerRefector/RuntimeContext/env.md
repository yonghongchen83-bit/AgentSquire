# Env — RuntimeContext

## Key Files

| File | Action |
|------|--------|
| New crate: `src-tauri/crates/engine-core/` | Define `RuntimeContext`, `RuntimeConfig`, `Engine` trait |
| `src-tauri/crates/provider-core/src/lib.rs` | May need `WorkspaceProvider` trait or similar |
| `src-tauri/src/commands/streaming_cmd.rs` | Extract engine loop into `SquireEngine` |
| New: `src-tauri/src/engine/` | `SquireEngine` implementation module |
| `src-tauri/src/commands/setup_cmd.rs` | Build `RuntimeContext` during app setup |
| `src-tauri/src/commands/mod.rs` | May slim down `AppState` |
| `src-tauri/src/agent/squire/adapter.rs` | Review — may be simplified |
| `src-tauri/src/agent/squire/types.rs` | May add new event types |

## Event Emitter Trait

Define in `engine-core`:
```rust
#[async_trait]
pub trait EventEmitter: Send + Sync {
    async fn emit(&self, event: EngineEvent) -> Result<()>;
}
```

This decouples the engine from Tauri's event system. The real app provides a `TauriEventEmitter` that wraps `AppHandle::emit()`. Tests provide a `RecordingEventEmitter`.

## Commands

```powershell
.\scripts\build.ps1
.\scripts\test-all.ps1
```
