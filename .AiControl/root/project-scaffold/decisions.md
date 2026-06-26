# Decisions — Phase 0

Decisions made during project scaffold.

| # | Decision | Rationale |
|---|----------|-----------|
| 1 | **pnpm** instead of npm | Workspace support, faster installs |
| 2 | **By type** folder convention | `components/`, `hooks/`, `lib/`, `stores/`, `types/` — simpler, scales well |
| 3 | **thiserror** for Rust errors | Typed, matchable error enum — serialize to JSON for frontend |
| 4 | Project name: **SquireCLI** | — |
| 5 | **Tauri v2** + **React 19** + **TypeScript 6** + **Vite 8** | Latest stable stack |
| 6 | **Tailwind v4** with `@tailwindcss/vite` plugin | CSS-first config (no tailwind.config.ts needed) |
| 7 | **shadcn/ui** with neutral gray base | UI component library, CSS variables theme |
| 8 | Rust modules by domain | `commands/`, `llm/`, `storage/`, `fs/`, `search/`, `state/`, `agent/` — each with `mod.rs` |
