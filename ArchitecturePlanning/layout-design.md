# UI Layout Design — VS Code-Inspired Panel Layout

> Phase 0.5 deliverable. Decided: VS Code-style single-window with resizable panels.

---

## 1. Panel Layout (ASCII Wireframe)

```
┌─────────────────────────────────────────────────────────────────┐
│  ≡ ─ MyAgent                                    _ □ ✕          │  (Title Bar)
├─────────────────────────────────────────────────────────────────┤
│  File  Edit  View  Help                                         │  (Menu Bar)
├─────────────────────────────────────────────────────────────────┤
│  ┌─────┐ ┌─────────────────────┐ ┌──────────────┐ ┌─────────┐  │
│  │     │ │                     │ │              │ │         │  │
│  │  S  │ │                     │ │   Chat       │ │    R    │  │
│  │  i  │ │    Editor /         │ │   (Right     │ │    i    │  │
│  │  d  │ │    Viewer /         │ │   Side       │ │    g    │  │
│  │  e  │ │    Doc Area         │ │   Panel)     │ │    h    │  │
│  │  b  │ │                     │ │              │ │    t    │  │
│  │  a  │ │   (Center Area)     │ │  Message     │ │         │  │
│  │  r  │ │                     │ │  List        │ │         │  │
│  │     │ │                     │ │              │ │         │  │
│  │  I  │ │                     │ │  Composer    │ │         │  │
│  │  c  │ │                     │ │              │ │         │  │
│  │  o  │ │                     │ │              │ │         │  │
│  │  n  │ │                     │ │              │ │         │  │
│  │  s  │ │                     │ │              │ │         │  │
│  │  ─── │ │                     │ │              │ │         │  │
│  │     │ │                     │ │              │ │         │  │
│  │  A  │ │                     │ │              │ │         │  │
│  │  c  │ │                     │ │              │ │         │  │
│  │  t  │ │                     │ │              │ │         │  │
│  │  i  │ │                     │ │              │ │         │  │
│  │  v  │ │                     │ │              │ │         │  │
│  │  i  │ │                     │ │              │ │         │  │
│  │  t  │ │                     │ │              │ │         │  │
│  │  y  │ │                     │ │              │ │         │  │
│  │     │ │                     │ │              │ │         │  │
│  └─────┘ └─────────────────────┘ └──────────────┘ └─────────┘  │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │  Bottom Panel (Terminal / Output / Errors)              │    │
│  │  ┌─────┬──────┬───────┬──────────┬─────────┐           │    │
│  │  │Term1│Term2 │Output│  Errors  │   +     │  (tabs)   │    │
│  │  ├─────┴──────┴───────┴──────────┴─────────┘           │    │
│  │  │  stdout / stderr / debug console / notifications     │    │
│  │  │  [Source: ▼] (dropdown: stdout, debug, errors, ...)  │    │
│  └─────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────┘
```

---

## 2. Zone Definitions

### Zone 0: Title Bar — Topmost
- **Height:** ~32px (fixed, non-resizable)
- **Purpose:** Custom HTML title bar with window controls and app icon
- **Content:** Hamburger menu `≡` (left), project name "MyAgent" (center), window control buttons `— □ ✕` (right)
- **Behavior:** Draggable region for window move. Double-click to maximize/restore. Custom HTML title bar for consistent look with light blue theme across all platforms.

