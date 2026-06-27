# Decisions

| # | Decision |
|---|----------|
| 1 | **tauri-driver + WDIO** for E2E testing — chosen over Playwright because Tauri officially supports WDIO via `@tauri-apps/test` ecosystem |
| 2 | **Separate e2e/tsconfig.json** — E2E specs need commonjs module resolution (WDIO requirement), separate from frontend bundler-based tsconfig |
| 3 | **No data-testid yet** — initial tests use generic CSS selectors; components should be annotated as a follow-up for reliable targeting |
| 4 | **Rust tests in src-tauri/tests/** — integration tests live in the standard Rust `tests/` directory, compiled as a separate binary |
