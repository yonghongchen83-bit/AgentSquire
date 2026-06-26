# Dependency Report

> Full inventory of every language, package, crate, binary, and system dependency required.

---

## 1. Rust Crates (`Cargo.toml`)

### Core Framework

| Crate | Version | Size | Why | Source |
|-------|---------|------|-----|--------|
| `tauri` | v2 | ~15MB (binary) | Desktop shell, window mgmt, IPC | Official |
| `tauri-build` | v2 | — | Build scripts | Official |
| `serde` | 1.x | ~50KB | Config serialization | Official |
| `serde_json` | 1.x | ~100KB | JSON IPC messages | Official |
| `toml` | 0.8 | ~30KB | Config file format (TOML) | Official |
| `tokio` | 1.x | ~200KB | Async runtime (comes with Tauri) | Official |
| `tracing` | 0.1 | ~30KB | Structured logging | Official |

### Tauri Plugins

| Plugin | Why |
|--------|-----|
| `tauri-plugin-shell` | Spawn processes, terminal pty |
| `tauri-plugin-fs` | File read/write operations |
| `tauri-plugin-dialog` | Native file dialogs |
| `tauri-plugin-updater` | Auto-update |

### LLM Provider SDKs

| Crate | Size | Why |
|-------|------|-----|
| `async-openai` | ~200KB | OpenAI / Azure / compatible APIs |
| `anthropic-sdk-rust` | ~150KB | Claude API |
| `llama-cpp-rs` | ~500KB | Local models (optional feature) |

### Storage & Data

| Crate | Size | Why |
|-------|------|-----|
| `rusqlite` | ~600KB | SQLite — conversation history |
| `chrono` | ~100KB | Timestamps for conversations |

### File System & Search

| Crate | Size | Why |
|-------|------|-----|
| `notify` | ~100KB | File watching (momoi-explorer adapter) |
| `grep` (ripgrep crate) | ~200KB | Programmatic grep (alternative to shelling out) |
| `ignore` (ripgrep crate) | ~100KB | Gitignore-aware file walking |

### Git

| Crate | Size | Why |
|-------|------|-----|
| `git2` | ~2MB (libgit2) | Git operations via native libgit2 binding |

### Telemetry (opt-in)

| Crate | Size | Why |
|-------|------|-----|
| `sentry` | ~200KB | Crash reporting (optional feature flag) |

### Dev Dependencies (Cargo)

| Crate | Why |
|-------|-----|
| `tauri-cli` | Build & dev tooling |

**Rust total:** ~20 crates, ~4MB compiled deps (before LTO/stripping)

---

## 2. npm Packages (`package.json`)

### Core Framework

| Package | Version | Size (gzip) | Why |
|---------|---------|-------------|-----|
| `react` | 19 | ~12KB | Frontend framework |
| `react-dom` | 19 | ~130KB | DOM renderer |
| `typescript` | 5.x | — | Dev only |
| `vite` | 6.x | — | Dev only — bundler |
| `@vitejs/plugin-react` | — | — | Vite React plugin |

### UI Layer

| Package | Size (gzip) | Why |
|---------|-------------|-----|
| `tailwindcss` v4 | ~2KB (runtime) | CSS framework (mostly dev) |
| `@tailwindcss/typography` | — | Prose styling for markdown |
| `class-variance-authority` | <1KB | shadcn variant API |
| `clsx` | <1KB | Class merging |
| `tailwind-merge` | ~2KB | Tailwind class dedup |
| `lucide-react` | ~30KB (tree-shaken) | Icons |
| `@radix-ui/*` (various) | varies | shadcn primitives (accordion, context-menu, dialog, dropdown, scroll-area, select, separator, tooltip, tabs) — ~2-5KB each |

### Copy-Paste Components (zero runtime deps)

| Component | Actual Deps |
|-----------|-------------|
| **shadcn/ui** (button, input, card, badge, etc.) | None — code in our repo |
| **shadcn-chatbot-kit** | None — code in our repo |

These are installed via `npx shadcn@latest add` — the source is copied into our codebase. No `package.json` entries.

### Monaco Editor

| Package | Size (gzip) | Why |
|---------|-------------|-----|
| `@monaco-editor/react` | ~2KB | React wrapper |
| `monaco-editor` | ~2MB | The editor engine (tree-shakeable to ~1MB) |

### Chat & Markdown

| Package | Size (gzip) | Why |
|---------|-------------|-----|
| `react-markdown` | ~10KB | Markdown rendering |
| `remark-gfm` | ~5KB | GitHub-flavored markdown |
| `rehype-raw` | ~3KB | Raw HTML in markdown |
| `rehype-highlight` | ~5KB | Code syntax highlighting |
| `react-syntax-highlighter` | ~50KB | Alternative code highlighting (if not using rehype) |