### Zone 0b: Menu Bar — Top
- **Height:** ~28px (fixed, non-resizable)
- **Purpose:** Application menu — open/close projects, settings, help, etc.
- **Menus:**
  | Menu | Items |
  |------|-------|
  | **File** | Open Project, Close Project, Save (Ctrl+S), Save As..., Recent Projects ▸, Exit |
  | **Edit** | Undo, Redo, Cut, Copy, Paste, Find, Replace |
  | **View** | Toggle Left Panel (Ctrl+B), Toggle Right Panel, Toggle Bottom Panel (Ctrl+`), Zoom In, Zoom Out, Reset Zoom |
  | **Help** | About, Documentation, Report Issue, Check for Updates |
- **Behavior:** Click opens dropdown. `Alt` key activates menu bar focus.

### Zone 1: Sidebar (Activity Bar) — Far Left
- **Width:** ~48px (fixed, non-resizable)
- **Purpose:** Icon-based navigation to toggle left side panel views
- **Icons (top-down):**
  - File Explorer (Ctrl+Shift+E)
  - Search (Ctrl+Shift+F)
  - Git (Ctrl+Shift+G)
  - Extensions (placeholder, post-MVP)
- **Bottom:** Settings gear icon
- **Behavior:** Clicking an icon toggles the left side panel. Active icon is highlighted. Only one view active at a time.

### Zone 2: Left Side Panel
- **Width:** ~250-350px (resizable via splitter with center area)
- **Views (tab-switched via sidebar icons):**
  - **File Explorer** — momoi-explorer file tree, with context menus (new file/folder, rename, delete, reveal)
  - **Search in Files** — ripgrep-based search: input field, replace field, regex/whole-word/case toggles, glob filters, results tree grouped by file
  - **Git** — git status, staged/unstaged changes, branch selector (Phase 6)
- **Behavior:** Can be collapsed by clicking the active sidebar icon again or via Ctrl+B. Only one view visible at a time.

### Zone 3: Center Area (Editor / Viewer / Doc)
- **Behavior:** Fills remaining horizontal space between left panel and right panel
- **Content:**
  - **Tab bar** at top — open files positioned as tabs (max 15, scrollable when overflow — VS Code-style horizontal scrolling). Active tab highlighted. Drag to reorder. Close button on hover (X). Dirty indicator dot. **Pinnable** — right-click → Pin Tab keeps it in place, prevents closing.
  - **Monaco Editor** — the active tab's content. Language detection from extension. Read-only mode for non-editable files.
  - **Diff view** — Monaco built-in diff when comparing versions (Phase 4+)
  - **Empty state (Welcome Screen)** — app logo, **Open Project** button, recent projects list, link to **Model Configuration** / **Test Connection**

### Zone 4: Right Side Panel (Chat)
- **Width:** ~300-450px (resizable via splitter with center area)
- **Content:**
  - **Message list** — scrollable history of conversation messages
  - **Thinking blocks** — collapsible, animated while streaming
  - **Tool call blocks** — expandable cards showing tool name, args, result
  - **Code blocks** — mini Monaco read-only with copy/apply/diff buttons
  - **Composer** — text input at bottom, send button, Ctrl+Enter to send
- **Behavior:** Can be collapsed to reclaim space for editor. Right panel is exclusive to chat (no view switching).

### Zone 5: Bottom Panel (Terminal / Output / Errors)
- **Height:** ~150-300px (resizable via splitter with center area)
- **Tabs:**
  | Tab | Content | Behavior |
  |-----|---------|----------|
  | **Terminal** | xterm.js instances | Multiple tabs, each a separate shell process. `+` button spawns new terminal. Close button on each tab. |
  | **Output** | stdout/stream output | Dropdown to select source: stdout, debug console, notifications. Output is read-only, scrollable. |
  | **Errors** | Error messages | Structured error list (not freeform output). Severity levels, timestamps, clickable source links. |
- **Behavior:** Can be toggled via Ctrl+`. Height persisted across restarts.

---

## 3. Key Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Single vs multi-window | **Single-window** | VS Code model. All panels in one window with resizable splitters. Multi-window can be added post-MVP. |
| Chat location | **Right side panel** | Dedicated panel, not mixed with editor or terminal. Familiar to users who use chat-aside extensions. |
| File tree vs search | **Tab-switch via sidebar icons** | Same as VS Code. Only one left panel view at a time to save horizontal space. |
| Terminal visibility | **Toggle with Ctrl+`** | Bottom panel is collapsible. Always visible when active, hidden when not needed. |
| Resize persistence | **Save ratios to config (TOML)** | `state/config.rs` stores panel sizes. Restored on app restart. |
| Multiple terminals | **Tab bar in bottom panel** | `+` button creates new xterm.js instance. Each tab is an independent shell. |
| Output source selection | **Dropdown in Output tab** | User selects stdout, debug console, notifications, or errors. Similar to VS Code's "Output" channel selector. |
| Menu bar implementation | **Custom HTML menu bar** | Cross-platform consistency. Native OS menu via Tauri is an option but custom HTML gives us full control over styling and behavior (light blue theme, keyboard nav, etc.). |
| Title bar | **Custom HTML title bar** | Consistent look with light blue theme. Hamburger menu + project name + native-style window controls (— □ ✕). |
| File icons | **Icon library (file-icons/seti)** | Visual file type recognition in file tree. Lightweight icon font or SVG set. |
| Welcome screen | **Logo + Open Project + Recent Projects + Model Config link** | Quick onboarding — open project, reconfigure LLM, test connection. All on the welcome page. |
| Tab limit | **Max 15 tabs**, scrollable when overflow | Prevents tab bar from becoming unmanageable. VS Code-style horizontal scroll on overflow. |
| Tab close behavior | **Close button (X) on hover only** | Close via X button. No double-click. |
| Pin tabs | **Pinnable** | Right-click → Pin Tab. Pinned tabs are smaller, stay in place, can't be closed by accident, survive "Close Others". Unpin via right-click. |
| Panel collapse | **Hide only, re-expandable, no process kills** | Collapsing right/bottom panel just hides the view. Terminals stay alive, chat state preserved. |
| Minimum window size | **600px width minimum** | Prevents panels from being squished below usability threshold. |
| Theme | **Light blue** | Default theme. Avoids eye fatigue from dark themes. Customizable later. |
| Font zoom | **In status bar: − 100% +** | Global UI font size adjustment. Persisted in config. Also available in View menu. |
| Status bar items | **LLM connection + notification center + cursor position + font zoom** | Show connection status to LLM provider, notification bell, current cursor line:col, and quick font size control. |
| Context menus (file tree) | **Standard** | New File, New Folder, Rename, Delete, Copy Path, Reveal in Explorer |
| Context menus (editor tabs) | **Standard** | Close, Close Others, Close All, Copy Path |

---

## 4. React Component Tree

```
<App>
├── <TitleBar>                         // Custom HTML title bar (~32px fixed)
│   ├── <AppMenuButton />              // Hamburger ≡
│   ├── <TitleText label="MyAgent" />  // Project name
│   └── <WindowControls />             // Minimize, Maximize, Close
│
├── <MenuBar>                          // Top menu bar (~28px fixed)
│   ├── <Menu label="File">
│   │   ├── <MenuItem label="Open Project" shortcut="Ctrl+O" />
│   │   ├── <MenuItem label="Close Project" />
│   │   ├── <Separator />
│   │   ├── <MenuItem label="Save" shortcut="Ctrl+S" />
│   │   ├── <MenuItem label="Save As..." />
│   │   ├── <Separator />
│   │   ├── <Menu label="Recent Projects">
│   │   │   └── <MenuItem />*
│   │   └── <MenuItem label="Exit" />
│   ├── <Menu label="Edit">
│   │   ├── <MenuItem label="Undo" />
│   │   ├── <MenuItem label="Redo" />
│   │   ├── <Separator />
│   │   ├── <MenuItem label="Cut" />
│   │   ├── <MenuItem label="Copy" />
│   │   └── <MenuItem label="Paste" />
│   ├── <Menu label="View">
│   │   ├── <MenuItem label="Toggle Left Panel" shortcut="Ctrl+B" />
│   │   ├── <MenuItem label="Toggle Right Panel" />
│   │   ├── <MenuItem label="Toggle Bottom Panel" shortcut="Ctrl+`" />
│   │   ├── <Separator />
│   │   ├── <MenuItem label="Zoom In" />
│   │   ├── <MenuItem label="Zoom Out" />
│   │   └── <MenuItem label="Reset Zoom" />
│   └── <Menu label="Help">
│       ├── <MenuItem label="About" />
│       ├── <MenuItem label="Documentation" />
│       ├── <MenuItem label="Report Issue" />
│       └── <MenuItem label="Check for Updates" />
│
├── <Sidebar>                          // Activity bar (48px fixed)
│   ├── <SidebarIcon icon="explorer" />
│   ├── <SidebarIcon icon="search" />
│   ├── <SidebarIcon icon="git" />
│   ├── <Spacer />
│   └── <SidebarIcon icon="settings" />
│
├── <ResizablePanelGroup direction="horizontal">
│   │
│   ├── <LeftSidePanel>                // Collapsible, 250-350px
│   │   ├── <FileExplorer>             // momoi-explorer tree
│   │   │   ├── <TreeHeader />         // "EXPLORER" label + buttons
│   │   │   ├── <TreeView />           // momoi-explorer React bindings
│   │   │   └── <ContextMenu />        // Right-click menu
│   │   ├── <SearchPanel>              // ripgrep search
│   │   │   ├── <SearchInput />
│   │   │   ├── <ReplaceInput />
│   │   │   ├── <SearchOptions />      // Regex, case, whole-word toggles
│   │   │   └── <SearchResults />
│   │   └── <GitPanel>                // Future: git status
│   │
│   ├── <ResizableHandle />
│   │
│   ├── <MainArea>                     // Center — fills remaining width
│   │   ├── <TabBar>
│   │   │   └── <Tab />*              // Open file tabs
│   │   ├── <EditorArea>
│   │   │   └── <MonacoEditor />       // Or <DiffEditor /> or empty state
│   │   └── <WelcomeScreen />          // When no tabs open
│   │
│   ├── <ResizableHandle />
│   │
│   ├── <RightSidePanel>               // Chat, 300-450px, collapsible
│   │   ├── <ChatHeader />             // Conversation title + actions
│   │   ├── <MessageList>
│   │   │   ├── <Message />*           // Block-based rendering
│   │   │   │   ├── <TextBlock />
│   │   │   │   ├── <ThinkingBlock />
│   │   │   │   ├── <ToolCallBlock />
│   │   │   │   ├── <CodeBlock />
│   │   │   │   └── <ErrorBlock />
│   │   │   └── <ScrollAnchor />
│   │   └── <Composer>
│   │       ├── <TextArea />           // Auto-resizing
│   │       └── <SendButton />
│   │
│   └── <ResizableHandle />
│
├── <BottomPanel>                      // Collapsible, 150-300px
│   ├── <BottomTabBar>
│   │   ├── <BottomTab label="Terminal" />
│   │   ├── <BottomTab label="Output" />
│   │   └── <BottomTab label="Errors" />
│   ├── <TerminalPanel>
│   │   ├── <TerminalTabBar>
│   │   │   ├── <TerminalTab />*       // One per shell
│   │   │   └── <AddTerminalButton />
│   │   └── <XTerm />                  // xterm.js instance
│   ├── <OutputPanel>
│   │   ├── <OutputSourceDropdown />   // stdout, debug, notifications
│   │   └── <OutputContent />
│   └── <ErrorsPanel>
│       └── <ErrorList />
│
└── <StatusBar>                        // Thin bar at bottom (~24px)
    ├── <LlmConnectionIndicator />     // Connected/disconnected status
    ├── <NotificationCenter />         // Bell icon with badge
    ├── <Spacer />
    ├── <CursorPosition />             // Line:col
    ├── <Separator />
    ├── <ZoomControl />                // − 100% +  (font zoom in/out)
```

### Component Props & State Shapes

```typescript
// --- Layout State (Zustand) ---
interface LayoutStore {
  leftPanelVisible: boolean;
  leftPanelActiveView: 'explorer' | 'search' | 'git';
  leftPanelWidth: number;
  rightPanelVisible: boolean;
  rightPanelWidth: number;
  bottomPanelVisible: boolean;
  bottomPanelHeight: number;
  bottomPanelActiveTab: 'terminal' | 'output' | 'errors';
  uiFontZoom: number; // 75–150, step 5, default 100
}

// --- Status Bar State (Zustand) ---
interface StatusBarStore {
  llmConnected: boolean;
  llmProvider: string;       // e.g. "OpenAI", "Anthropic"
  notificationCount: number;
  cursorLine: number;
  cursorCol: number;
  uiFontZoom: number;
}

// --- Tab State (Zustand) ---
interface TabStore {
  tabs: EditorTab[];
  activeTabId: string | null;
  openFile: (path: string) => void;
  closeTab: (id: string) => void;
  reorderTabs: (from: number, to: number) => void;
  setActiveTab: (id: string) => void;
}

interface EditorTab {
  id: string;
  path: string;
  filename: string;
  language: string;
  isDirty: boolean;
  isLoading: boolean;
}

// --- Terminal State (Zustand) ---
interface TerminalStore {
  terminals: TerminalInstance[];
  activeTerminalId: string;
  addTerminal: () => void;
  closeTerminal: (id: string) => void;
  setActiveTerminal: (id: string) => void;
}

interface TerminalInstance {
  id: string;
  label: string; // "Terminal 1", "Terminal 2", etc.
  shellPath: string;
}

// --- Editor State (TanStack Query / IPC) ---
// File content fetched via: useQuery({ queryKey: ['file', path], queryFn: () => invoke('read_file', { path }) })
// File save via: useMutation({ mutationFn: ({ path, content }) => invoke('write_file', { path, content }) })

// --- Chat State (TanStack Query / IPC) ---
// Conversations listed via: useQuery({ queryKey: ['conversations'], queryFn: () => invoke('list_conversations') })
// Messages via: useQuery({ queryKey: ['conversation', id], queryFn: () => invoke('get_conversation', { id }) })
// Send via: useMutation which triggers streaming events from Rust
```

---

## 5. IPC Contract Review

| IPC Command | Direction | Serves | Notes |
|-------------|-----------|--------|-------|
| `list_directory(path)` | FE→Rust | **File Explorer** | Returns `FileEntry[]`. Must include file type, size, modified time. |
| `read_file(path)` | FE→Rust | **Editor** | Returns string content. Monospace threshold? |
| `write_file(path, content)` | FE→Rust | **Editor (save)** | Returns success/error. |
| `create_dir(path)` | FE→Rust | **File Explorer** | For "New Folder" action. |
| `delete_item(path)` | FE→Rust | **File Explorer** | Trash vs permanent delete configurable. |
| `rename_item(old, new)` | FE→Rust | **File Explorer** | Handles name collision. |
| `grep(query, options)` | FE→Rust | **Search Panel** | Returns `SearchMatch[]` grouped by file. Stream results? |
| `grep_replace(query, replacement, options)` | FE→Rust | **Search Panel** | Returns diff, applies changes. |
| `git_status()` | FE→Rust | **Git Panel, File Explorer** | Status dots on file tree. |
| `git_diff(path?)` | FE→Rust | **Git Panel** | Stage/unstaged diffs. |
| `list_terminals()` | FE→Rust | **Terminal Panel** | List active terminal sessions. |
| `spawn_terminal(shell?)` | FE→Rust | **Terminal Panel** | Creates new terminal, returns PTY fd. |
| `write_stdin(terminal_id, data)` | FE→Rust | **Terminal Panel** | Send input to terminal. |
| `resize_pty(terminal_id, cols, rows)` | FE→Rust | **Terminal Panel** | Resize terminal PTY. |
| `kill_terminal(terminal_id)` | FE→Rust | **Terminal Panel** | Terminate terminal session. |
| `list_conversations()` | FE→Rust | **Chat Panel** | Returns `ConversationSummary[]`. |
| `get_conversation(id)` | FE→Rust | **Chat Panel** | Returns full message history. |
| `send_message(session_id, message)` | FE→Rust | **Chat Panel** | Starts streaming response (Tauri events). |
| `delete_conversation(id)` | FE→Rust | **Chat Panel** | Deletes session. |
| `load_config()` | FE→Rust | **Settings** | Returns full config object. |
| `save_config(updates)` | FE→Rust | **Settings** | Partial config update. |
| `get_output(source)` | FE→Rust | **Output Panel** | Returns recent output lines from selected source. |
| `get_errors()` | FE→Rust | **Errors Panel** | Returns error list with severity, timestamp, stack traces. |

**Events (Rust → Frontend):**
| Event | Payload | Serves |
|-------|---------|--------|
| `chat:stream_block` | `StreamBlock` | Chat — each completed block |
| `chat:stream_error` | `{ message }` | Chat — stream errors |
| `terminal:output` | `{ terminal_id, data }` | Terminal — stdout/stderr |
| `terminal:exit` | `{ terminal_id, code }` | Terminal — process exit |
| `fs:change` | `{ path, kind }` | File Explorer — file watcher updates |
| `output:append` | `{ source, line }` | Output panel |
| `error:new` | `ErrorEntry` | Errors panel |

---

## 6. State Ownership Matrix

| State | Owner | Storage | Why? |
|-------|-------|---------|------|
| File content | **Rust** | Filesystem | Single source of truth. Frontend only caches open tabs. |
| Conversations & messages | **Rust** | SQLite | Authoritative storage. Frontend queries via IPC. |
| Git state | **Rust** | On-demand (git2) | Always fresh from git. No caching possible. |
| Terminal PTY state | **Rust** | In-memory process handles | PTY is Rust-owned. Frontend has view-only terminal_id. |
| Config/settings | **Rust** | TOML file | Persisted config owned by Rust. |
| Panel layout & visibility | **Zustand** | Transient + persisted on change | Transient UI state. Ratios saved to config on change. |
| Active tab & open file list | **Zustand** | Transient | UI-only state. Reconstructed from workspace on load. |
| Search input & results | **Zustand** | Transient | Search results are temporary UI state. Re-run on demand. |
| Terminal list & active tab | **Zustand** | Transient | UI tabs don't need to survive restart. |
| Chat composer draft | **Zustand** | Transient | Unsent draft — local only. |
| Sidebar active view | **Zustand** | Transient | Last-view remembered per session only. |
| File tree expand state | **Zustand** | Transient | momoi-explorer manages its own tree state. |
| Monaco editor state | **Monaco** | In-memory | Editor's cursor, selections, undo history are Monaco-internal. |
| Ripgrep cached results | **None** | — | Results not cached. Re-run on every search. |

### Data Flow Rules

1. **Rust is authoritative.** Frontend never assumes it has the latest data. Mutations go through IPC → Rust validates + persists → frontend invalidates TanStack Query cache.
2. **TanStack Query for server-state.** File content, conversations, git status, config — anything fetched from Rust — uses `useQuery` with appropriate `staleTime` and `refetchOnMount`.
3. **Zustand for transient UI state.** Panel visibility, tab selection, search input text, composer draft — state that doesn't need Rust involvement.
4. **Config persistence bridge.** When layout sizes change (Zustand), a debounced effect calls `invoke('save_config', { layout: ... })` to persist. On app start, `load_config` populates Zustand initial state.

---

## 7. Resize & Persistence Strategy

- **Library:** shadcn `ResizablePanelGroup` (wraps `react-resizable-panels`)
- **Storage:** `state.config.panel_layout` in TOML
- **Trigger:** Panel resize events debounced (500ms) → `invoke('save_config', { layout })`
- **Restore:** On app start, `load_config` → layout state initialized from config → `ResizablePanelGroup` sizes set via `defaultSize`
- **Minimum window size:** 600px width
- **Default sizes:**
  - Left panel: 280px
  - Right panel: 380px
  - Bottom panel: 200px

---

## 8. Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Ctrl+B` | Toggle left side panel |
| `Ctrl+Shift+E` | Focus file explorer |
| `Ctrl+Shift+F` | Focus search panel |
| `Ctrl+Shift+G` | Focus git panel |
| `Ctrl+` ` ` | Toggle bottom panel |
| `Ctrl+Shift+` ` ` | Focus terminal (next terminal if multiple) |
| `Ctrl+W` | Close active editor tab |
| `Ctrl+Tab` | Switch editor tab |
| `Ctrl+=` | Zoom in (font size) |
| `Ctrl+-` | Zoom out (font size) |
| `Ctrl+0` | Reset zoom |
| `Ctrl+P` | Quick file open (fuzzy finder — post-MVP) |
| `Ctrl+Enter` | Send chat message |
| `Ctrl+\`` | Toggle right side panel (chat focus) |
| `Alt` | Activate menu bar (focus first menu) |
| `Alt+F` | Open File menu |
| `Alt+E` | Open Edit menu |
| `Alt+V` | Open View menu |
| `Alt+H` | Open Help menu |
| `Escape` | Close active panel / dismiss context menu / close menu |

---

## 9. Theme — Light Blue

Default theme is **light blue** — chosen for readability and reduced eye strain vs dark themes.

| Element | Color |
|---------|-------|
| Background | `#F0F4F8` (light blue-gray) |
| Sidebar/panel backgrounds | `#E8EDF2` |
| Active tab / selected item | `#D0DCE8` |
| Accent / highlights | `#4A90D9` (medium blue) |
| Text | `#1A2332` |
| Muted text | `#6B7B8D` |
| Border / dividers | `#D6DEE8` |

Editor theme within Monaco is a separate concern — Monaco gets its own theme (e.g., a light variant). The layout chrome uses the above palette.

**Font zoom** — status bar shows `− 100% +` buttons. Changes global UI font size (stored in config, not per-session). Range: 75%–150%, step 5%.
