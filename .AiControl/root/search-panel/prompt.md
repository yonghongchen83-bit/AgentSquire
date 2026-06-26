# Prompt — Phase 5: Search Panel

Build the search-in-files panel. ripgrep backend, custom React UI.

## Steps (from implementation-plan.md)
5.1 Search panel layout — search input, replace input, options toggles
5.2 IPC grep_search command — calls ripgrep, returns structured results
5.3 Results tree — grouped by file, context lines, match highlighting
5.4 Click to open — double-click opens file in Monaco at line
5.5 Replace — replace field + Replace All via IPC
5.6 Collapse/expand per-file results
