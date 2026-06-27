---
id: task-002
title: Open Project Sets Project Workspace Path
priority: high
status: pending
---

## Description

Verify that clicking "Open Project" (either from the Welcome Screen or the Menu Bar) sets the project workspace path in the layout store, populates the file tree, hides the Welcome Screen, and updates the StatusBar.

## Test Cases

1. **Open Project button exists**: The Welcome Screen has an "Open Project" button in the center area
2. **Recent projects list**: After a project is opened, it appears in the "Recent Projects" section (stored in localStorage)
3. **Recent project click sets path**: Clicking a recent project entry calls `setProjectPath`, which is reflected in the StatusBar
4. **StatusBar shows project path**: The StatusBar displays the current project path when set (e.g., `D:\work\MyAgent`)
5. **File → Open Project in menu bar**: The MenuBar File menu has an "Open Project" item

## Selectors
- Welcome Screen: The `<div>` containing "MyAgent" heading and "Open a project to get started" text
- Open Project button: Button with text "Open Project" inside the Welcome Screen
- Recent projects: Buttons under "Recent Projects" heading in the Welcome Screen
- File Tree: The FileTree component div (`class="h-full overflow-auto py-1"`)
- StatusBar: The bottom bar with `class="flex h-6 items-center justify-between"`
- StatusBar project path: A `<span>` element containing the project path text in the StatusBar
- Menu Bar: The top bar with `class="flex h-7 items-stretch"` and "File" menu button

## Important Notes
- `projectPath` is stored in `useLayoutStore` (see `src/stores/ui-store.ts`)
- `FileTree` already uses `const root = projectPath || '.'`
- `StatusBar` now displays `projectPath` in the left section when non-empty
- The WelcomeScreen is shown by `MonacoWrapper` when there are no open editor tabs, NOT tied to `projectPath`
- The Tauri dialog API (`@tauri-apps/plugin-dialog`) opens a native OS dialog that cannot be automated via WebdriverIO
- Recent projects are persisted in `localStorage` key `myagent_recent_projects` — can be pre-seeded for testing
