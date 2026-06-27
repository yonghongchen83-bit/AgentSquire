## Compliance Fixes Completed

### Phase 1: Trivial UI Fixes
- File tree: Added "Copy Path" to right-click context menu (`file-tree.tsx:116`)
- Chat input: Changed send trigger from Enter to Ctrl+Enter (`chat-input.tsx:29`)
- Search results: Added `context_after` display alongside `context_before` (`search-panel.tsx:260`)
- Title bar: Replaced decorative circles with functional minimize/maximize/close buttons using `@tauri-apps/api/window`; switched hamburger to Menu icon (`title-bar.tsx`)

### Phase 2: Keyboard Shortcuts
Added all missing shortcuts: Ctrl+Shift+E (explorer), Ctrl+Shift+G (git), Ctrl+Shift+\` (terminal), Ctrl+W (close tab), Ctrl+Tab/Shift+Tab (switch tab), Ctrl+=/-/0 (zoom), Ctrl+\\ (toggle right panel), Escape (blur). (`keyboard-shortcuts.tsx`)

### Phase 3: Welcome Screen
- Recent projects list (stored in localStorage, max 10)
- "Model Configuration" and "Test Connection" links that open Settings dialog (`welcome-screen.tsx`)

### Phase 4: Code Block Actions
Added Copy (with confirmation), Apply, and Diff buttons to code blocks. Copy uses clipboard API. (`chat-blocks.tsx`)

### Phase 5: Tab Bar Improvements
- Max 15 tab limit enforced in editor store
- Right-click context menu: Close (disabled for pinned), Close Others, Close All, Pin/Unpin Tab, Copy Path
- Pin tabs with Pin icon, prevent close on pinned tabs, survive Close Others/Close All
- Dirty indicator preserved (`tab-bar.tsx`, `editor-store.ts`)

### Phase 6: Menu Bar
New component with File/Edit/View/Help menus. Each menu has proper dropdown items with keyboard shortcuts displayed. Click-to-open, hover-to-switch between menus, click-outside to close. (`menu-bar.tsx`)

### Phase 7: Bottom Panel Layout
Restructured App.tsx: bottom panel moved from vertical split inside editor panel to the outer layout level. Now spans full window width under both editor AND right panel, matching the ASCII wireframe. Conditionally rendered when visible. (`App.tsx`)

### Phase 8: Output & Errors Panels
- OutputPanel: source dropdown (stdout/debug/notifications), timestamped entries, real-time append via `output:append` event
- ErrorsPanel: structured list with severity dots (red/amber/blue), source, timestamp, stack trace, real-time via `error:new` event
- IPC stubs: `getOutput()`, `getErrors()`, `onOutputAppend()`, `onErrorNew()` (`bottom-panel.tsx`, `lib/ipc.ts`)

### Phase 9: Terminal Multi-Tab + IPC
- Multi-terminal tabs with tab bar, close button, `+` button to spawn new
- Wired xterm onData → `writeStdin()` IPC, onResize → `resizePty()` IPC
- Listens for `terminal:output` and `terminal:exit` events
- Auto-creates first terminal on mount (`xterm-terminal.tsx`)
- IPC stubs: `listTerminals()`, `spawnTerminal()`, `writeStdin()`, `resizePty()`, `killTerminal()`, `onTerminalOutput()`, `onTerminalExit()` (`lib/ipc.ts`)

### Phase 10: Events Wiring + Resize Persistence
- `fs:change` event listener wired to auto-refresh file tree
- Resize persistence: `onLayout` callbacks on ResizablePanelGroups, debounced (500ms) `saveConfig()` call
- `onFsChange()` IPC stub added (`App.tsx`, `file-tree.tsx`)
