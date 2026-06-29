<!-- from UI_Business_Test -->
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

## Prerequisites (RUNTIME DEPENDENCIES)

All three must be running simultaneously for tests to work:

### 1. Vite Dev Server (port 5173)
```powershell
Start-Process -WindowStyle Hidden -FilePath "cmd.exe" -ArgumentList @"
/c cd /d D:\work\MyAgent && npx vite --port 5173
"@

# Verify:
try { $r = Invoke-WebRequest -Uri "http://localhost:5173/" -UseBasicParsing -TimeoutSec 5; "Vite: $($r.StatusCode)" } catch { "Vite: DOWN" }
```

### 2. MSEdgeDriver (port 9515)
```powershell
Start-Process -WindowStyle Hidden -FilePath "cmd.exe" -ArgumentList @"
/c C:\Users\cheny\AppData\Roaming\npm\node_modules\@sitespeed.io\edgedriver\vendor\msedgedriver.exe --port=9515
"@

# Verify:
try { $r = Invoke-WebRequest -Uri "http://127.0.0.1:9515/status" -UseBasicParsing -TimeoutSec 3; "EdgeDriver: $($r.StatusCode)" } catch { "EdgeDriver: DOWN" }
```

### 3. tauri-driver (port 4444)
```powershell
Start-Process -WindowStyle Hidden -FilePath "cmd.exe" -ArgumentList @"
/c C:\Users\cheny\.cargo\bin\tauri-driver.exe --native-driver C:\Users\cheny\AppData\Roaming\npm\node_modules\@sitespeed.io\edgedriver\vendor\msedgedriver.exe
"@

# Verify:
try { $r = Invoke-WebRequest -Uri "http://127.0.0.1:4444/" -UseBasicParsing -TimeoutSec 3; "tauri-driver: $($r.StatusCode)" } catch { "tauri-driver: $($_.Exception.Message)" }
```

## How to Run Tests
```powershell
npx wdio run ./e2e/wdio.conf.ts --spec e2e/specs/task-NNN-*.test.ts

# Run all:
npx wdio run ./e2e/wdio.conf.ts --spec e2e/specs/task-001-resize-side-panel.test.ts e2e/specs/task-002-open-project.test.ts e2e/specs/task-003-file-explorer.test.ts e2e/specs/task-004-search-panel.test.ts e2e/specs/task-005-bottom-panel.test.ts
```

## Known Issues
- Tauri app must be rebuilt via `cargo build` in `src-tauri/` to pick up Rust changes before running tests
- WDIO spec glob pattern `./e2e/specs/**/*.ts` doesn't match on Windows — use explicit `--spec` flags
- Tests run against `http://localhost:5173/` (Vite dev server) — IPC bridge is available but some commands may behave differently than in bundled app

## Related
- [Lessons Learned](../../lessons-learned/lessons.md)
- [Test Skill](./skill.md)

## Workflow Rules
See `.opencode/rules/workflow.md` for the full development workflow. Summary:
- **Feature**: Gather requirement → Design → Implement → Test against requirement. Each gates the next.
- **Bug Fixing** (UNCONDITIONAL): **Reproduce → Find root cause → Design & implement fix → Verify fix by reproducing.** Reproduction is mandatory before any planning or fixing. No exceptions.
- **Prohibited** (UNCONDITIONAL): No modifying code before reproduction, no fixing unrelated issues, no closing bugs on unrelated fixes.

## API Key
- **Service**: opencode zen free aipkey
- **Key**: sk-xsjXJidLxkJxBtkhPpugyqnxNC1maFiIKuMnQGkRgKExOd3s7uWbwWJiebO0xAvs