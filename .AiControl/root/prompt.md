# Prompt

We are building an opencode-like agent desktop app.
Architecture is fully planned. Now executing phased implementation.

## Key Constraints

- **Rust owns all state** — frontend is cache only
- **shadcn/ui copy-paste model** — zero runtime deps from UI components
- **Block-based chat rendering** — no incremental markdown parsing
- **ripgrep only** — no built-in indexing. Advanced search = MCP (post-MVP)
- **No premature plugins** — plugin system is post-MVP

## Phase Nodes

| Directory | Phase | Description |
|-----------|-------|-------------|
| `project-scaffold` | 0 | Project structure, tooling, boilerplate |
| `ui-layout-design` | 0.5 | Wireframes, component tree, IPC contract |
| `rust-backbone` | 1 | Config, DB, LLM trait, IPC commands |
| `app-shell` | 2 | Monaco, file tree, tabs, terminal |
| `chat` | 3 | Block streaming, LLM wiring, history |
| `agent-tools` | 4 | File/git/grep/terminal agent tools |
| `search-panel` | 5 | ripgrep UI, results tree, replace |
| `settings-polish` | 6 | Config UI, theme, updates, error handling |
| `post-mvp` | 7 | Plugin system, MCP-like protocol |
| `UiAutoTestFramework` | T | tauri-driver + WDIO for automated UI verification |

Set `.current` to the active phase node before starting work.
