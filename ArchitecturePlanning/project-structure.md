# Project Structure

> Directory tree with annotations. Decided during Phase 0.

---

## Root

```
D:\work\MyAgent\
├── src/                        # Frontend — React + TypeScript + Vite
├── src-tauri/                  # Backend — Rust + Tauri v2
├── public/                     # Static assets (favicon, etc.)
├── ArchitecturePlanning/       # Design docs, ADRs, dependency report, plan
├── .AiControl/                 # Phase nodes (project-scaffold, rust-backbone, etc.)
├── .vscode/                    # Editor settings
├── .gitignore
├── package.json                # Frontend dependencies & scripts
├── vite.config.ts              # Vite bundler config
├── tsconfig.json               # TypeScript config
├── tailwind.config.ts          # Tailwind CSS config
├── postcss.config.js           # PostCSS (Tailwind pipeline)
└── components.json             # shadcn/ui config
```

---

## Frontend: `src/`

Organized **by type** (components, hooks, stores, lib, types).

```
src/
├── main.tsx                    # React entry point
├── App.tsx                     # Root component — layout shell
├── App.css                     # Global styles (Tailwind directives)
├── vite-env.d.ts               # Vite type declarations
│
├── components/                 # Reusable UI components
│   ├── ui/                     # shadcn/ui components (copy-pasted)
│   │   ├── button.tsx
│   │   ├── card.tsx
│   │   ├── input.tsx
│   │   ├── dialog.tsx
│   │   ├── scroll-area.tsx
│   │   ├── separator.tsx
│   │   ├── tooltip.tsx
│   │   └── resizable.tsx
│   ├── chat/                   # Chat panel components
│   │   ├── ChatPanel.tsx       #   message list + composer
│   │   ├── MessageList.tsx     #   scrollable message container
│   │   ├── MessageBlock.tsx    #   renders a single block
│   │   ├── ThinkingBlock.tsx   #   collapsible reasoning
│   │   ├── ToolCallBlock.tsx   #   expandable tool card
│   │   ├── CodeBlock.tsx       #   Monaco read-only + actions
│   │   └── ConversationList.tsx # sidebar: sessions
│   ├── editor/                 # Monaco editor components
│   │   ├── EditorPanel.tsx     #   tabs + editor area
│   │   ├── EditorTabs.tsx      #   tab bar
│   │   └── MonacoWrapper.tsx   #   @monaco-editor/react wrapper
│   ├── sidebar/                # Sidebar components
│   │   ├── Sidebar.tsx         #   container with tabs
│   │   ├── FileTree.tsx        #   momoi-explorer tree
│   │   ├── FileTreeNode.tsx    #   single tree node renderer
│   │   └── SearchPanel.tsx     #   grep search UI
│   ├── terminal/               # Terminal components
│   │   ├── TerminalPanel.tsx   #   xterm.js wrapper
│   │   └── TerminalTab.tsx     #   terminal tab
│   └── settings/               # Settings components
│       ├── SettingsDialog.tsx   #   modal/sheet
│       ├── GeneralTab.tsx
│       ├── LlmProvidersTab.tsx
│       ├── SearchTab.tsx
│       └── TerminalTab.tsx
│
├── hooks/                      # Custom React hooks
│   ├── useTabs.ts              #   tab state management
│   ├── useFileTree.ts          #   momoi-explorer bridge
│   ├── useChat.ts              #   chat IPC + stream handling
│   └── useTerminal.ts          #   terminal IPC bridge
│
├── stores/                     # Zustand stores (UI-only transient state)
│   ├── uiStore.ts              #   active tab, panel layout, sidebar width
│   └── editorStore.ts          #   open files, active file, dirty state
│
├── lib/                        # Utility functions
│   ├── utils.ts                #   cn(), etc. (shadcn helpers)
│   └── ipc.ts                  #   typed Tauri invoke() wrappers
│
└── types/                      # TypeScript types
    ├── ipc.ts                  #   IPC payload types (matches Rust structs)
    ├── chat.ts                 #   ChatRequest, ChatResponse, Block types
    ├── editor.ts               #   Tab, OpenFile types
    └── search.ts               #   SearchQuery, SearchResult types
```

