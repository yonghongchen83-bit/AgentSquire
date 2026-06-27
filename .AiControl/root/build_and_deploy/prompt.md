# Prompt

Goal: Set up build, startup, logging, and deployment for the app.

## Tasks

1. **Startup Script** — Create dev startup scripts (`npm run tauri dev` wrapper) and production build scripts. Dual-platform: `.ps1` for Windows, `.sh` for macOS/Linux.
2. **Log / Debug Mode** — Configure structured logging with tracing. Support `--debug` / `--verbose` flags. File + console sinks with rotation. Per-module level filtering.
3. **Deploy Method** — Define concrete build pipeline. Tauri `tauri build` for each platform. Windows: MSI. macOS: DMG. Linux: AppImage. Auto-update via `tauri-plugin-updater`.
4. **Record Decisions** — Write ADR-0008 (Startup Script), ADR-0009 (Log/Debug Mode), ADR-0010 (Deploy Method).
