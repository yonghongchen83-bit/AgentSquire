# ADR-0008: Startup Script Design

**Status:** Accepted

**Date:** 2026-06-27

## Context

The application needs reliable, repeatable startup and build scripts for both development and production use across Windows, macOS, and Linux. Developers frequently need to install dependencies, verify toolchain versions, and launch the app without manually remembering steps.

## Decision

We will provide dual platform-appropriate startup scripts:

- **`scripts/start-dev.ps1`** (Windows PowerShell 5.1+) — installs Node/Rust deps if missing, runs `npm run tauri dev`
- **`scripts/start-dev.sh`** (Bash, macOS/Linux) — equivalent functionality
- **`scripts/build.ps1`** / **`scripts/build.sh`** — production build via `npm run tauri build`

Each script performs:
1. Toolchain version check (Node >=20, Rust stable via rustup, cargo)
2. `npm ci` (clean install) or `pnpm install` if `node_modules` missing/stale
3. Launch the target command

## Consequences

### Positive

- Zero-guess startup for new contributors — single command to run
- CI-ready: same scripts used in GitHub Actions matrix builds
- Platform-appropriate: idiomatic PowerShell on Windows, Bash elsewhere

### Negative

- Dual script maintenance — changes must be mirrored in both
- PowerShell execution policy may block `.ps1` scripts on some Windows setups
