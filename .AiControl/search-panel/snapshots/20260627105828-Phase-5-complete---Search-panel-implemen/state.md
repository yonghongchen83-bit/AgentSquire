# Search Panel — Phase 5 Complete

## What was implemented

### Frontend
- **`src/components/search-panel.tsx`** — Full search panel UI:
  - Search input with Enter-to-search and button trigger
  - Replace input with "Replace All" button (collapsible)
  - Options toggles: regex (`.*`), case-sensitive (`Ab`), whole word (`W`)
  - Glob filter input and context lines input (collapsible options panel)
  - Results tree grouped by file with match count per file
  - Expand/collapse per-file results (ChevronRight/ChevronDown)
  - Click result → open file in Monaco at line number
  - Loading spinner during search
- **`src/stores/search-store.ts`** — Zustand store: query, replaceText, path, options, search results groups
- **`src/stores/editor-store.ts`** — Added `gotoLine` / `setGotoLine` for jumping to search result lines
- **`src/components/monaco-wrapper.tsx`** — Added `useEffect` for `gotoLine`: reveals line in center and sets cursor
- **`src/components/left-side-panel.tsx`** — Replaced `SearchPlaceholder` with `SearchPanel`
- **`src/lib/ipc.ts`** — Added `searchFiles()` and `replaceInFiles()` IPC wrappers
- **`src/types/ipc.ts`** — Fixed `SearchMatch` to match Rust's snake_case fields, added `ReplaceOptions`

### Backend (Rust)
- **`src-tauri/src/commands/mod.rs`** — Added `replace_in_files` Tauri command wrapping `grep::grep_replace()`
- **`src-tauri/src/lib.rs`** — Registered `replace_in_files` in invoke_handler

### Files changed: 9
### Build: lint + tsc + cargo check all pass
