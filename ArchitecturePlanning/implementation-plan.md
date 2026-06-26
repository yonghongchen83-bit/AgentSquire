# Implementation Plan

> Build order, phase by phase. Each phase depends on the previous.

---

## Phase 0: Project Scaffold

**Goal:** Project structure decided, boilerplate compiles and runs.

| Step | What | Details |
|------|------|---------|
| 0.1 | **Directory structure decision** | Decide flat vs monorepo: `packages/` for shared types? Single `src/` for frontend? Document the final tree. |
| 0.2 | `npm create tauri-app` | Tauri v2 + React + TypeScript + Vite |
| 0.3 | **Frontend folder layout** | `src/` → `components/`, `hooks/`, `lib/`, `stores/`, `types/`, `pages/` (or panels/). Decide component co-location rules. |
| 0.4 | **Rust module layout** | `src-tauri/src/` → `commands/` (IPC handlers), `llm/` (provider trait + impls), `storage/` (conversation store), `fs/` (file ops, watcher, git), `search/` (grep), `state/` (config), `agent/` (tools). Each module gets `mod.rs` + feature files. |
| 0.5 | Install Tailwind v4 | `npm install tailwindcss @tailwindcss/vite` |
| 0.6 | Init shadcn/ui | `npx shadcn@latest init` — configure components.json |
| 0.7 | Add base shadcn components | `npx shadcn@latest add button card input dialog scroll-area separator tooltip resizable` |
| 0.8 | Install Rust dev tools | `cargo add tokio serde serde_json tracing` |
| 0.9 | Configure Tauri capabilities | Set up allow-lists for shell, fs, dialog permissions |
| 0.10 | **Shared types** | Create `src/types/ipc.ts` — TypeScript interfaces for every IPC payload (matches Rust structs). This is the contract between frontend and backend. First version typed, validated by serde. |
| 0.11 | Verify build | `npm run tauri dev` — empty window appears |
| 0.12 | **Deliverable** | `ArchitecturePlanning/project-structure.md` — full directory tree with annotations for every folder and file |

**Depends on:** Nothing

---

## Phase 0.5: UI Layout Design

**Goal:** Decide the panel layout before writing any shell code. This is the blueprint everything else builds to.

| Step | What | Details |
|------|------|---------|
| 0.5.1 | Panel layout decision | Choose between: (A) VS Code single-panel — sidebar + editor center + optional bottom terminal, or (B) split layout — chat on right, editor on left. Document the chosen layout with a diagram. |
| 0.5.2 | Wireframe the 4 zones | **Sidebar:** file tree (top) → search panel (tab-switchable). **Editor area:** tabs bar + Monaco editor. **Chat panel:** message list + composer input. **Bottom:** terminal panel (collapsible, resizable). |
| 0.5.3 | Component tree | Define React component hierarchy: `<App>` → `<Sidebar>` + `<MainArea>` + `<ChatPanel>` + `<BottomPanel>`. Every component's props and state shape. |
| 0.5.4 | IPC contract review | Walk each IPC command and verify it matches what every panel needs. E.g., `list_directory(path)` returns `FileEntry[]` — is that enough for the file tree? `grep(query, opts)` returns `SearchMatch[]` — enough for search results? |
| 0.5.5 | Route / state design | Single-page or tabs? Active conversation + active file + active search query — where do they live? (Zustand for transient, Rust for persistent) |
| 0.5.6 | Resize & persistence | `ResizablePanelGroup` ratios — save panel sizes in config so layout survives restart. |
| 0.5.7 | Deliverable | A markdown document in `ArchitecturePlanning/layout-design.md` with: wireframe ASCII diagram, component tree, IPC contract checklist, state ownership matrix. |

**Depends on:** Phase 0 (scaffold running), but runs **parallel to Phase 1** (Rust backbone)

**Runs parallel to Phase 1** — layout design doesn't block backend work.

---

## Phase 1: Rust Backbone

**Goal:** All Rust infrastructure in place. No UI yet, but can test via IPC.

