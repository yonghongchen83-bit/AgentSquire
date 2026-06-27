# State

Integration & System Testing — in progress.

## Goals
- Full `cargo tauri build` — both Rust + frontend compile cleanly
- `cargo tauri dev` launches without runtime panics
- Manual smoke test of every feature zone
- Fix regressions uncovered during integration

## Workflow
1. Build Rust backend (`cargo check`, `cargo build`)
2. Build frontend (`pnpm build`)
3. Full Tauri build (`cargo tauri build`)
4. Launch dev mode and smoke-test each panel
5. Log any issues and fix them in their respective phase nodes
