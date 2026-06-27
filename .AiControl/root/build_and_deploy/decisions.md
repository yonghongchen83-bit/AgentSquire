# Decisions

Build & deploy architecture decisions. See `ArchitecturePlanning/adr/` for full ADR text.

| # | Decision | ADR |
|---|----------|-----|
| 10 | **Dual-platform startup scripts** — `.ps1` (Windows) + `.sh` (macOS/Linux), toolchain version check before launch | 0008 |
| 11 | **tracing-based structured logging** — `tracing-subscriber` + `tracing-appender`, YAML config with per-module level filtering, console + file sinks | 0009 |
| 12 | **Tauri native bundler + auto-update** — platform-specific formats (MSI/DMG/AppImage) via `tauri build`, auto-update via `tauri-plugin-updater` with GitHub Releases | 0010 |
