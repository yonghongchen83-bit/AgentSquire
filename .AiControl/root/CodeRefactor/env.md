# Env

## Workspace
- Root: D:/work/MyAgent
- Frontend: React + Vite + TypeScript in src
- Backend: Tauri + Rust in src-tauri

## Relevant Paths
- src-tauri/src/commands/mod.rs
- src/components/settings-dialog.tsx
- src/stores/chat-store.ts
- src/lib/ipc.ts
- src-tauri/src/llm/openai.rs
- src-tauri/src/llm/anthropic.rs

## Constraints
- Keep behavior stable while refactoring.
- Work in small, reviewable batches.
- Preserve compatibility with existing config and IPC payload shape unless explicitly migrated.
- Validate each batch with lint/build/tests.

## Known Hotspots
- Oversized files and mixed responsibilities.
- IPC naming mismatches between frontend invoke payloads and backend command signatures.
- Event naming mismatch around file watcher notifications.

