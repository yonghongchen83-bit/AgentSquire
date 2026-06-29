# Lesson 002: Tauri IPC Commands Silently Fail (cmd_ Prefix Mismatch)

## Problem

Frontend calls to `invoke('list_directory', ...)`, `invoke('read_file', ...)`, etc. all fail. The file tree shows "Unable to list directory". File reads silently return nothing. All file operations are broken.

## Symptoms

- File tree shows `"Unable to list directory"` error message
- Monaco editor can't load file contents (content stays empty)
- No errors in browser console (the IPC call fails silently because it's in try/catch)
- StatusBar doesn't show git status

## Root Cause

Tauri v2 registers commands by **exact Rust function name**. The codebase used `cmd_list_directory` as the Rust function name, but the frontend called `invoke('list_directory')` — missing the `cmd_` prefix.

**Affected functions:**

| Rust function (old) | Frontend `invoke()` call | Result |
|---|---|---|
| `cmd_list_directory` | `list_directory` | ❌ Not found |
| `cmd_read_file` | `read_file` | ❌ Not found |
| `cmd_write_file` | `write_file` | ❌ Not found |
| `cmd_create_directory` | `create_dir` | ❌ Not found |
| `cmd_delete_item` | `delete_item` | ❌ Not found |
| `cmd_rename_item` | `rename_item` | ❌ Not found |

Additional mismatches:
- **`git_status`**: Rust required `path: String` but frontend called it with no args → missing parameter error. Returned `Vec<GitStatus>` (field: `path`) but frontend expected `string` and accessed `item.file` → wrong field name.
- **`ChatRequest.tools`**: Type was `Vec<ToolDefinition>` but new code tried `tools: None` → compilation error.

## Fix

1. **Renamed Rust functions** to match frontend calls (removed `cmd_` prefix):

   ```rust
   // Before (commands/mod.rs)
   #[tauri::command]
   pub fn cmd_list_directory(path: String) -> Result<Vec<FileEntry>, String> { ... }

   // After
   #[tauri::command]
   pub fn list_directory(path: String) -> Result<Vec<FileEntry>, String> { ... }
   ```

2. **Updated `lib.rs`** handler registration to use new names:

   ```rust
   .invoke_handler(tauri::generate_handler![
       commands::list_directory,  // was: commands::cmd_list_directory
       ...
   ])
   ```

3. **Fixed `git_status`** — made `path` optional, added `#[serde(rename = "file")]` on struct field, fixed return type in `ipc.ts`.

## Prevention

1. **Naming convention**: When adding a new Tauri command, the Rust function name IS the command name. No prefixes, no suffixes. `fn list_directory` → `invoke('list_directory')`.
2. **Parameter matching**: Use `Option<T>` for optional parameters. Match the frontend call signature exactly.
3. **Return type alignment**: Verify the Rust return type matches what the frontend expects. If Rust returns `Vec<X>`, the frontend gets a JavaScript array — no need for `JSON.parse`.
4. **Test early**: After adding a command, call it directly via `browser.execute(() => window.__TAURI_INTERNALS__.invoke(...))` in a test to verify it works before building UI around it.

## Related

- [Tauri v2 command documentation](https://v2.tauri.app/develop/calling-rust/)
- [Lesson 003](./003-tests-bypass-ipc.md) — how to properly test IPC commands
