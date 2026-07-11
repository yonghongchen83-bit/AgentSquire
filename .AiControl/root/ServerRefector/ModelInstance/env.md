# Env — ModelInstance

## Key Files

| File | Action |
|------|--------|
| `src-tauri/crates/provider-core/src/lib.rs` | Add `ModelInstance`, `ModelOptions` structs |
| `src-tauri/crates/provider-core/src/lib.rs` | Update `LlmProvider::chat()` signature |
| `src-tauri/crates/provider-openai/src/lib.rs` | Update to use `ModelInstance` |
| `src-tauri/crates/provider-anthropic/src/lib.rs` | Update to use `ModelInstance` |
| `src-tauri/crates/provider-registry/src/lib.rs` | Add `resolve_model_instance()` |
| `src-tauri/src/commands/streaming_cmd.rs` | Update orchestration to build `ModelInstance` |
| `src-tauri/src/commands/providers_cmd.rs` | Update IPC handlers if needed |
| `src/types/ipc.ts` | Update TypeScript types |
| `src/stores/chat-store/` | Update frontend to send structured model selection |

## Commands

```powershell
.\scripts\build.ps1
.\scripts\test.ps1
```
