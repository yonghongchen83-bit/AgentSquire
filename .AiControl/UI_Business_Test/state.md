<!-- from UI_Business_Test -->
# Current State

## Tasks Verified
- **Task-001**: Side Panel Resize ✅ — 4/4 passing
- **Task-002**: Open Project Sets Workspace Path ✅ — 4/4 passing
- **Task-003**: File Explorer ✅ — 3/3 passing
- **Task-004**: Search Panel ✅ — 5/5 passing
- **Task-005**: Bottom Panel ✅ — 2/2 passing

## Changes Made

### IPC Command Name Fix (Rust backend)
Renamed Tauri v2 commands to match frontend `invoke()` calls — the `cmd_` prefix caused all file IPC to fail silently:

| File | Change |
|------|--------|
| `src-tauri/src/commands/mod.rs` | `cmd_list_directory` → `list_directory`, `cmd_read_file` → `read_file`, `cmd_write_file` → `write_file`, `cmd_create_directory` → `create_dir`, `cmd_delete_item` → `delete_item`, `cmd_rename_item` → `rename_item` |
| `src-tauri/src/lib.rs` | Updated `generate_handler!` to use new function names |
| `src-tauri/src/commands/mod.rs` | `git_status`: made `path` optional (`Option<String>`), default to `"."` |
| `src-tauri/src/fs/git.rs` | `GitStatus.path` renamed to `file` via `#[serde(rename = "file")]` |
| `src/lib/ipc.ts` | `gitStatus()` return type `Promise<string>` → `Promise<{file;status}[]>`, accepts optional `path` arg |
| `src/components/file-tree.tsx` | Removed `JSON.parse(gitStatus())`, pass `projectPath` to `gitStatus()` |

### Xterm Crash Fix
- `src/components/xterm-terminal.tsx`: Wrapped `onTerminalOutput`/`onTerminalExit` in try/catch; guarded `.unlisten` calls; wrapped cleanup in try/catch

### File Tree UX
- `src/components/file-tree.tsx`: No longer falls back to `'.'` when `projectPath` is empty. Shows "No project open", "Unable to list directory", or "Empty directory" messages. Fixed `onFsChange` cleanup (was a memory leak).

### LLM Model Configuration
- `src/components/settings-dialog.tsx`: Added provider type selector with presets (ChatGPT, Anthropic, Google Gemini, Ollama) that auto-fill model/endpoint. Added "Test Connection" button per provider.
- `src-tauri/src/commands/mod.rs`: Added `test_connection` Tauri command
- `src-tauri/src/lib.rs`: Registered `test_connection` in handler
- `src/lib/ipc.ts`: Added `testConnection()` IPC wrapper

### Tests Updated
- `e2e/specs/task-005-bottom-panel.test.ts`: Removed store manipulation — now clicks real "Show Terminal" button (xterm fix verified)
- `e2e/specs/task-003-file-explorer.test.ts`: Removed `"(empty due to IPC)"` caveat — asserts tree has content or error message

## Workflow Rule Added: Unconditional Bug Reproduction
- Added unconditional rule: **Reproduce bug before any planning or fixing** — this is non-negotiable, applies to ALL bug-fixing scenarios without exception.
- `skill.md`: Added unconditional reproduce rule to the Workflow section
- `env.md`: Updated Workflow Rules to reflect unconditional nature

## Next Todo
- **[P0] Reproduce bug before planning and fixing** — This is now an unconditional requirement. Before any bug fix: (1) reproduce first, (2) then plan, (3) then fix. No exceptions.

## Environment Updated
- `env.md`: Added workflow rules summary and opencode zen free aipkey

## Lessons Learned
See [lessons-learned index](../lessons-learned/lessons.md) for detailed write-ups of key engineering lessons from this session:
- [001](../lessons-learned/001-vite-server-survival.md) — Vite dev server dies when shell tool times out
- [002](../lessons-learned/002-tauri-command-naming.md) — Tauri `cmd_` prefix mismatch breaks IPC
- [003](../lessons-learned/003-tests-bypass-ipc.md) — Tests that bypass IPC give false confidence