| Step | What | Details |
|------|------|---------|
| 1.1 | Config module | `state/config.rs` — load/save TOML from `~/.app/config.toml` via serde. Config struct for LLM keys, theme, settings. |
| 1.2 | Logging setup | `tracing` subscriber: file log + stdout. Log level from config. |
| 1.3 | SQLite init | `state/db.rs` — `rusqlite` connection, run migrations. Store in app data dir. |
| 1.4 | ConversationStore trait | `storage/conversation_store.rs` — trait with `create_session`, `append_message`, `get_session`, `list_sessions`, `delete_session` |
| 1.5 | SQLite ConversationStore | `storage/sqlite_store.rs` — impl the trait, schema: `sessions` + `messages` tables |
| 1.6 | LlmProvider trait | `llm/provider.rs` — trait: `chat(&self, ChatRequest) -> Result<ChatResponse>` — streaming via mpsc |
| 1.7 | OpenAI impl | `llm/openai.rs` — wraps `async-openai`, maps to LlmProvider trait |
| 1.8 | Anthropic impl | `llm/anthropic.rs` — wraps `anthropic-sdk-rust` |
| 1.9 | Provider registry | `llm/registry.rs` — `HashMap<String, Box<dyn LlmProvider>>` built from config |
| 1.10 | Tauri IPC commands | `commands/mod.rs` — wire up: `list_conversations`, `get_conversation`, `send_message`, `list_directory`, `read_file`, `write_file` |
| 1.11 | File ops module | `fs/ops.rs` — `read_file(path)`, `write_file(path, content)`, `create_dir`, `delete_item`, `rename_item` — all via Tauri fs plugin |
| 1.12 | Grep command | `search/grep.rs` — IPC command: `grep(query, path, options)` → stream results. Uses `grep` crate (ripgrep internals). |
| 1.13 | Git ops module | `fs/git.rs` — wrap git2: status, diff, commit, log, branch list. IPC commands. |
| 1.14 | Terminal/process module | `shell/exec.rs` — spawn process, stream stdout/stderr via Tauri events. Uses `tauri-plugin-shell`. |
| 1.15 | File watcher adapter | `fs/watcher.rs` — `notify` crate watcher, emit events over Tauri event bus for momoi-explorer sync |

**Depends on:** Phase 0

---

## Phase 2: App Shell

**Goal:** The window chrome — file tree, tabs, editor — wired to Rust IPC but no chat yet.

| Step | What | Details |
|------|------|---------|
| 2.1 | Layout shell | React component: sidebar (file tree) + main area (editor + tabs) + optional bottom panel (terminal). Uses shadcn `ResizablePanelGroup`. |
| 2.2 | Monaco editor | `@monaco-editor/react` — single file viewer. IPC `read_file` on file open. Language detection from extension. |
| 2.3 | Tab management | Custom `useTabs` hook + tab bar component. Open files from tree → new tab. Close, reorder, highlight dirty. |
| 2.4 | File tree (momoi) | Install `momoi-explorer` + `momoi-explorer/react`. Write Tauri `FileSystemAdapter` (implements `readDir`, `rename`, `delete`, `createFile`, `createDir`, `move`, `watch`). Wire click → open in Monaco. |
| 2.5 | File tree styling | shadcn tree look — folder/file icons, indent lines, selection highlight |
| 2.6 | Git status on tree | IPC `git_status` per file → color dots (green=modified, red=deleted, yellow=staged) |
| 2.7 | Terminal panel | `@xterm/xterm` in bottom panel. IPC to Tauri shell plugin for process spawn. Fit addon for resize. |
| 2.8 | Context menus | Right-click on file tree: New File, New Folder, Rename, Delete, Reveal in Explorer. Wire to IPC. |

**Depends on:** Phase 1 (IPC commands exist)

---

## Phase 3: Chat

**Goal:** Send messages to LLM, stream response, render blocks.

| Step | What | Details |
|------|------|---------|
| 3.1 | Chat IPC wiring | `send_message` IPC: takes `session_id + message`, routes through LlmProvider, streams blocks back via Tauri events |
| 3.2 | shadcn-chatbot-kit copy | Copy shadcn-chatbot-kit's `<Chat>` + `<MessageList>` + `<Composer>` into our codebase. Adapt to call our IPC instead of AI SDK. |
| 3.3 | Block-based stream render | Frontend receives stream events: `{ type: "text", content }`, `{ type: "thinking", content }`, `{ type: "tool_call", ... }`, `{ type: "code", ... }`. Each block renders when complete. |
| 3.4 | Thinking block | `<ThinkingBlock>` — collapsible, animated dots while streaming, shows content when done |
| 3.5 | Tool call block | `<ToolCallBlock>` — expandable card showing tool name, args, result |
| 3.6 | Code block | `<CodeBlock>` — Monaco read-only view (mini) + action buttons: copy, apply, diff |
| 3.7 | Conversation sidebar | List of sessions from IPC `list_conversations`. Click → load. Create new. Delete. |
| 3.8 | Message persistence | Every message appended via IPC → ConversationStore SQLite impl. On reload, `get_session` loads history. |

**Depends on:** Phase 1 (LLM + storage), Phase 2 (layout for chat panel)

---

## Phase 4: Agent Tools

**Goal:** The agent can actually do things — read files, search code, run commands.

