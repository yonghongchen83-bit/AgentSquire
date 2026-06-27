# Decisions — Phase 2

Decisions made during app shell implementation.

| # | Decision | Rationale |
|---|----------|-----------|
| 1 | **Custom file tree instead of momoi-explorer** | The momoi-explorer npm package may not be available/updated. Built a custom tree using IPC `list_directory` with expand/collapse, which provides same functionality without external dependency risk. Can swap to momoi-explorer later if needed. |
| 2 | **Custom resizable.tsx adapted for react-resizable-panels v4** | shadcn's resizable component was generated for an older version. v4 exports `Group`, `Panel`, `Separator` instead of `PanelGroup`, `PanelResizeHandle`. Updated the component to match. |
| 3 | **shadcn components moved from @/ to src/components/ui/** | Initial scaffold created components at project root `@/components/ui/` mismatch with `components.json` aliases pointing to `./src/*`. Moved to correct location for working `@/` alias resolution. |
| 4 | **Monaco editor as default, not placeholder contentEditable** | Phase 2.2 called for `@monaco-editor/react` integration. Implemented directly instead of using a contentEditable placeholder, with IPC `read_file` for loading and language detection from extension. |
| 5 | **xterm.js with FitAddon for terminal** | Terminal uses @xterm/xterm v6 with FitAddon for auto-resize. Dark theme background (#1A2332) with green foreground for terminal feel. |
