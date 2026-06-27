# Prompt

Goal: Automated UI verification so AI can validate UI behavior without human testers.

## Architecture

Three-tier testing:
1. **Rust** — `#[cfg(test)]` unit tests + `src-tauri/tests/` integration tests
2. **Frontend** — Vitest + `@testing-library/react` for components and stores
3. **E2E** — WebDriverIO + `tauri-driver` for full-stack WebView testing

## Key Constraint

E2E tests require the app binary and `tauri-driver` to be running. See `env.md` for startup instructions.

## When adding new features
- Add `data-testid` attributes to new components for E2E selector reliability
- Write a Rust unit test for backend logic
- Write a Vitest test for frontend logic
- Write a WDIO E2E test for end-to-end UI flows

## Available Scripts
- `npm test` — frontend unit tests (Vitest)
- `npm run test:rust` — all Rust tests (unit + integration)
- `npm run test:e2e` — E2E tests (requires app + tauri-driver running)
