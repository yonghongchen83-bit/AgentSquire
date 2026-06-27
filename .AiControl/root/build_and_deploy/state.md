# State

## Build & Deploy Planning

Build and deploy process defined. Startup scripts, logging/debug modes, and deployment methods have been decided and documented.

## Deliverables

- `scripts/start-dev.ps1` / `scripts/start-dev.sh` — dev startup scripts
- `scripts/build.ps1` / `scripts/build.sh` — production build scripts
- Logging config: `src-tauri/logging.yaml` — tracing config with per-module levels, rotation
- CI/CD: `.github/workflows/release.yml` — GitHub Actions build + deploy
- Deploy targets: Windows (MSI), macOS (DMG), Linux (AppImage) — via Tauri bundler
- Auto-update: `tauri-plugin-updater` — checks GitHub Releases
