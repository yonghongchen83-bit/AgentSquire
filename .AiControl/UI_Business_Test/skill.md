# AI-Driven UI Test Skill

## Concept

Each UI test case is defined as a **markdown task spec** in `e2e/tasks/`. The AI reads the spec, generates a WebdriverIO test file, and runs it against the running Tauri app.

## Workflow

### Unconditional Rule: Reproduce Before Planning
**This rule applies to ALL bug-fixing scenarios without exception:**
1. **Reproduce** — Observe the bug in the running app. Gather concrete evidence (error logs, screenshots, unexpected behavior).
2. **Root cause** — Only after reproduction, analyze code to find the cause.
3. **Plan & fix** — Design the fix and implement it.
4. **Verify** — Re-reproduce to confirm the fix works.

No planning, no designing, no coding is permitted until the bug has been successfully reproduced.

### Test Creation Workflow

1. **Define the task** — Create `e2e/tasks/task-NNN.md` with: description, steps, expected results, and selectors
2. **Generate the test** — AI writes `e2e/specs/task-NNN-*.test.ts` implementing the spec
3. **Set up test environment** — Start Vite dev server + MSEdgeDriver + tauri-driver (see below)
4. **Run the test** — Execute against a running Tauri dev instance
5. **Verify** — Check test output; if it passes, mark the spec `status: verified`
6. **Iterate** — Fix failures and retry until all assertions pass

## Prerequisites

### Required Tools (one-time install)
- Node 20+ with pnpm
- Rust stable with `cargo install tauri-cli --version ^2`
- `cargo install tauri-driver` → `~/.cargo/bin/tauri-driver.exe`
- `npm install -g @sitespeed.io/edgedriver` → MSEdgeDriver

### Runtime Services (start before test run)

```powershell
# ── Terminal 1: Vite Dev Server ──
# IMPORTANT: Use cmd.exe /c — otherwise the process is killed when the
# shell tool times out (see .AiControl/lessons-learned/001-vite-server-survival.md)
Start-Process -WindowStyle Hidden -FilePath "cmd.exe" -ArgumentList @"
/c cd /d D:\work\MyAgent && npx vite --port 5173
"@

# ── Terminal 2: MSEdgeDriver + tauri-driver ──
# MSEdgeDriver first:
Start-Process -WindowStyle Hidden -FilePath "cmd.exe" -ArgumentList @"
/c C:\Users\cheny\AppData\Roaming\npm\node_modules\@sitespeed.io\edgedriver\vendor\msedgedriver.exe --port=9515
"@

# Then tauri-driver pointing to MSEdgeDriver:
Start-Process -WindowStyle Hidden -FilePath "cmd.exe" -ArgumentList @"
/c C:\Users\cheny\.cargo\bin\tauri-driver.exe --native-driver C:\Users\cheny\AppData\Roaming\npm\node_modules\@sitespeed.io\edgedriver\vendor\msedgedriver.exe
"@
```

### Verify all services are up
```powershell
try { $r = Invoke-WebRequest -Uri "http://localhost:5173/" -UseBasicParsing -TimeoutSec 5; "Vite: $($r.StatusCode)" } catch { "Vite: DOWN" }
try { $r = Invoke-WebRequest -Uri "http://127.0.0.1:9515/status" -UseBasicParsing -TimeoutSec 3; "EdgeDriver: $($r.StatusCode)" } catch { "EdgeDriver: DOWN" }
try { $r = Invoke-WebRequest -Uri "http://127.0.0.1:4444/" -UseBasicParsing -TimeoutSec 3; "tauri-driver: $($_.Exception.Message)" } catch { "tauri-driver: running (404 expected)" }
```

## Test File Template

```typescript
import { expect } from '@wdio/globals'

async function waitForAppReady(): Promise<void> {
  await browser.url('http://localhost:5173/')
  await browser.waitUntil(
    async () => {
      const exists = await $('#left-panel').isExisting()
      return exists
    },
    { timeout: 15000, timeoutMsg: 'App did not render left panel within 15s' },
  )
}

describe('Task-NNN: Title', () => {
  before(async () => { await waitForAppReady() })

  it('should ...', async () => {
    // navigate, interact, assert
  })
})
```

## Running Tests

```powershell
# Single spec:
npx wdio run ./e2e/wdio.conf.ts --spec e2e/specs/task-NNN-*.test.ts

# All specs (use explicit paths — glob pattern broken on Windows):
npx wdio run ./e2e/wdio.conf.ts --spec e2e/specs/task-001-*.test.ts e2e/specs/task-002-*.test.ts ...

# Rebuild Rust backend after changes:
cd src-tauri; cargo build; cd ..
```

## CRITICAL GAP — What Tests DON'T Verify

### The Problem

All existing tests bypass Tauri IPC by manipulating Zustand stores directly via `browser.execute()`:

```typescript
// ❌ Direct store manipulation — never goes through IPC
await browser.execute((path) => {
  const store = (window as any).__layoutStore
  store.getState().setProjectPath(path)
})
```

This means tests **never exercise** the real Rust backend IPC handlers:
| Test | What it should test | What it actually tests |
|------|-------------------|----------------------|
| task-002 (open project) | Tauri dialog → select dir → IPC setProjectPath | Zustand `setProjectPath` |
| task-003 (file explorer) | IPC `listDirectory` → render tree | Store has `projectPath` set |
| task-004 (search) | IPC `searchFiles` → display results | Store `setResults()` |
| task-005 (bottom panel) | IPC `spawnTerminal` / `onTerminalOutput` | Store `toggleBottomPanel()` |

### Root Cause

`browser.url('http://localhost:5173/')` navigates the Tauri webview to the Vite dev server. While the IPC bridge (`window.__TAURI_INTERNALS__`) IS available at this URL, the tests never use it — they go directly to Zustand stores exposed on `window.__layoutStore`.

**The fix for future tests**: Prefer real UI interactions (clicks, inputs) over store manipulation. Add an IPC bridge validation test before writing IPC-dependent tests. See `.AiControl/lessons-learned/003-tests-bypass-ipc.md` for detailed guidance.

## Key Lessons Learned

See [lessons-learned index](../lessons-learned/lessons.md) for full write-ups:

- **Lesson 001**: Use `cmd.exe /c` to launch Vite dev server — `Start-Process` without it gets killed on shell timeout
- **Lesson 002**: Tauri v2 command names are exact Rust function names — no `cmd_` prefix. Verify Rust fn name matches `invoke('name')`
- **Lesson 003**: Tests that manipulate stores via `browser.execute` pass but verify nothing about the real backend
- React-resizable-panels v4 treats **number** props as **pixels** — use strings like `"20%"`
- Panel elements use `data-testid` attribute (not `data-panel-id`)
- For drag actions, use `browser.action('pointer').move({ origin: element, x: offset, y: 0 })`
- WDIO spec glob pattern `./e2e/specs/**/*.ts` doesn't work on Windows — use explicit `--spec` flags
- Always add an IPC bridge validation test before relying on Tauri commands in tests
