# Prompt — Phase 0.5: UI Layout Design

Design the full layout before writing any shell code. This is the blueprint everything builds to.

## Steps
1. Decide panel layout: VS Code single-panel (sidebar + editor center + bottom terminal) vs split layout (chat right, editor left)
2. Wireframe the 4 zones: Sidebar (file tree / search), Editor area (tabs + Monaco), Chat panel (messages + composer), Bottom panel (terminal)
3. Define React component tree: `<App>` → `<Sidebar>` + `<MainArea>` + `<ChatPanel>` + `<BottomPanel>`
4. IPC contract review — walk every IPC command and verify it serves all panel needs
5. State ownership matrix — what lives in Zustand vs Rust vs TanStack Query cache
6. Resize persistence — panel ratios saved in config
7. Write `ArchitecturePlanning/layout-design.md`

## Key questions
- Single window or multi-window? (Tauri supports both)
- Chat as a side panel or main focus?
- File tree + search — tab-switch or both visible?
- Terminal — always visible at bottom or toggle?
