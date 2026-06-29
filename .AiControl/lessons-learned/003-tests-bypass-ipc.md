# Lesson 003: Tests Bypass IPC via Store Manipulation

## Problem

E2E tests pass but verify nothing about the real backend. A test can be green while the feature is completely broken. This creates false confidence and wastes debugging time.

## Symptoms

- All tests pass, but the app doesn't work in practice
- `browser.execute()` calls are the primary interaction method, not `element.click()`
- Test assertions check for placeholder text or error messages instead of real content
- Adding new IPC-based features leads to mysterious failures that tests don't catch

## Root Cause

Tests use `browser.execute()` to manipulate Zustand stores directly, bypassing the entire IPC pipeline:

```typescript
// ❌ Bypasses IPC entirely — only tests that the store can hold a value
await browser.execute((path) => {
  const store = (window as any).__layoutStore
  store.getState().setProjectPath(path)
})

// ✓ Correct — tests the full UI flow
const btn = await $('button=Open Project')
await btn.click()
// ... interact with Tauri dialog, wait for file tree to populate
```

The `waitForAppReady` function in every test file navigates to `http://localhost:5173/` (raw Vite dev server). IPC commands like `listDirectory`, `spawnTerminal`, `onTerminalOutput` may behave differently here than in the Tauri origin context.

**What tests actually verify vs. what they should verify:**

| Test | What it tests | What it should test |
|------|--------------|-------------------|
| task-002 (open project) | `window.__layoutStore.getState().setProjectPath()` works | Tauri dialog → IPC setProjectPath → StatusBar updates |
| task-003 (file explorer) | Store has `projectPath` set, tree div renders | IPC `listDirectory` → files appear in tree |
| task-004 (search) | `window.__searchStore.getState().setResults()` renders items | IPC `searchFiles` → results display |
| task-005 (bottom panel) | Clicking "Show Terminal" toggles visibility | IPC `spawnTerminal` → terminal renders without crash |

## Fix

1. **task-005**: Replaced `store.setState({ bottomPanelVisible: true, bottomPanelActiveTab: 'output' })` with clicking the real "Show Terminal" button. This tests the xterm mount path.

2. **task-003**: Removed the `"(empty due to IPC in browser)"` caveat. Now asserts the tree has either files, an error message, or "Empty directory" — acknowledges the gap while still checking something meaningful.

3. **Updated the skill** with a CRITICAL GAP section documenting the issue and how to close it.

## Prevention

1. **Add an IPC bridge validation test** before running any IPC-dependent tests:

   ```typescript
   it('IPC bridge should be available', async () => {
     const hasBridge = await browser.execute(() => {
       return typeof window.__TAURI_INTERNALS__?.invoke === 'function'
     })
     expect(hasBridge).toBe(true)
   })
   ```

2. **Test via real UI interactions** — clicks, inputs, waits. Reserve `browser.execute` for setup/teardown only.

3. **Don't expose internal stores** for test purposes. Remove `window.__layoutStore` and `window.__searchStore` from production code. Tests should interact through the same surface as users.

4. **Test on the Tauri origin** — use `browser.url('https://tauri.localhost/')` instead of `http://localhost:5173/` if IPC behaves differently at the dev server URL.

5. **For native dialogs** (file picker, etc.):
   - Preselect paths via Tauri CLI flags or env vars in dev mode
   - Mock dialog responses on the Rust side for test builds
   - Use `tauri::api::dialog::ask` with auto-approve in dev builds

## Related

- [Lesson 001](./001-vite-server-survival.md) — keeping the dev server alive for tests
- [Lesson 002](./002-tauri-command-naming.md) — fixing IPC command names so they actually work
