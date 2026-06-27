---
id: task-004
title: Search Panel Searches Project Directory, Not App CWD
priority: high
status: pending
---

## Description

The Search panel should search within the currently opened project directory, not the application's current working directory (`.`). This is currently **broken** — the search panel uses its own `path` state from `useSearchStore` (defaults to `''`), and falls back to `'.'` when empty, which searches the app's CWD instead of the project path.

## Test Cases

1. **Search uses project path**: When `projectPath` is set in layout store and `search-store.path` is empty, search should default to `projectPath`
2. **Search respects explicit path override**: If user sets a custom path in search options, that path takes precedence
3. **Results are from project dir**: Searching for a term returns files only from within the project directory (not from the app root)

## Selectors
- Search input: `input[placeholder="Search"]`
- Search button: The search icon button (lucide `Search` icon)
- Search results: Result groups with `FileText` icon and file path text
- Path input: The options panel path field (if exposed — currently `path` from `search-store` is NOT exposed in the UI!)

## Important Notes
- **BUG**: `SearchPanel` at `src/components/search-panel.tsx:57` uses `path || '.'` where `path` comes from `useSearchStore(s => s.path)` (default `''`)
- **FIX REQUIRED**: Search panel should use `useLayoutStore((s) => s.projectPath)` as the fallback when `search-store.path` is empty
- The path field is stored in `search-store` but has no UI input — the user cannot see or change it
- After fixing, search should default to the project path when no explicit path is set
- The `Replace All` function at line 85 also has the same bug (`path || '.'`)
