# AI-Driven UI Test Skill

## Concept

Each UI test case is defined as a **markdown task spec** in `e2e/tasks/`. The AI reads the spec, generates a WebdriverIO test file, and runs it against the running Tauri app.

## Workflow

1. **Define the task** — Create `e2e/tasks/task-NNN.md` with: description, steps, expected results, and selectors
2. **Generate the test** — AI writes `e2e/specs/task-NNN-*.test.ts` implementing the spec
3. **Run the test** — Execute against a running Tauri dev instance
4. **Verify** — Check test output; if it passes, mark the spec `status: verified`
5. **Iterate** — Fix failures and retry until all assertions pass

## Prerequisites

- `pnpm dev` running (Vite dev server on port 5173)
- `tauri-driver.exe` running (`cargo install tauri-driver`; needs MSEdgeDriver in PATH)
- The Tauri app running (`npx tauri dev`)
- MSEdgeDriver: installed via `npm install -g @sitespeed.io/edgedriver`

## Test File Template

```typescript
import { expect } from '@wdio/globals'

async function waitForAppReady(): Promise<void> {
  // IMPORTANT: Do NOT navigate away from the Tauri origin.
  // The webview already starts at the app URL (tauri://localhost or similar).
  // Only do browser.url() if you need to hard-reload to clear state.
  // If you must reload, use the Tauri origin, NOT localhost:5173.
  await browser.url('https://tauri.localhost/')
  await browser.waitUntil(
    async () => { /* wait for key element */ },
    { timeout: 15000 }
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
# From project root, with tauri dev + tauri-driver already running:
npx wdio run ./e2e/wdio.conf.ts --spec e2e/specs/task-NNN-*.test.ts
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

`browser.url('http://localhost:5173/')` navigates the Tauri webview **away from the Tauri origin** to the Vite dev server. At `localhost:5173`:

- `window.__TAURI_INTERNALS__` may not be available or may behave differently
- `@tauri-apps/api` calls (`invoke`, event listeners like `onTerminalOutput`) fail silently
- The IPC bridge is tied to the Tauri origin (`tauri://localhost`, `https://tauri.localhost/`, or the app's configured dev URL)

**The correct origin for IPC during `tauri dev` is NOT `localhost:5173`**. The webview's actual URL is something like `https://tauri.localhost/` which loads content proxied from Vite. Navigating to raw `localhost:5173` skips the IPC injection that happens at the Tauri origin.

### How to Close the Gap

**1. Use the Tauri origin in tests**

Replace `browser.url('http://localhost:5173/')` with the Tauri app's actual URL:

```typescript
// Use the Tauri origin — IPC bridge is active here
await browser.url('https://tauri.localhost/')
// or use browser.reloadSession() to clear state without navigating
```

To find the exact Tauri dev URL, check:
- Console output when running `npx tauri dev`
- Or inspect `window.location.origin` in the running webview

**2. Test IPC directly before UI testing**

Add a validation test that confirms IPC works end-to-end:

```typescript
it('IPC bridge should be available', async () => {
  const hasBridge = await browser.execute(() => {
    return typeof window.__TAURI_INTERNALS__?.invoke === 'function'
  })
  expect(hasBridge).toBe(true)
})

it('listDirectory IPC should return entries', async () => {
  const entries = await browser.execute(async (path) => {
    try {
      return await window.__TAURI_INTERNALS__.invoke('plugin:fs|read_dir', { path })
    } catch (e) {
      return { error: String(e) }
    }
  }, 'D:\\work\\MyAgent')
  expect(Array.isArray(entries)).toBe(true)
  expect(entries.length).toBeGreaterThan(0)
})
```

**3. Remove store-exposure workarounds**

Once IPC works, remove `window.__layoutStore` / `window.__searchStore` from `App.tsx`. Tests should interact through the **actual UI** (clicks, inputs, waits), not store manipulation.

**4. Test via real UI interactions, not execute()**

Instead of:
```typescript
await browser.execute(() => {
  const store = (window as any).__layoutStore.getState()
  store.setProjectPath('D:\\work\\MyAgent')
})
```

Do:
```typescript
const openBtn = await $('button=Open Project')
await openBtn.click()
// ... interact with the actual Tauri dialog
// (may require OS-level automation for native dialogs)
```

For native dialogs (file open, etc.) that can't be automated via WebDriver, consider:
- Preselecting paths via Tauri's CLI flags or env vars
- Mocking the dialog response on the Rust side for dev mode
- Using `tauri::api::dialog::ask` with auto-approve in dev builds

### What to Verify When IPC Works

Once the Tauri origin is used and IPC is functional, run these checks:

| Check | Method | Expected |
|-------|--------|----------|
| `listDirectory` | `browser.execute` invoking IPC directly | Returns array of FileEntry |
| `searchFiles` | `browser.execute` invoking IPC directly | Returns matching results |
| `spawnTerminal` | `browser.execute` invoking IPC directly | Returns terminal ID |
| `onTerminalOutput` | Subscribe via IPC, then write to terminal | Data callback fires |
| `gitStatus` | `browser.execute` invoking IPC directly | Returns status string |
| `onFsChange` | Create a file on disk via Node | RefreshTree fires |

Only after these pass should you write UI tests that rely on them.

## Key Lessons Learned

- `react-resizable-panels` v4 treats **number** props as **pixels** — use strings like `"20%"`
- `onLayout` is not a valid prop in v4; use `onLayoutChanged` instead
- Panel elements use `data-testid` attribute (not `data-panel-id`)
- For drag actions, use `browser.action('pointer').move({ origin: element, x: offset, y: 0 })`
- **DO NOT use `browser.url('http://localhost:5173/')`** — navigates away from Tauri origin, breaks IPC. Use `https://tauri.localhost/` or find the actual Tauri dev URL.
- Tests that bypass IPC via `browser.execute` on Zustand stores pass but verify NOTHING about the real backend
- Always add an IPC bridge validation test before relying on Tauri commands in tests
- The xterm-terminal component crashes if IPC event listeners (`onTerminalOutput`, `onTerminalExit`) fail. Wrap them in try/catch. Fix is in `src/components/xterm-terminal.tsx`.
