---
name: ui-business-test
description: "Use when writing, running, or fixing WDIO E2E tests for a Tauri v2 application. Covers test generation from markdown specs in e2e/tasks/, running tests against the Vite dev server, fixing IPC command failures, and setting up tauri-driver + MSEdgeDriver. Use ONLY for E2E UI test tasks — not for unit tests, integration tests, or non-test frontend work."
---

# UI Business Test Skill

## Concept
Each UI test case is defined as a markdown task spec in `e2e/tasks/`. The AI reads the spec, generates a WebdriverIO test file, and runs it against the running Tauri app.

## Workflow
1. Define the task — Create `e2e/tasks/task-NNN.md` with description, steps, expected results, selectors
2. Generate the test — Write `e2e/specs/task-NNN-*.test.ts`
3. Set up test environment — Start Vite dev server + MSEdgeDriver + tauri-driver
4. Run the test — `npx wdio run ./e2e/wdio.conf.ts --spec e2e/specs/task-NNN-*.test.ts`
5. Verify — Check test output; if pass, mark spec `status: verified`
6. Iterate — Fix failures and retry

## Critical: Starting Services
All three services MUST be running. Use `cmd.exe /c` to decouple from shell timeout:
```powershell
Start-Process -WindowStyle Hidden -FilePath "cmd.exe" -ArgumentList "/c cd /d D:\work\MyAgent && npx vite --port 5173"
Start-Process -WindowStyle Hidden -FilePath "cmd.exe" -ArgumentList "/c <msedgedriver-path> --port=9515"
Start-Process -WindowStyle Hidden -FilePath "cmd.exe" -ArgumentList "/c <tauri-driver-path> --native-driver <msedgedriver-path>"
```

## Critical Gap: Tests Bypass IPC
Tests use `browser.execute()` to manipulate Zustand stores directly — they NEVER exercise real Rust IPC handlers. File tree, search, terminal, and open-project tests pass but verify nothing about the backend. Prefer real UI interactions over store manipulation. Add an IPC bridge validation test before writing IPC-dependent tests.

## Key Conventions
- Use `cmd.exe /c` for all long-running server processes (see Lesson 001)
- Tauri v2 command names are exact Rust function names — no `cmd_` prefix (see Lesson 002)
- Tests that bypass IPC give false confidence — prefer real UI interactions (see Lesson 003)
- `react-resizable-panels` v4: number props = pixels, string props = percentages
- Panel elements use `data-testid` attribute
- For drag actions: `browser.action('pointer').move({ origin: element, x: offset, y: 0 })`
