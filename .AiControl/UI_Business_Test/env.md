# UI_Business_Test Environment

## Tech Stack
- WebdriverIO v9 (e2e test runner)
- Mocha (test framework)
- react-resizable-panels v4 (layout panels)
- Tauri v2 (app framework)
- MSEdgeDriver (WebDriver for Tauri/WebView2 on Windows)

## Directory Structure
- `e2e/tasks/` — Human-readable test task specs (markdown)
- `e2e/specs/` — Generated WebdriverIO tests
- `e2e/wdio.conf.ts` — WebdriverIO configuration
- `e2e/helpers/` — Test helper utilities

## How to Run Tests
1. Start `tauri dev` (runs Vite + Tauri app)
2. Start `tauri-driver --native-driver <path-to-msedgedriver>`
3. Run: `npx wdio run ./e2e/wdio.conf.ts --spec e2e/specs/<test-file>`
