# State — UI Layout Compliance Fixes

## Overall Status: ✅ Complete

All 10 phases of compliance fixes are implemented and verified (TypeScript + Vite build pass clean).

### Fixed Items (15 critical gaps closed)

| Gap | Fix | File(s) |
|-----|-----|---------|
| Menu Bar missing | New File/Edit/View/Help component | `menu-bar.tsx`, `App.tsx` |
| Window controls decorative | Functional minimize/maximize/close via Tauri API | `title-bar.tsx` |
| Bottom panel in wrong layout | Moved to outer level, full-width | `App.tsx` |
| No terminal IPC | Multi-tab xterm + spawn/write/resize/kill stubs | `xterm-terminal.tsx`, `lib/ipc.ts` |
| No multi-terminal tabs | Tab bar with close/+ buttons | `xterm-terminal.tsx` |
| Output panel placeholder | Source dropdown + timestamped entries + event listener | `bottom-panel.tsx` |
| Errors panel placeholder | Structured list + severity + event listener | `bottom-panel.tsx` |
| No resize persistence | Debounced onLayout → saveConfig | `App.tsx` |
| Most keyboard shortcuts missing | 15 shortcuts implemented | `keyboard-shortcuts.tsx` |
| Tab bar missing features | Max 15, pin tabs, right-click menu | `editor-store.ts`, `tab-bar.tsx` |
| Code blocks missing actions | Copy/Apply/Diff buttons | `chat-blocks.tsx` |
| Welcome screen bare | Recent projects + config/test links | `welcome-screen.tsx` |
| File tree missing Copy Path | Added to context menu | `file-tree.tsx` |
| Chat send on plain Enter | Changed to Ctrl+Enter | `chat-input.tsx` |
| Search missing context_after | Added display | `search-panel.tsx` |

### Remaining Polish Items (not blocking)

| Item | Reason Left |
|------|-------------|
| Tab drag-to-reorder | reorderTabs store method exists, UI drag not wired |
| Menu item actions (Save, Edit ops, Help) | UI stubs — need Monaco editor API wiring + backend |
| Alt-key menu navigation | Requires focus tracking |
| git_diff IPC | Phase 6 per design, low priority |
| Extensions sidebar icon | Post-MVP per design |

### Build Verification
- `npx tsc --noEmit` — ✅ zero errors
- `npx vite build` — ✅ builds clean (chunk-size warning only)
