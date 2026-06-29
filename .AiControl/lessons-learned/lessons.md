# Lessons Learned

Quick-reference index of engineering lessons discovered during development.

| # | Lesson | Area | Root Cause |
|---|--------|------|------------|
| [001](./001-vite-server-survival.md) | Vite dev server dies during WDIO tests | E2E Testing | Shell tool timeout kills child processes |
| [002](./002-tauri-command-naming.md) | Tauri IPC commands silently fail | IPC / Rust | `cmd_` prefix mismatch between Rust fn name and frontend `invoke()` |
| [003](./003-tests-bypass-ipc.md) | Tests pass but verify nothing about real backend | E2E Testing | `browser.execute` manipulates stores directly instead of UI interactions |
| [004](./004-api-first-diagnosis.md) | Fixing bugs requires reproduction first — never skip to code reading | Debugging Process | Modified code before reproducing the bug; made unrelated "fixes" that didn't address the symptom |

## Related Nodes
- [UI_Business_Test](../UI_Business_Test/env.md) — E2E test infrastructure
