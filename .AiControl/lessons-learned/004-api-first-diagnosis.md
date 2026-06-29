# Lesson 004: Fixing bugs requires reproduction first — never skip to code reading

## Problem
User reported "test connection doesn't work with my key". Instead of reproducing the bug (physically clicking the button or calling the API with curl), I skipped straight to reading frontend and backend code. I then "fixed" unrelated UI bugs and claimed the issue was resolved without ever confirming the original failure. The actual cause — the OpenCode Zen API returning HTTP 500 for all chat endpoints — was never addressed.

## Symptoms
- User reported test connection failure with a valid API key
- I never pressed the "Test Connection" button to observe the actual error
- I never confirmed the API key provided was the one I was testing
- I analyzed code instead of reproducing the failure
- I made multiple unrelated changes (dialog close guard, initialTab, provider select logic, model tag list) and declared the bug fixed
- None of those changes addressed the actual symptom

## Root Cause
**I violated the most fundamental rule of debugging: reproduce the bug before touching any code.**

Correct debugging process:
1. **Reproduce** — physically observe the failure or construct undeniable logical proof
2. **Diagnose** — trace from observed failure to root cause
3. **Design fix** — change only what addresses the root cause
4. **Verify** — repeat reproduction procedure; bug must be gone

What I did instead: skipped step 1, made assumptions about the cause, changed unrelated code, and declared success. This is not debugging — it's guessing.

## Fix
Never modify code before reproducing the bug. The reproduction procedure is:

```
1. Confirm the exact user input (key, model, endpoint)
2. Physically trigger the failing action (click the button, call the API)
3. Observe the actual error message
4. Only then start tracing from that observation
```

For API integration bugs specifically, reproduction means calling the real API with curl:

```powershell
curl.exe -X POST "https://opencode.ai/zen/v1/chat/completions" ^
  -H "Authorization: Bearer $KEY" ^
  -H "Content-Type: application/json" ^
  -d '{"model":"deepseek-v4-flash-free","messages":[{"role":"user","content":"Say ok"}],"max_tokens":50}'
```

## Prevention
The project now has a formal development workflow at `.opencode/rules/workflow.md` that enforces:

- **Bug fixing**: reproduce → diagnose → fix → verify (same reproduction procedure)
- **Features**: gather requirement → design → implement → test against requirement
- **Prohibited**: modifying code before reproduction, fixing unrelated issues, assuming user error without physical reproduction

This file is loaded as instructions via `opencode.json` and must be read before any coding task.

## Related
- [Workflow Rules](../../.opencode/rules/workflow.md) — the formal development process document
- [001](./001-vite-server-survival.md) — Shell tool timeout kills child processes (another "test the real thing" lesson)
- [003](./003-tests-bypass-ipc.md) — Mock-based tests give false confidence
