# Env — Phase 1

See `.AiControl/root/env.md` for full toolchain setup.

Phase-specific:
- Rust nightly not required — stable is fine
- Test IPC commands via `cargo test` or Tauri's `invoke` API in devtools console
- Use `cargo watch -x run` for hot-reload during development
