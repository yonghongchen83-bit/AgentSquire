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
  await browser.url('http://localhost:5173/')  // fresh page load
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

## Key Lessons Learned

- `react-resizable-panels` v4 treats **number** props as **pixels** — use strings like `"20%"`
- `onLayout` is not a valid prop in v4; use `onLayoutChanged` instead
- Panel elements use `data-testid` attribute (not `data-panel-id`)
- For drag actions, use `browser.action('pointer').move({ origin: element, x: offset, y: 0 })`
- Always do `browser.url('http://localhost:5173/')` to get a clean app state
