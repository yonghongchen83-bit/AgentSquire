---
id: task-006
title: File Explorer Icons and Directory Expansion
priority: high
status: pending
---

## Description

The file explorer tree should visually distinguish folders from files (blue folder icons), render file-type-specific icons for common extensions, and correctly expand/collapse directories to reveal their children. Symlinks should display an indicator.

## Test Cases

1. **Folder vs file icons**: Folder items render `Folder`/`FolderOpen` icons (blue `#4A90D9`), file items render extension-specific icons (e.g., `FileCode` for `.ts`, `FileJson` for `.json`)
2. **Directory expansion**: Clicking a directory toggles its children into view — the `ChevronRight` changes to `ChevronDown` and child `TreeItem` elements appear in the DOM
3. **Directory collapse**: Clicking the expanded directory again removes children from the DOM and the chevron reverts to `ChevronRight`
4. **File extension icons**: A `.ts` or `.tsx` file renders a `FileCode` icon, a `.json` file renders a `FileJson` icon, a `.md` file renders a `FileText` icon
5. **Symlink indication**: If any symlinks exist in the project, they show a `Link` icon overlay and an italic "symlink" label

## Selectors
- File tree container: `div.h-full.overflow-auto.py-1`
- Tree item rows: `div.flex.items-center.gap-1.px-2.py-0\.5.text-sm`
- Folder icon: `svg.lucide-folder` or `svg.lucide-folder-open` (blue colored)
- File icons: `svg.lucide-file-code`, `svg.lucide-file-json`, `svg.lucide-file-text`, etc.
- Chevron icons: `svg.lucide-chevron-right` / `svg.lucide-chevron-down`
- Symlink label: `span` containing text "symlink"

## Important Notes
- Icons come from `lucide-react` and render as inline SVGs with BEM-like class names (e.g., `lucide-folder`, `lucide-file-code`)
- Folders use `text-[#4A90D9]` style (blue); files use `text-[#607d8b]` (gray-blue)
- The file icon mapping is defined in `pickFileIcon()` in `file-tree.tsx`
- Children are rendered as sibling DOM elements after the parent `ContextMenu` — use `$$` to find next-sibling tree items after toggling
- Use `browser.execute()` to set `projectPath` in the layout store since Tauri dialog IPC cannot be automated
