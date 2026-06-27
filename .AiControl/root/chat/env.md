# Env — Phase 3

See `root/env.md` for full toolchain setup.

## Phase-specific
- No additional npm packages needed (custom components, zero new deps)
- Vitest for frontend tests: `pnpm test` / `pnpm test:watch`
- Rust tests: `cargo test` (in src-tauri/)
- LLM API keys go in config.toml (not committed to git)

## Key Files
| Path | Purpose |
|------|---------|
| `src/components/chat-*.tsx` | Chat UI components |
| `src/stores/chat-store.ts` | Chat state (Zustand) |
| `src/lib/ipc.ts` | Chat IPC + event listeners |
| `src/types/ipc.ts` | Chat type definitions |
