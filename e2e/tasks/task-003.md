---
id: task-003
title: File Explorer Populated After Project Opened
priority: high
status: pending
---

## Description

After opening a project, the file tree (explorer panel in the left sidebar) should list the files and directories from the project root. Expanding a directory should show its children.

## Test Cases

1. **Root files listed**: After opening a project, the FileTree displays top-level entries (directories and files) from the project root
2. **Directory expandable**: Clicking a directory chevron expands it and loads its children
3. **Directory collapse**: Clicking the chevron again collapses the directory
4. **File opens in editor**: Clicking a file in the tree opens it in the editor panel

## Selectors
- File Tree root: The `<div class="h-full overflow-auto py-1">` inside the FileTree component
- File tree items: TreeItem divs with class `flex items-center gap-1 px-2 py-0.5 text-sm cursor-pointer`
- Directory expand icon: `ChevronRight` / `ChevronDown` icons (h-3 w-3)
- File icon: `File` icon (h-4 w-4 shrink-0)
- Folder icon: `Folder` or `FolderOpen` icon
- Context menu: `ContextMenuContent` (right-click)

## Important Notes
- `FileTree` already uses `projectPath` from layout store via `useLayoutStore((s) => s.projectPath)`
- `FileTree` calls `listDirectory(root)` then `gitStatus()` on mount and refresh
- Children are loaded lazily via `loadChildren(path)` on expand toggle
- The tree uses CSS class selectors; no `data-testid` attributes exist
- For test 4, verify the file opens by checking the editor panel contains the file content or the tab appears
