---
id: task-001
title: Side Panel Resize via Divider Drag
priority: high
status: verified
---

## Description

Verify the left side panel (containing Explorer/Search/Git) can be resized by dragging the divider handle between it and the editor area. Also verify the panel initializes at 20% of the app's content width.

## Verified Tests

1. **initial width**: Left panel starts at ~20% of content width (window minus 48px sidebar)
2. **drag right**: Dragging the left-handle right increases the panel width
3. **drag left**: Dragging the left-handle left decreases the panel width
4. **editor adjustment**: Editor panel width + left panel width ≈ content width (handle accounted for)

## Selectors (react-resizable-panels v4)
- Left panel: `#left-panel` or `[data-testid="left-panel"]`
- Divider handle: `#left-handle` or `[data-testid="left-handle"]`
- Editor panel: `#editor-panel` or `[data-testid="editor-panel"]`
- Sidebar: `.w-12` (fixed 48px icon bar)

## Important Notes
- `react-resizable-panels` v4 treats **number** props as **pixels**, not percentages
- Always use string percentages: `defaultSize="20%"`, `minSize="15%"`, `maxSize="50%"`
- The `onLayout` callback is not supported in v4; use `onLayoutChanged` instead
- Tests run against the Vite dev server at `http://localhost:5173/`
- Test setup must call `browser.url('http://localhost:5173/')` for a fresh page load
