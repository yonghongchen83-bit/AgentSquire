# Env

## Toolchain

| Tool | Version | Notes |
|------|---------|-------|
| Rust | stable | via rustup |
| Node | 20+ | via nvm-windows / fnm |
| npm / pnpm | latest | pnpm preferred for workspace support |
| Tauri CLI | v2 | `cargo install tauri-cli --version ^2` |
| tauri-driver | 2.0.6 | `cargo install tauri-driver` → `~/.cargo/bin/tauri-driver.exe` |
| VS Code | latest | extensions: rust-analyzer, Tailwind CSS, ESLint |

## Platform Targets

| Platform | WebView | Notes |
|----------|---------|-------|
| Windows 10/11 | WebView2 (built-in Win11, runtime Win10) | Primary dev target |
| macOS 12+ | WKWebView | Verify before release |
| Linux | WebKitGTK | Deferred |

## Project Paths

| Path | Purpose |
|------|---------|
| `D:\work\MyAgent\` | Project root |
| `D:\work\MyAgent\ArchitecturePlanning\` | Design docs & ADRs |
| `D:\work\MyAgent\.AiControl\` | Node documents per phase |
| `D:\work\MyAgent\src\` | Frontend source |
| `D:\work\MyAgent\src-tauri\` | Rust backend source |
| `D:\work\MyAgent\e2e\` | E2E test specs & WDIO config |
| `D:\work\MyAgent\src-tauri\tests\` | Rust integration tests |
