# ADR-0001: Use Tauri as Desktop Framework

**Status:** Accepted

**Date:** 2026-06-26

## Context

We are building an opencode-like agent desktop app client. We need a cross-platform desktop framework that:

- Provides native-like performance and small binary size
- Allows using web-based UI (HTML/CSS/JS) for the frontend
- Supports both Windows and macOS (and ideally Linux)
- Has good security model (sandboxing, CSP)
- Integrates well with Rust for backend logic

## Decision

We will use **Tauri v2** as the desktop application framework.

Tauri was chosen over alternatives (Electron, NW.js, .NET MAUI, Flutter) because:

- **Binary size**: Tauri apps are ~10MB vs Electron's ~150MB+ baseline
- **Memory footprint**: Significantly lower RAM usage than Electron
- **Security**: Tauri's IPC model with allow-listed commands is more secure than Node.js integration
- **Rust backend**: Native performance, memory safety, and rich ecosystem for system-level operations
- **Flexible frontend**: Any web framework (React, Vue, Svelte, etc.) can be used
- **Mature v2 release**: Tauri v2 is stable with plugin ecosystem

## Consequences

### Positive

- Small distribution size and fast startup for end users
- Lower resource usage compared to Electron-based alternatives
- Rust enables safe system-level operations (file system, process management, etc.)
- Strong security model with capability-based permissions
- Active community and growing plugin ecosystem

### Negative

- Rust has a steeper learning curve vs JavaScript/TypeScript-only stacks
- Smaller ecosystem of plugins compared to Electron
- Some platform-specific features may require custom Rust code
- WebView rendering can have subtle cross-platform differences

## Alternatives Considered

- **Electron**: Rejected due to large binary size, high memory usage, and broader attack surface
- **.NET MAUI / WPF**: Rejected due to Windows-only or weaker cross-platform support
- **Flutter Desktop**: Rejected due to less mature ecosystem for system-level integrations
- **Go + WebView**: Rejected due to less mature desktop framework options