| Step | What | Details |
|------|------|---------|
| 4.1 | Tool trait | `agent/tool.rs` — `trait Tool { name, description, execute(args) }` |
| 4.2 | File read tool | `ReadFileTool` — reads file content, returns to LLM |
| 4.3 | File write/edit tool | `WriteFileTool` / `EditFileTool` — write/edit files via IPC |
| 4.4 | Code search tool | `GrepTool` — runs grep via IPC, returns results |
| 4.5 | Terminal tool | `TerminalTool` — runs a command, returns stdout+stderr |
| 4.6 | Git tool | `GitTool` — git status, diff, commit |
| 4.7 | Tool registry | Register all tools, inject into `ChatRequest` for LLM tool calling |
| 4.8 | Tool result rendering | Wire tool call results back into chat stream as `<ToolCallBlock>` |
| 4.9 | Approve/reject flow | Optional: human-in-the-loop for write/delete/terminal tools |

**Depends on:** Phase 3 (chat system to invoke tools)

---

## Phase 5: Search Panel

**Goal:** VS Code-style Ctrl+Shift+F search across files.

| Step | What | Details |
|------|------|---------|
| 5.1 | Search panel layout | Sidebar panel with search input + replace input + options toggles (regex, case, whole word, glob include/exclude) |
| 5.2 | IPC search command | `grep_search(query, options)` → calls ripgrep crate, returns `Vec<{file, line, column, content, context_lines}>` |
| 5.3 | Results tree | Group by file, show context lines, match highlighting. File header with match count. |
| 5.4 | Click to open | Double-click result → open file in Monaco at line number |
| 5.5 | Replace | Replace field + "Replace All" button. IPC `grep_replace(query, replacement, options)` → returns diff, applies changes. |
| 5.6 | Collapse/expand results | per-file collapse toggle |

**Depends on:** Phase 2 (editor to open files), Phase 1 (grep IPC)

---

## Phase 6: Settings & Polish

**Goal:** Configurable, shippable.

| Step | What | Details |
|------|------|---------|
| 6.1 | Settings UI | shadcn dialog/sheet with tabs: General (theme, font size), LLM Providers (API keys, model selection, endpoints), Search (exclude patterns), Terminal (shell path, font) |
| 6.2 | Theme switching | Light/dark via Tailwind `class` strategy. Persist in config. |
| 6.3 | Font/editor settings | Monaco font size, tab size, word wrap — read from config IPC, apply to editor |
| 6.4 | Auto-update | Wire `tauri-plugin-updater` — check on startup, prompt to install |
| 6.5 | Error handling | Global error boundary (React), IPC error display, crash reporter (Sentry opt-in) |
| 6.6 | Keyboard shortcuts | Monaco keybindings, global shortcuts (Ctrl+P file search, Ctrl+Shift+F search panel, Ctrl+` terminal) |
| 6.7 | Loading/splash | Initial loading state while Rust initializes SQLite + config |

**Depends on:** Phase 2 (shell to have settings access)

---

## Phase 7: Post-MVP

**Goal:** Extensibility. Not in initial build.

| Step | What | Details |
|------|------|---------|
| 7.1 | Plugin system design | Evaluate Wasmtime for sandboxed plugins. Define plugin API surface. |
| 7.2 | MCP-like protocol | If we change how MCP works, this is where it happens — as a plugin, not baked in. |

**Depends on:** Everything else

---

## Dependency Graph Between Phases

```
Phase 0 (scaffold)
   ├── Phase 0.5 (Layout design) ─── parallel ─── Phase 1 (Rust backbone)
   └── Phase 2 (App shell) ←─── depends on both
          ├── Phase 3 (Chat) ←─── depends on Phase 2
          │    └── Phase 4 (Agent tools) ←─── depends on Phase 3
          └── Phase 5 (Search panel) ←─── depends on Phase 2
   └── Phase 6 (Settings & polish) ←─── depends on Phase 2
          └── Phase 7 (Post-MVP)
```

Phases 0.5 and 1 run in parallel. Phases 3 and 5 run in parallel after Phase 2. Phase 4 depends on Phase 3.

---

## Rough Timeline (estimate per phase)

| Phase | What | Est. days | Parallel with |
|-------|------|-----------|---------------|
| 0 | Scaffold | 1 | — |
| 0.5 | Layout design | 2 | Phase 1 |
| 1 | Rust backbone | 5-7 | Phase 0.5 |
| 2 | App shell | 5-7 | — |
| 3 | Chat | 4-5 | Phase 5 |
| 4 | Agent tools | 3-4 | — |
| 5 | Search panel | 2-3 | Phase 3 |
| 6 | Settings & polish | 3-4 | — |
| **Total MVP** | **Phases 0-6** | **~22-30 days** (10-12 calendar with parallelism) |