### Terminal

| Package | Size (gzip) | Why |
|---------|-------------|-----|
| `@xterm/xterm` | ~300KB | Terminal emulator |
| `@xterm/addon-fit` | ~3KB | Auto-fit terminal to container |
| `@xterm/addon-web-links` | ~2KB | Clickable links in terminal |

### File Explorer

| Package | Size (gzip) | Why |
|---------|-------------|-----|
| `momoi-explorer` | ~30KB | Core file explorer engine |
| `momoi-explorer/react` | ~10KB | React bindings |

### State Management

| Package | Size (gzip) | Why |
|---------|-------------|-----|
| `zustand` | ~1KB | UI-only transient state |
| `@tanstack/react-query` | ~12KB | IPC data fetching & caching |

### Telemetry (opt-in)

| Package | Size (gzip) | Why |
|---------|-------------|-----|
| `@sentry/react` | ~30KB | Error tracking (optional) |

### npm total

| Category | Count | Bundle size |
|----------|-------|-------------|
| Runtime deps | ~25 packages | ~1.8MB gzip |
| Dev-only | 5 packages | — |
| Copy-paste (shadcn) | ~20+ components | Zero — in our code |

---

## 3. Bundled Binaries

| Binary | Size | Why |
|--------|------|-----|
| `rg` (ripgrep) | ~5MB | Code search engine — bundled with Tauri resources |
| `libgit2` | ~2MB | Linked via `git2` crate — not a separate binary |

**Binary total:** ~5MB (ripgrep)

---

## 4. System Dependencies (OS-level, not in project)

| Dependency | Windows | macOS | Linux |
|-----------|---------|-------|-------|
| **WebView2** | Built into Win 11 / available via runtime | — | — |
| **WebKit** | — | Built into macOS | `libwebkit2gtk-4.1` |
| **GTK** | — | — | `libgtk-3` + `libayatana-appindicator` |
| **Git** | Optional (user-installed) | Optional | Optional |

These are Tauri v2 prerequisites. Not shipped by us — required at runtime.

---

## 5. Grand Total

| Category | Count | Size Impact |
|----------|-------|-------------|
| Rust crates | ~20 | ~4MB compiled |
| npm runtime | ~25 | ~1.8MB gzip |
| npm dev-only | 5 | 0 (not bundled) |
| shadcn components | ~20+ | 0 (source in repo) |
| Bundled binaries | 1 (ripgrep) | ~5MB |
| System deps | 3 | not shipped by us |
| **App binary (estimated)** | — | **~12-15MB** (typical Tauri + deps after stripping) |

Compare to Electron baseline: **~150MB**.

---

## 6. Dependency Graph (Visual)

```
react-markdown ─┬─ remark-gfm
                ├─ rehype-raw
                └─ rehype-highlight

zustand ─── (standalone, 0 deps)

@tanstack/react-query ─── @tanstack/query-core

@monaco-editor/react ─── monaco-editor

@xterm/xterm ─── (standalone, 0 deps)

momoi-explorer ─── momoi-explorer/core(framework-agnostic)
         └── momoi-explorer/react

shadcn/ui ─── @radix-ui/* (per-component, tree-shakeable)
         └── tailwind-merge + clsx + class-variance-authority

───────────────────────────────────── Rust side ─────────────────────────────────────

tauri ─┬─ tauri-plugin-shell
       ├─ tauri-plugin-fs
       ├─ tauri-plugin-dialog
       └─ tauri-plugin-updater

rusqlite ─── libsqlite3-sys (bundled SQLite C lib)

async-openai / anthropic-sdk-rust ─── reqwest (HTTP client)

git2 ─── libgit2-sys (bundled libgit2 C lib)

grep + ignore ─── (ripgrep internals, no extra linking)

notify ─── (cross-platform file watcher)
```

## 7. Zero-Dependency Components (our code, not borrowed)

These are not dependencies — they're our own code. Listed for completeness:

| Component | Est. LOC |
|-----------|----------|
| LLM Provider trait + impls | ~300 |
| Conversation Store trait + SQLite impl | ~200 |
| Chat block rendering (thinking, tool calls, etc.) | ~400 |
| Search panel UI (over ripgrep) | ~250 |
| File tree styling (over momoi-explorer) | ~200 |
| File system adapter (Tauri ↔ momoi) | ~80 |
| Agent tool implementations | ~500+ (grows) |
| Settings UI | ~300 |
| **Our total code** | **~2,200+ LOC (grows with features)** |
