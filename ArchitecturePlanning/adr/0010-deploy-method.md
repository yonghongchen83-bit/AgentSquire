# ADR-0010: Deploy Method

**Status:** Accepted

**Date:** 2026-06-27

## Context

The application needs a repeatable, cross-platform deployment pipeline. Users should receive native installers appropriate for their platform, and updates should be delivered automatically without requiring manual download and reinstallation.

## Decision

We will use Tauri's native bundler exclusively:

| Platform | Format | Mechanism |
|----------|--------|-----------|
| Windows | `.msi` (WiX) | Tauri `tauri build` — WiX MSI |
| macOS | `.dmg` | Tauri `tauri build` — create-dmg |
| Linux | `.AppImage` | Tauri `tauri build` — appimagetool |

### CI/CD Pipeline

- **GitHub Actions**: `.github/workflows/release.yml`
- **Trigger**: Push tag matching `v*`
- **Matrix**: `windows-latest`, `macos-latest`, `ubuntu-latest`
- **Output**: Bundled artifacts attached to GitHub Release

### Auto-Update

- **Library**: `tauri-plugin-updater`
- **Check**: On startup, queries GitHub Releases latest tag
- **Install**: Downloads and applies update silently; rollback on failure

### Code Signing

- **Windows**: Authenticode certificate (Azure Key Vault HSM in CI, self-signed for local dev)
- **macOS**: Apple Developer ID certificate + `gon` for notarization
- **Linux**: No signing required (AppImage is typically unsigned)

## Consequences

### Positive

- Zero extra tooling beyond Tauri's built-in bundlers
- Auto-update keeps users on latest version without manual effort
- GitHub Actions provides free CI minutes for public/open-source repos
- Single CI config covers all three platforms

### Negative

- Windows signing requires a code signing certificate (annual cost ~$200-500)
- macOS notarization requires Apple Developer Program membership ($99/year)
- WiX MSI build requires WiX Toolset installed on Windows runner (~5min additional setup time)
- Auto-update only works with signed releases on macOS
