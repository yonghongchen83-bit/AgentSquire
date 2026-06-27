# Env

## Startup Scripts

| Script | Path | Purpose |
|--------|------|---------|
| `start-dev.ps1` | `scripts/start-dev.ps1` | Dev: toolchain check + `npm run tauri dev` (Windows) |
| `start-dev.sh` | `scripts/start-dev.sh` | Dev: toolchain check + `npm run tauri dev` (macOS/Linux) |
| `build.ps1` | `scripts/build.ps1` | Production: `npm run tauri build` (Windows) |
| `build.sh` | `scripts/build.sh` | Production: `npm run tauri build` (macOS/Linux) |

## Log / Debug Mode

- **Framework**: `tracing` (Rust) + `tracing-subscriber` + `tracing-appender`
- **Sinks**: Console (stdout, colorized in dev) + rolling file (`{app_data_dir}/logs/`)
- **Levels**: ERROR, WARN, INFO, DEBUG, TRACE — from `--verbose` flag or config
- **Config file**: `src-tauri/logging.yaml` — per-module level, file path, rotation policy
- **Frontend debug**: `localStorage.debug='app:*'` + React DevTools

## Deploy Methods

| Platform | Format | Bundler |
|----------|--------|---------|
| Windows | `.msi` (WiX) | Tauri `tauri build` |
| macOS | `.dmg` | Tauri `tauri build` (create-dmg) |
| Linux | `.AppImage` | Tauri `tauri build` (appimagetool) |

- **CI/CD**: GitHub Actions — `.github/workflows/release.yml` (trigger: tag `v*`)
- **Auto-update**: `tauri-plugin-updater` — checks GitHub Releases on startup
- **Signing**: Windows Authenticode cert; macOS Apple Developer ID + notarization
