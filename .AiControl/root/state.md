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

## Skills

Available reusable workflows:
- [UI_Business_Test](../UI_Business_Test/skill.md) — AI-driven WDIO test generation and execution
- [Lessons Learner](../lessons-learner/skill.md) — Post-session lesson documentation and indexing

## Lessons Learned

Key engineering lessons from development sessions — quick reference for recurring issues:
- [Lessons Learned Index](../lessons-learned/lessons.md)

### Recent Lessons
| # | Lesson | Area |
|---|--------|------|
| [001](../lessons-learned/001-vite-server-survival.md) | Vite dev server dies when shell tool times out | E2E Testing |
| [002](../lessons-learned/002-tauri-command-naming.md) | Tauri `cmd_` prefix mismatch breaks IPC | Rust / IPC |
| [003](../lessons-learned/003-tests-bypass-ipc.md) | Tests that bypass IPC give false confidence | E2E Testing |
| [004](../lessons-learned/004-api-first-diagnosis.md) | Fixing bugs requires reproduction first — never skip to code reading | Debugging Process |
