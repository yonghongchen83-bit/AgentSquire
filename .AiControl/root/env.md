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

## 🚫 CRITICAL RULE: Scripts Only — No Manual Cargo Commands

**Do NOT run `cargo build`, `cargo test`, `cargo run`, `cargo clean`, or `npm run tauri dev` directly in the terminal.** Every Rust compilation action MUST go through the fixed scripts in `scripts/`. These scripts share one build cache (the `--no-default-features` feature set, matching what `tauri dev` uses) and prevent accidental full rebuilds.

Each script is a fixed-purpose wrapper that accepts NO arbitrary CLI arguments. They must be invoked as-is.

| Script | Purpose | Notes |
|--------|---------|-------|
| `scripts/build.ps1` | Build Rust backend (debug) | Uses existing cache — fast incremental |
| `scripts/run.ps1` | Launch full app (tauri dev) | Builds Rust + starts Vite |
| `scripts/test.ps1` | Run Rust unit tests (`--lib`) | Fast — no integration tests |
| `scripts/test-all.ps1` | Run ALL Rust tests | Slower — includes integration tests |
| `scripts/frontend-test.ps1` | Run frontend Vitest tests | No Rust compilation needed |
| `scripts/clean.ps1` | **Full clean + rebuild (~30 min)** | **REQUIRES user confirmation** — never run automatically |

### How to invoke scripts

```powershell
# From the project root:
.\scripts\build.ps1
.\scripts\test.ps1
.\scripts\test-all.ps1
.\scripts\run.ps1
.\scripts\frontend-test.ps1
```

Or via npm scripts:
```bash
npm run build:rust
npm run test:rust
npm run clean:rust
```

Or via VS Code tasks (F5 / Run Task):
- `build-rust-debug` → runs `scripts/build.ps1`
- `test-rust` → runs `scripts/test.ps1`
- `test-rust-all` → runs `scripts/test-all.ps1`
- `test-frontend` → runs `scripts/frontend-test.ps1`
- `run-app` → runs `scripts/run.ps1`

### ⚠️ Clean/Rebuild Protocol

A full clean rebuild takes **~30 minutes** (arrow, lancedb/lance/datafusion, git2, rusqlite all recompile at opt-level 2). **Before running `scripts/clean.ps1`, the AI MUST ask the user for explicit permission.** The script itself also requires the user to type "yes" to proceed. Never call it autonomously.

### Dev build profile

`src-tauri/Cargo.toml` sets `[profile.dev.package."*"] opt-level = 2`, so third-party deps compile optimized once and are cached. Our own crate compiles at `opt-level 0` for fast iterative rebuilds. `tauri dev` runs `cargo run --no-default-features` under the hood; the build scripts use the identical feature set, so there's no cache split.

### VS Code debugging (F5)

The `Tauri: Debug All` launch config (`.vscode/launch.json`, `lldb`) chains through the `build-rust-debug` task, which now runs `scripts/build.ps1` — same cache, same feature set, no redundant rebuild.

**E2E prerequisite (Windows):** the Vite dev server must be launched decoupled from the shell tool's process tree, or it is killed on timeout. Use `cmd.exe /c` and verify `http://localhost:5173/` returns 200 before running WDIO. See [lessons-learned/001-vite-server-survival.md](../lessons-learned/001-vite-server-survival.md).

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
| `D:\work\MyAgent\src-tauri\src\llm\openai.rs` | OpenAI provider with verbose wire logging |
| `D:\work\MyAgent\src-tauri\src\state\config.rs` | Config directory resolution |

## Verbose Wire Log

When verbose mode is enabled (Settings → `verboseLogging`), the OpenAI provider dumps all requests, responses, and stream events to:

```
C:\Users\cheny\AppData\Roaming\com.squirecli.app\config\provider-wire.log
```

This is controlled by `AppConfig.verbose_logging` → `OpenAIProvider.verbose` → `append_wire_log()` in `src-tauri\src\llm\openai.rs:35-48`.