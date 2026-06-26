# Prompt — Phase 0: Project Scaffold

Initialize the Tauri v2 + React 19 + shadcn/ui project.
Make all structural decisions before writing code.

## Steps
1. Decide directory structure (flat vs monorepo, frontend layout, Rust modules)
2. `npm create tauri-app` with React + TypeScript + Vite template
3. Install Tailwind v4, configure `@tailwindcss/vite`
4. `npx shadcn@latest init` — pick CSS variables, Tailwind, neutral gray
5. `npx shadcn@latest add` base components: button, card, input, dialog, scroll-area, separator, tooltip, resizable
6. Set up Rust modules: `commands/`, `llm/`, `storage/`, `fs/`, `search/`, `state/`, `agent/`
7. `cargo add tokio serde serde_json tracing`
8. Configure Tauri capabilities (shell, fs, dialog permissions)
9. Create `src/types/ipc.ts` — typed IPC contract mirroring Rust structs
10. Verify `npm run tauri dev` produces an empty window
11. Write `ArchitecturePlanning/project-structure.md` documenting final directory tree

## Key decisions to make
- pnpm vs npm? (pnpm recommended for workspace)
- src/ folder conventions (colocate by feature vs by type)
- Rust error type convention (thiserror? anyhow?)
- IPC payload naming conventions
