# State — Phase 6: Settings & Polish

**Status:** ✅ Complete

## Deliverables

### p6-1 Settings UI
- Created `src/components/settings-dialog.tsx` — shadcn Dialog with 4 tabs:
  - **General**: theme cards (light/dark/system), font size, word wrap toggle, tab size select
  - **LLM**: dynamic provider list with add/remove, inline editing for name, model, API key, endpoint
  - **Search**: textarea for exclude patterns (one per line)
  - **Terminal**: shell path, terminal font size
- Save/Cancel buttons with TanStack Query mutation

### p6-2 Theme Switching
- Added `@custom-variant dark` in `index.css` with full dark palette
- Theme persisted in config, applied via `applyThemeClass()` which toggles `.dark` class
- System preference detection for `system` mode
- Monaco editor theme switches to `vs-dark` when dark mode active

### p6-3 Font/Editor Settings
- `MonacoWrapper` reads `fontSize`, `tabSize`, `wordWrap` from settings store
- Editor options update reactively via `useEffect`
- Applied both on mount and on config change

### p6-4 Auto-Update
- `check_update` IPC command in `commands/mod.rs`
- `tauri-plugin-updater` already in Cargo.toml and wired in `lib.rs`
- `checkUpdate()` wrapper in `src/lib/ipc.ts`

### p6-5 Error Handling
- Created `src/components/error-boundary.tsx` — class-based React error boundary
- Wraps the entire app in `main.tsx`
- Displays error message with "Try again" button
- Supports custom `fallback` prop

### p6-6 Keyboard Shortcuts
- Created `src/components/keyboard-shortcuts.tsx`
  - `Ctrl+Shift+P` — open settings
  - `` Ctrl+` `` — toggle terminal panel
  - `Ctrl+Shift+F` — focus search panel
  - `Ctrl+B` — toggle sidebar

### p6-7 Loading/Splash Screen
- Created `src/components/splash-screen.tsx` — branded splash with app icon, loading bar
- Fades out after ~1.1s, then renders the main app
- Controlled via `useSettingsStore.showSplash`

### Tests
- `settings-store.test.ts` — 13 tests covering all store actions
- `error-boundary.test.tsx` — 4 tests (render, catch, custom fallback, reset)
- `splash-screen.test.tsx` — 3 tests (render, animation timing, loading bar)
- `keyboard-shortcuts.test.tsx` — 4 tests (all keyboard combos)
- `settings-dialog.test.tsx` — 5 tests (render, buttons, LLM tab, theme cards, closed state)

### Rust Backend Changes
- Refactored `config.rs` to flat camelCase struct matching frontend `AppConfig`
- Added `load_config` and `check_update` IPC commands
- Updated `registry.rs` to use new flat config
- All 49 Rust tests pass

### New/Modified Files
- `src/components/ui/tabs.tsx`, `select.tsx`, `switch.tsx`, `label.tsx`
- `src/components/settings-dialog.tsx`, `error-boundary.tsx`, `splash-screen.tsx`, `keyboard-shortcuts.tsx`
- `src/stores/settings-store.ts`
- Updated: `App.tsx`, `main.tsx`, `sidebar.tsx`, `monaco-wrapper.tsx`, `xterm-terminal.tsx`, `index.css`, `types/ipc.ts`, `lib/ipc.ts`
- Updated Rust: `state/config.rs`, `llm/registry.rs`, `commands/mod.rs`, `lib.rs`
