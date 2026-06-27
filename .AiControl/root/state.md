# State

Architecture planning complete. 21/21 components decided across 7 ADRs.
Ready to begin phased implementation.

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

## Active Node

Current: `root/build_and_deploy` (Build & Deploy — in progress)
