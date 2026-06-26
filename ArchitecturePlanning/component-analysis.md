# Component Analysis: Critical Components & Build-vs-Borrow

> **Goal:** Identify every critical component, evaluate open-source options, assess dependency bloat, and determine configurability headroom.

---

## 1. Desktop Shell

| Criterion | Assessment |
|-----------|------------|
| Build vs Borrow | **Borrow** — Tauri v2 |
| OSS Option | [Tauri v2](https://github.com/tauri-apps/tauri) |
| Completeness | Full-featured: window mgmt, system tray, notifications, multi-window, menus |
| Dependency Bloat | Minimal — single Rust crate + WebView (OS-native, no bundled Chromium) |
| Configurability | Highly configurable via `tauri.conf.json` + Rust plugins; CSP/permissions per command |
| Risk | WebView inconsistencies across platforms (especially Linux); plugin ecosystem maturing |

**Decision (ADR-0001):** Tauri v2 — accepted.

---

## 2. Frontend UI Framework

| Criterion | Assessment |
|-----------|------------|
| Build vs Borrow | **Borrow** — React |
| Decision | **React 19** |
| Completeness | Production-ready, largest ecosystem, best tooling & library support |
| Dependency Bloat | ~12KB gzipped runtime; acceptable |
| Configurability | Full control over component design; no lock-in |
| Rationale | React's ecosystem (shadcn/ui, TanStack Query, Monaco, react-markdown) provides the most mature integration story for every other component. Svelte is lighter but would require more custom work. |

**Status:** ✅ Decided

---

## 3. UI Component Library

| Criterion | Assessment |
|-----------|------------|
| Build vs Borrow | **Borrow** |
| Decision | **shadcn/ui** |
| Completeness | 50+ unstyled, accessible primitives; copy-paste model avoids dependency lock |
| Dependency Bloat | Zero runtime deps — copy-paste model installs only what you use |
| Configurability | Fully customizable via Tailwind CSS; no lock-in |
| Rationale | Shadcn/ui's copy-paste model means we own the code — maximal configurability with zero inherited dependency chains |

**Status:** ✅ Decided

---

## 4. Chat / Conversation UI

| Criterion | Assessment |
|-----------|------------|
| Build vs Borrow | **Borrow (primitives), Build (custom rendering)** |
| OSS Options Found | **assistant-ui** (10.7k ★), **shadcn-chatbot-kit** (792 ★), **gravity-ui/aikit** (163 ★), **nlux/react** (5.8K/wk), **react-agent-ui**, **Cognipeer/chat-ui** |
| Analysis | assistant-ui is the most mature (10.7k ★, 120 contributors, 1663 releases). Built on Radix/shadcn primitives — composable, unstyled, accessible. Includes `<Thread>`, `<Composer>`, `<Message>`, streaming, tool calls, generative UI. However, it's coupled to Vercel AI SDK runtime. gravity-ui/aikit is SDK-agnostic but heavier. |
| Dependency Bloat | assistant-ui: ~150KB unpacked (primitives only, no styling). shadcn-chatbot-kit: zero runtime deps (copy-paste model). |
| Configurability | assistant-ui: fully composable primitives, swap any part. shadcn-chatbot-kit: copy-paste = own every line. |
| Core risk | The **message rendering** part (markdown, thinking blocks, tool calls) is the hard part — react-markdown alone won't handle streaming deltas, thinking sections, collapsible tool calls, etc. The **input/composer** part is trivial. |
| Decision | **shadcn-chatbot-kit** (scaffold) + custom rendering |
| Rationale | Copy-paste model = zero runtime deps, full code ownership. Hard parts (thinking blocks, tool calls, streaming) are simplified by waiting for each tag/block to complete before rendering — no need for incremental partial-markdown streaming. Renders on block boundaries, not character boundaries. |
| Dep Bloat | Zero — copied into our codebase |

### Block-based rendering strategy

Instead of streaming raw markdown character-by-character, the backend emits structured blocks. Each block renders only when complete:

| Block Type | Render Trigger | Component |
|------------|----------------|-----------|
| `text` | Complete text segment | `<TextBlock>` → react-markdown |
| `thinking` | Closing `</thinking>` tag | `<ThinkingBlock>` — collapsible, animated |
| `tool_call` | Complete tool invocation | `<ToolCallBlock>` — expand/collapse args + results |
| `code` | Complete code fence | `<CodeBlock>` — copy, diff, apply actions |
| `error` | Error signal | `<ErrorBlock>` |

This keeps the frontend simple — no incremental markdown parsing, no partial render edge cases.

**Status:** ✅ Decided

---

## 5. Markdown / Rich Content Rendering

| Criterion | Assessment |
|-----------|------------|
| Build vs Borrow | **Borrow** |
| Decision | **react-markdown** + remark/rehype plugins |
| Completeness | Full GFM, syntax highlighting (rehype-pretty-code), math (KaTeX), diagrams (mermaid) |
| Dependency Bloat | Install only needed plugins; tree-shakeable |
| Configurability | Fully composable plugin pipeline |

**Status:** ✅ Decided

---

## 6. Code Editor / Code Viewer

| Criterion | Assessment |
|-----------|------------|
| Build vs Borrow | **Borrow** |
| Decision | **Monaco Editor** |
| Completeness | 95% of VS Code editing features — themes, languages, keybindings, minimap, diff |
| Dependency Bloat | ~5MB (can tree-shake to ~2MB); acceptable for desktop app |
| Configurability | Full control via IEditorOptions, themes, language providers |
| Rationale | VS Code-compatible UX, best-in-class diff viewer built-in, largest language support out of the box |

**Status:** ✅ Decided

---

## 7. Diff Viewer

| Criterion | Assessment |
|-----------|------------|
| Build vs Borrow | **Borrow** (already covered by Monaco) |
| Decision | **Monaco editor built-in diff** |
| Completeness | Excellent — inline & side-by-side, syntax highlighted |
| Dependency Bloat | Zero additional — Monaco already included |
| Configurability | Full control via Monaco editor API |

**Status:** ✅ Decided (inherited from Monaco)

---

## 8. File Explorer (File Tree)

| Criterion | Assessment |
|-----------|------------|
| Build vs Borrow | **Borrow** |
| Decision | **momoi-explorer** (headless file explorer engine + React bindings) |
| What it gives us | Tree state management, virtualized rendering, file watching (debounce/coalesce/throttle — VS Code-compatible), create/rename/delete/move, drag & drop, multiselect, lazy loading. All file ops handled by core engine — no manual sync. |
| What we provide | A **file system adapter** (interface: `readDir`, `rename`, `delete`, `createFile`, `createDir`, `move`, `watch`) — we implement the Tauri IPC bridge here. The engine core is framework-agnostic; React bindings give us hooks + context. |
| Dep Bloat | momoi-explorer core is framework-agnostic + React bindings. We only import the UI primitives we need. The core handles all the complex stuff (event coalescing, debounce, chunked throttling). |
| Configurability | Headless = we style everything with shadcn/ui. The engine manages state; we render nodes with our own components. |

### What momoi-explorer handles for us

| Pain Point | How momoi handles it |
|------------|---------------------|
| **File watching sync** | `adapter.watch()` — debounce 75ms (VS Code-compatible), event coalescing (rename→delete+create merge, same-path dedup), chunk throttling (500 events/200ms) |
| **Directory tree state** | Internal tree data structure with expand/collapse, lazy loading, visibility tracking |
| **Drag & drop** | Built-in — just wire `adapter.move()` |
| **Rename / Create / Delete** | Tree engine handles state transitions; we provide the adapter implementation |
| **Virtualization** | Only renders visible nodes — handles 100k+ files |
| **Multiselect** | Built-in selection state |

We just write the **Tauri fs adapter** (~50 lines of Rust + ~30 lines of TypeScript) and style the tree nodes with shadcn.

**Alternatives considered at this level:** SVAR React File Manager (3.8K/wk, full UI but 428KB + 10 deps), react-arborist (tree view only, no fs), Arborix (headless tree, no file watching), vfs-kit (early dev). momoi-explorer is the only option that handles file watching + sync out of the box.

**Status:** ✅ Decided — borrow momoi-explorer engine

---

## 9. Terminal Emulator UI

| Criterion | Assessment |
|-----------|------------|
| Build vs Borrow | **Borrow** |
| Decision | **xterm.js** (frontend) + **Tauri shell plugin** (backend) |
| Completeness | Full VT100/xterm emulation, themes, fonts, keybindings, addon ecosystem |
| Dependency Bloat | xterm.js ~1MB (acceptable for desktop app). Tauri shell plugin is native — no bundled runtime. |
| Configurability | xterm.js: themes, font sizes, keybindings via addons. Shell plugin: permission-scoped commands. |
| Rationale | xterm.js is the de facto standard terminal emulator for web. Tauri shell plugin provides access-controlled process spawning. No heavy deps — xterm.js is self-contained. |

**Status:** ✅ Decided

---

## 10. LLM Provider Integration Layer

| Criterion | Assessment |
|-----------|------------|
| Build vs Borrow | **Borrow (Rust SDKs), Build (abstraction layer)** |
| Decision | **Abstract LLM trait in Rust** wrapping provider SDKs |
| Abstraction | `trait LlmProvider { async fn chat(&self, request: ChatRequest) -> Result<ChatResponse>; }` — generic over streaming, tool calls, model params |
| SDKs | **async-openai** (OpenAI), **anthropic-sdk-rust** (Anthropic), **llama-cpp-rs** (local) |
| Dependency Bloat | SDKs are focused; no transitive bloat |
| Configurability | Provider-agnostic trait prevents lock-in; registration via provider registry (string key → Box<dyn LlmProvider>) |
| Rationale | The trait boundary ensures provider SDKs never leak into app logic. New providers = new impl, no other code changes. |

**Status:** ✅ Decided

---

## 11. Conversation / History Storage

| Criterion | Assessment |
|-----------|------------|
| Build vs Borrow | **Borrow (SQLite), Build (abstraction layer)** |
| Decision | **Abstract `ConversationStore` trait** + SQLite implementation |
| Abstraction | `trait ConversationStore { fn create(&self, ...); fn append(&self, ...); fn get(&self, ...); fn list(&self, ...); }` — generic over storage backend |
| Implementation | **SQLite via `rusqlite`** as default backend |
| Dependency Bloat | SQLite ~600KB, bundled; trait itself is zero-cost |
| Configurability | The trait boundary prevents storage details from leaking. Future backends (Postgres, cloud API, file-based) are new impls only. |
| Rationale | The user explicitly requires swappable storage — trait from day one, no concrete type escapes the boundary. Bare-minimum interface: create session, append message, query history. |

**Status:** ✅ Decided

---

## 12. File System Operations (Read/Write/Edit)

| Criterion | Assessment |
|-----------|------------|
| Build vs Borrow | **Borrow from stdlib**, **Build (agent tool wrappers)** |
| OSS Options | Rust `std::fs`, `tokio::fs`, Tauri's `dialog` + `fs` plugins |
| Completeness | Full; all OS operations available |
| Dependency Bloat | Zero — stdlib |
| Configurability | N/A — standard I/O; we build tool wrappers for agent use |
| Recommendation | Use **Tauri's fs plugin** for access-controlled file operations; wrap as agent tools |

---

## 13. Terminal / Process Execution (Backend)

| Criterion | Assessment |
|-----------|------------|
| Build vs Borrow | **Borrow** |
| OSS Options | Tauri's **shell plugin**, Rust `std::process::Command`, **duct** crate |
| Completeness | All handle stdin/stdout/stderr, pty support via shell plugin |
| Dependency Bloat | Minimal — Tauri shell plugin is optional; `duct` is tiny |
| Configurability | Permissions per command in Tauri capability files |
| Recommendation | **Tauri shell plugin** for access-controlled command execution + **xterm.js** pty integration |

---

## 14. Code Search Engine

| Criterion | Assessment |
|-----------|------------|
| Build vs Borrow | **Borrow** |
| OSS Options | **ripgrep** (`rg`), **ripgrep-all** (`rga`), **ugrep**, **git-grep**, **tree-sitter** (structural) |
| Completeness | ripgrep is fastest, most feature-complete (PCRE2, multi-line, smart-case) |
| Dependency Bloat | Binary ~5MB, no runtime deps; can shell out or use `grep` Rust crate |
| Configurability | Flags for everything: glob, regex, context lines, file type |
| Recommendation | **Bundled ripgrep binary** + `grep` crate for programmatic use; also evaluate **tree-sitter** for AST-aware search |

---

## 15. Git Integration

| Criterion | Assessment |
|-----------|------------|
| Build vs Borrow | **Borrow** |
| OSS Options | **gix** (pure Rust git), **libgit2** (via `git2` crate), shell out to `git` CLI |
| Completeness | gix: growing but incomplete; git2: most complete (uses libgit2 C lib); CLI: 100% |
| Dependency Bloat | git2: links libgit2 (~2MB); CLI requires git installed |
| Configurability | git2: full git operations; CLI: every git feature |
| Recommendation | **git2** crate for programmatic git ops (commit, diff, branch, log). Shell out only for advanced workflows. |

---

## 16. Code Search (Search in Files Panel)

| Criterion | Assessment |
|-----------|------------|
| Build vs Borrow | **Borrow (ripgrep engine) + Build (search panel UI)** |
| Decision | **Custom search panel** — ripgrep backend, our React UI |
| What we build | A VS Code-style "Search in Files" panel with: search input + replace field, regex/whole-word/case toggles, glob include/exclude filters, results list grouped by file with context lines, double-click → open file at line in Monaco, replace single / replace all, match count per file |
| ripgrep backend | Tauri IPC command: `grep(query, options)` streams results back to frontend. Results grouped by file with line numbers and context. |
| Dep Bloat | ripgrep binary ~5MB (already included for other uses). UI is our code — zero additional deps. |
| Configurability | Full — we own every line of the UI |
| Rationale | No off-the-shelf React component exists for a multi-file search panel. Monaco's find/replace is single-file only. The UI is ~250 lines of React (input bar + results tree) — not worth importing a heavy dependency. |

Search is separated from indexing — this panel is interactive (user types a query). Indexing (ADR-0006) is about the agent searching the codebase programmatically — also ripgrep, just a different entry point.

**Status:** ✅ Decided — build custom panel over ripgrep

---

## 17. Configuration Management

| Criterion | Assessment |
|-----------|------------|
| Build vs Borrow | **Borrow** |
| OSS Options | **serde** + **toml**/json/yaml crates, **tauri-plugin-store** (persistent KV) |
| Completeness | Full: serde for (de)serialization, store plugin for persistence |
| Dependency Bloat | Minimal — serde is always needed |
| Configurability | Schema-defined config; support config file + env vars + CLI overrides |
| Recommendation | **serde** + TOML config files + **tauri-plugin-store** for runtime preferences |

---

## 18. Plugin / Extension System

| Criterion | Assessment |
|-----------|------------|
| Build vs Borrow | **Deferred — post-MVP** |
| Decision | **Not in MVP.** The app will be monolithic initially. A plugin/extension API will be extracted once we understand real extension patterns from usage. |
| Approach | Post-MVP, evaluate **Wasm** (Wasmtime/WasmEdge) for sandboxed plugins. This aligns with our intention to redefine how MCP works — MCP-like capabilities will emerge from the plugin system, not the other way around. |
| Rationale | Premature plugin APIs are worse than none. Building a plugin system before we know what users want to extend will force us into a corner. The first users get the full feature set built-in; extensibility comes from iteration. |

**Status:** ✅ Decided — post-MVP

---

## 19. Auto-Update System

| Criterion | Assessment |
|-----------|------------|
| Build vs Borrow | **Borrow** |
| OSS Options | **Tauri updater plugin** (built-in), **tauri-awesome-updater** |
| Completeness | Tauri updater: GitHub Releases, S3, custom endpoints; differential updates |
| Dependency Bloat | Zero — built into Tauri |
| Configurability | Server endpoint, update interval, manual/auto toggle |
| Recommendation | **Tauri built-in updater** — no reason not to use it |

---

## 20. Telemetry / Error Reporting

| Criterion | Assessment |
|-----------|------------|
| Build vs Borrow | **Borrow** |
| OSS Options | **sentry-rust**, **tracing** (structured logging), **actix**/tokio-console |
| Completeness | Sentry: full crash reporting, breadcrumbs, performance; tracing: structured logs |
| Dependency Bloat | Sentry is modular (opt-in features); tracing is zero-cost |
| Configurability | Sentry: sampling, custom context; tracing: filter levels, targets |
| Recommendation | **tracing** for structured logging (always); **sentry-rust** for crash reporting (opt-in). Must be privacy-respecting. |

---

## 21. State Management (Frontend)

| Criterion | Assessment |
|-----------|------------|
| Build vs Borrow | **Borrow** |
| Decision | **Zustand** (client cache) + **TanStack Query** (IPC data layer) |
| Ownership | **Rust owns all authoritative state.** Frontend is a cache/view. All mutations go through Tauri IPC → Rust validates + persists → frontend invalidates cache. |
| Pattern | TanStack Query calls Tauri IPC commands (e.g., `useQuery({ queryKey: ['conversations'], queryFn: () => invoke('list_conversations') })`). Zustand holds transient UI state only (active tab, panel layout, search input text). |
| Dep Bloat | ~13KB combined; tree-shakeable |
| Rationale | State sync bugs are exponentially harder to fix later. Rust as single source of truth means: no stale read-after-write, no conflict resolution, no recovery logic. The IPC latency cost is negligible for a desktop app. |

**Status:** ✅ Decided

---

## Summary: Build vs Borrow

| Component | Decision | Key Dependency |
|-----------|----------|----------------|
| Desktop Shell | **Borrow** | Tauri v2 |
| Frontend Framework | **Borrow** | React 19 |
| UI Components | **Borrow** | shadcn/ui |
| Chat UI | **Borrow scaffold + Build blocks** | shadcn-chatbot-kit |
| Markdown Rendering | **Borrow** | react-markdown |
| Code Editor | **Borrow** | Monaco Editor |
| Diff Viewer | **Borrow** | Monaco diff (built-in) |
| Terminal UI | **Borrow** | xterm.js |
| LLM Integration | **Borrow SDKs + Build trait** | async-openai, anthropic-sdk-rust |
| History Storage | **Borrow SQLite + Build trait** | rusqlite |
| File Operations | **Borrow** | Tauri fs plugin |
| Process Execution | **Borrow** | Tauri shell plugin |
| File Explorer | **Borrow (momoi-explorer)** | momoi-explorer core + React |
| Code Search | **Borrow** | ripgrep |
| Git Ops | **Borrow** | git2 crate |
| Code Search (Search Panel) | **Build UI + ripgrep backend** | ripgrep |
| Config Management | **Borrow** | serde + TOML |
| Plugin / Extension System | **Defer — post-MVP** | Wasm (future) |
| Auto-Update | **Borrow** | Tauri updater |
| Telemetry | **Borrow** | tracing + sentry (opt-in) |
| State Management | **Borrow** | Zustand + TanStack Query |

**Decided so far:** 21/21 components locked in. ✅
