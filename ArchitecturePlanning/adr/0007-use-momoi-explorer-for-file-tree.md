# ADR-0007: Use momoi-explorer for File Tree

**Status:** Accepted

**Date:** 2026-06-26

## Context

The app needs a file explorer sidebar — the most complex UI component in the project. Building one from scratch requires:

- Directory tree state management (expand/collapse, lazy loading)
- File system watching (detect external file changes, keep tree in sync)
- Event coalescing (handle rapid file change bursts without re-render storms)
- File operations: create, rename, delete, move
- Drag and drop reordering
- Virtualization for large projects
- Multiselect

This is months of bug-prone work. Off-the-shelf tree components (react-arborist, Arborix) handle tree rendering but not file system sync.

## Decision

Use **momoi-explorer** — a headless file explorer engine with React bindings. It provides:

1. **Framework-agnostic core** — tree state machine, file watching (VS Code-compatible debounce/coalesce/throttle), change detection
2. **React bindings** — hooks + context for tree state, file operations, selection
3. **Adapter pattern** — we implement a `FileSystemAdapter` that talks to Tauri IPC for actual fs operations

### Architecture

```
┌─────────────────────┐
│  shadcn Tree UI     │  ← Our styling, per-node rendering
├─────────────────────┤
│  momoi React hooks  │  ← Tree state, selection, file ops
├─────────────────────┤
│  momoi Core         │  ← Tree engine, watching, coalescing
├─────────────────────┤
│  Tauri FS Adapter   │  ← Our code: ~80 lines, bridges to Rust
├─────────────────────┤
│  Rust (std::fs)     │  ← Tauri commands: list, read, write, rename, delete
└─────────────────────┘
```

### FileSystemAdapter interface

```typescript
interface FileSystemAdapter {
  readDir(dirPath: string): Promise<FileEntry[]>;
  rename(oldPath: string, newPath: string): Promise<void>;
  delete(paths: string[]): Promise<void>;
  createFile(parentPath: string, name: string): Promise<void>;
  createDir(parentPath: string, name: string): Promise<void>;
  move(srcPath: string, destDir: string): Promise<void>;
  watch?(dirPath: string, callback: (events: WatchEvent[]) => void): () => void;
}
```

The `watch` function uses Tauri's `fs.watch` or the `notify` crate behind a Tauri event emission.

## Consequences

### Positive

- All the hard stuff (file watching, coalescing, tree state, virtualization) is handled by momoi-explorer — mature, tested, VS Code-compatible behavior
- Adapter pattern means zero coupling to momoi's internals — swap later if needed
- Headless = full styling control with shadcn/ui
- File operations are consistent (drag-drop, context menu, keyboard all go through the same adapter)

### Negative

- momoi-explorer is relatively new — risk of API changes or bugs
- One more dependency in the tree (core + react + react-dom)
- File watching adapter needs careful Tauri IPC design to avoid perf issues on large directories
