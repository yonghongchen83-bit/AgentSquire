# State

Architecture planning complete. 21/21 components decided across 7 ADRs.
UI Auto Test Framework installed for AI-driven verification.

## Phase Progress

| Phase | Status |
|-------|--------|
| 0 — Project Scaffold | ✅ Complete |
| 0.5 — UI Layout Design | ✅ Complete |
| 1 — Rust Backbone | ✅ Complete |
| **2 — App Shell** | **✅ Complete** |
| **3 — Chat** | **✅ Complete** |
| **4 — Agent Tools** | **✅ Complete** |
| **5 — Search Panel** | **✅ Complete** |
| **6 — Settings & Polish** | **✅ Complete** |
| 7 — Post-MVP | 📅 Planned |
| **B — Build & Deploy** | **🏗️ In Progress** |
| **T — UI Auto Test Framework** | **✅ Complete** |

## Architecture Docs

All design documents live in `ArchitecturePlanning/`:
- `component-analysis.md` — 21 components analyzed, build-vs-borrow decisions
- `dependency-report.md` — full inventory: Rust crates, npm packages, binaries, OS deps
- `implementation-plan.md` — 7.5 phases with steps, timeline, dependency graph
- `layout-design.md` — panel layout, component tree, IPC contract, state ownership
- `adr/0001-use-tauri-as-desktop-framework.md`
- `adr/0002-use-react-as-frontend-framework.md`
- `adr/0003-llm-provider-abstraction-trait.md`
- `adr/0004-conversation-store-abstraction-trait.md`
- `adr/0005-chat-ui-with-block-based-rendering.md`
- `adr/0006-ripgrep-only-code-search.md`
- `adr/0007-use-momoi-explorer-for-file-tree.md`

## Test Infrastructure

| Suite | Framework | Count | Command |
|-------|-----------|-------|---------|
| Frontend unit | Vitest + RTL | 71 tests | `npm test` |
| Rust unit | `#[cfg(test)]` | 49 tests | `npm run test:rust` |
| Rust integration | `src-tauri/tests/` | 4 tests | `npm run test:rust` |
| **E2E (WebView)** | **WDIO + tauri-driver** | **6 smoke tests** | `npm run test:e2e` |

## Active Node

Current: `root/UiAutoTestFramework` (UI Auto Test Framework — recently set up)

## 🔨 Build System Change (2026-07-04)

**Problem:** Every `cargo build` / `tauri dev` invocation sometimes triggered a full recompilation of all external dependencies, taking ~30 minutes. The root cause was inconsistent feature sets/cache splits between different invocation paths (`cargo build` vs `cargo run` vs `tauri dev` wrappers), plus occasional corrupted incremental caches.

**Solution:** All Rust build/run/test/clean actions now go through fixed PowerShell scripts in `scripts/`:
- `scripts/build.ps1` — build debug (no CLI args)
- `scripts/run.ps1` — launch full app (no CLI args)
- `scripts/test.ps1` — unit tests (no CLI args)
- `scripts/test-all.ps1` — all tests (no CLI args)
- `scripts/frontend-test.ps1` — Vitest (no CLI args)
- `scripts/clean.ps1` — **requires user "yes" confirmation** (~30 min rebuild)

VS Code tasks, launch config (F5), and npm scripts all route through these scripts. Manual `cargo build` / `cargo test` / `npm run tauri dev` in the terminal is **banned** — the AI must use the scripts only. Clean/rebuild requires asking the user for explicit permission before invocation.

See root `env.md` for the full spec and reasoning.

## Skills

Key engineering lessons from development sessions — quick reference for recurring issues:
- [Lessons Learned Index](../lessons-learned/lessons.md)

### Recent Lessons
| # | Lesson | Area |
|---|--------|------|
| [001](../lessons-learned/001-vite-server-survival.md) | Vite dev server dies when shell tool times out | E2E Testing |
| [002](../lessons-learned/002-tauri-command-naming.md) | Tauri `cmd_` prefix mismatch breaks IPC | Rust / IPC |
| [003](../lessons-learned/003-tests-bypass-ipc.md) | Tests that bypass IPC give false confidence | E2E Testing |
| [004](../lessons-learned/004-api-first-diagnosis.md) | Fixing bugs requires reproduction first — never skip to code reading | Debugging Process |
