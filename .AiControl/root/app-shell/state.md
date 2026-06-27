# State — Phase 2: App Shell

**Status:** Complete

## What was built

### Components
- **TitleBar** — Custom HTML title bar (~32px) with hamburger menu, project name, draggable region
- **Sidebar** — Activity bar (~48px fixed) with Explorer/Search/Git icon navigation, tooltips, Settings button
- **StatusBar** — LLM connection indicator, terminal toggle, notification bell, cursor position (Ln/Col), font zoom (-/+)
- **TabBar** — Editor tabs with close-on-hover, dirty dot indicator, active tab highlighting, horizontal scroll on overflow
- **MonacoWrapper** — @monaco-editor/react wired to IPC `read_file` on file open, language detection from extension, cursor position tracking
- **FileTree** — File tree from IPC `list_directory`, expand/collapse, folder icons, git status color dots (yellow=modified, green=added, red=deleted), context menus with New File/Folder/Rename/Delete/Reveal
- **LeftSidePanel** — Container switching between Explorer/Search/Git views (search/git are stubs)
- **BottomPanel** — Terminal/Output/Errors tabs with xterm.js terminal (FitAddon for resize), Output/Errors placeholders
- **WelcomeScreen** — Empty state shown when no editor tabs open

### Infrastructure
- **Zustand stores**: `ui-store.ts` (LayoutStore + StatusBarStore), `editor-store.ts` (TabStore)
- **IPC wrapper**: `src/lib/ipc.ts` — typed invoke() wrappers for all file/fs/git/config commands
- **Theme**: Light blue palette (shadcn CSS variables in index.css)
- **Tauri capabilities**: Expanded to include shell:default, fs:default, dialog:default
- **shadcn/ui**: Fixed location (src/components/ui/), fixed resizable.tsx exports for react-resizable-panels v4

### Packages installed
class-variance-authority, lucide-react, zustand, @tanstack/react-query, @tauri-apps/api, @tauri-apps/plugin-shell/fs/dialog, @monaco-editor/react, monaco-editor, @xterm/xterm, @xterm/addon-fit, @radix-ui/react-context-menu

## Verified
- `npx tsc --noEmit` — zero errors
- `npx vite build` — builds clean (chunk size warning for Monaco only)
