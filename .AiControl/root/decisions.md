# Decisions

Cross-cutting architecture decisions. See `ArchitecturePlanning/adr/` for full ADR text.

| # | Decision | ADR |
|---|----------|-----|
| 1 | **Tauri v2** — desktop shell, IPC, plugins | 0001 |
| 2 | **React 19** — frontend framework | 0002 |
| 3 | **shadcn/ui** — component library (copy-paste, zero deps) | — |
| 4 | **LLM Provider trait** — Rust trait wrapping provider SDKs | 0003 |
| 5 | **ConversationStore trait** — abstract storage, SQLite default | 0004 |
| 6 | **Block-based chat rendering** — wait-for-completion blocks | 0005 |
| 7 | **ripgrep only** — no built-in tree-sitter/embeddings/vector DB | 0006 |
| 8 | **momoi-explorer** — headless file explorer engine for file tree | 0007 |
| 9 | **Rust owns all state** — frontend is a cache/view | — |
| 10 | **UI Auto Test Framework** — tauri-driver + WDIO for headless WebView E2E testing | — |

## State Principle

Rust is the single source of truth for all persistent/authoritative state. The frontend caches views via TanStack Query. UI-only transient state (active tab, panel layout, input text) lives in Zustand. All mutations go through Tauri IPC.

## Test Principle

AI must be able to verify UI behavior without human testers. All fixes should be validated by automated tests before being deployed. Three-tier testing: Rust unit/integration → Frontend component/store → E2E WebView (WDIO + tauri-driver).