---

## Backend: `src-tauri/src/`

Organized **by domain** (commands, llm, storage, fs, search, agent, state).

```
src-tauri/
├── Cargo.toml                  # Rust dependencies
├── tauri.conf.json             # Tauri configuration (window, permissions, updater)
├── capabilities/               # Tauri v2 capability files
│   └── default.json
├── icons/                      # App icons
└── src/
    ├── main.rs                 # Entry point (no logic — just launches)
    ├── lib.rs                  # Tauri builder, plugin registration, command wiring
    │
    ├── commands/               # IPC handlers — thin layer, delegates to modules
    │   ├── mod.rs
    │   ├── chat.rs             # send_message, list_conversations, get_conversation
    │   ├── files.rs            # read_file, write_file, create_item, delete_item
    │   ├── search.rs           # grep_search, grep_replace
    │   └── git.rs              # git_status, git_diff, git_commit
    │
    ├── llm/                    # LLM provider abstraction
    │   ├── mod.rs
    │   ├── provider.rs         # LlmProvider trait
    │   ├── openai.rs           # OpenAI impl (wraps async-openai)
    │   ├── anthropic.rs        # Anthropic impl (wraps anthropic-sdk-rust)
    │   └── registry.rs         # Provider registry: key → Box<dyn LlmProvider>
    │
    ├── storage/                # Conversation persistence
    │   ├── mod.rs
    │   ├── conversation_store.rs  # ConversationStore trait
    │   └── sqlite_store.rs     # SQLite impl
    │
    ├── fs/                     # File system operations
    │   ├── mod.rs
    │   ├── ops.rs              # Read, write, create, delete, rename
    │   ├── watcher.rs          # notify-based file watcher → Tauri events
    │   └── git.rs              # git2 wrapper: status, diff, log, commit
    │
    ├── search/                 # Code search (ripgrep)
    │   ├── mod.rs
    │   └── grep.rs             # grep() using the grep/ignore crates
    │
    ├── agent/                  # Agent tool implementations
    │   ├── mod.rs
    │   ├── tool.rs             # Tool trait
    │   ├── read_file.rs
    │   ├── write_file.rs
    │   ├── grep_tool.rs
    │   ├── terminal.rs
    │   └── git_tool.rs
    │
    └── state/                  # Application config & state
        ├── mod.rs
        └── config.rs           # serde + TOML config (LLM keys, theme, settings)
```

---

## Boundaries & Contracts

| Boundary | Interface | Enforcement |
|----------|-----------|-------------|
| Frontend ↔ Backend | Tauri IPC | `src/types/ipc.ts` mirrors Rust command structs |
| LLM providers | `LlmProvider` trait | Rust compiler — add provider = impl trait |
| Conversation storage | `ConversationStore` trait | Rust compiler — swap backend = impl trait |
| Agent tools | `Tool` trait | Rust compiler — add tool = impl trait |
| File system | momoi-explorer `FileSystemAdapter` | TypeScript interface |
| UI ↔ Store | Zustand selectors | TypeScript — no magic strings |

## Naming Conventions

| Category | Convention | Example |
|----------|-----------|---------|
| React components | PascalCase | `ChatPanel.tsx` |
| Hooks | camelCase, `use` prefix | `useTabs.ts` |
| Stores | camelCase, `Store` suffix | `uiStore.ts` |
| Types | PascalCase | `ChatRequest` |
| Rust files | snake_case | `conversation_store.rs` |
| Rust types | PascalCase | `SqliteStore` |
| IPC commands | snake_case | `send_message` |

## `.gitignore` recommendations

```
node_modules/
dist/
src-tauri/target/
.DS_Store
*.log
.env
.env.local
src-tauri/capabilities/  (if generated, keep if hand-written)
```
