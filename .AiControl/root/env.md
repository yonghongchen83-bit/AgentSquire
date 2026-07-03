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

## Build & Run Commands

Run from the project root (`D:\work\MyAgent\`). Package manager: `npm` (pnpm preferred where available). Rust commands target `src-tauri/Cargo.toml`.

| Purpose | Command | Notes |
|---------|---------|-------|
| Install JS deps | `npm install` | Run after cloning / lockfile changes |
| Frontend dev server | `npm run dev` | Vite on `http://localhost:5173/` |
| Full app (dev) | `npm run tauri dev` | Launches Tauri shell + Vite; primary dev loop |
| Frontend build | `npm run build` | `tsc -b && vite build` — typechecks then bundles |
| Full app build | `npm run tauri build` | Produces packaged desktop binary |
| Lint | `npm run lint` | `oxlint` |
| Preview built frontend | `npm run preview` | Serves `dist/` |

**Dev build profile:** `src-tauri/Cargo.toml` sets `[profile.dev.package."*"] opt-level = 2`, so third-party deps compile optimized. The **first** `tauri dev` / `cargo build` after a clean (or after any profile/`.cargo` change) recompiles the full dep tree at opt-2 and is slow (heavy crates: arrow, lancedb/lance/datafusion, git2, rusqlite). Subsequent rebuilds only recompile our own crate (`opt-level 0`) and are fast; the optimized deps are cached. Note also that `tauri dev` runs `cargo run --no-default-features` (injected by the Tauri v2 CLI), which uses a separate build cache from a plain `cargo check`/`cargo build` — expect a one-time recompile when switching between them.

**VS Code debugging:** the `Tauri: Debug All` launch config (`.vscode/launch.json`, `lldb`) starts Vite + builds the debug exe via the `build-rust-debug` task. That task uses `cargo build --no-default-features` so it shares the same build cache as `tauri dev` (matching feature set) — no redundant full rebuild when switching between F5-debug and `tauri dev`.

## Test Commands

| Purpose | Command | Notes |
|---------|---------|-------|
| Frontend unit tests | `npm test` | `vitest run` (one-shot) |
| Frontend unit (watch) | `npm run test:watch` | `vitest` |
| Rust tests | `npm run test:rust` | `cargo test --manifest-path src-tauri/Cargo.toml` |
| E2E (WDIO) | `npm run test:e2e` | `wdio run ./e2e/wdio.conf.ts` — requires app + tauri-driver running |
| E2E (dev, auto driver) | `npm run test:e2e:dev` | Starts `tauri-driver` then runs WDIO |

Rust tests can also be run directly: `cargo test --manifest-path src-tauri/Cargo.toml <filter>`.

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
| `D:\work\MyAgent\src-tauri\tests\` | Rust integration tests |
