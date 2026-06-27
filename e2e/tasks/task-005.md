---
id: task-005
title: Bottom Panel (Terminal / Output / Errors) Visibility and Toggle
priority: medium
status: pending
---

## Description

The bottom panel contains three tabs: Terminal, Output, and Errors. It is hidden by default (`bottomPanelVisible: false`). Verify it can be toggled via the StatusBar button, that all three tabs are present and switchable, and that the panel can be closed.

## Test Cases

1. **Hidden by default**: On fresh load, the bottom panel is not visible (no Terminal/Output/Errors elements rendered)
2. **Toggle on via StatusBar**: Clicking "Show Terminal" in the StatusBar makes the bottom panel appear with the Terminal tab active
3. **Tab switching**: Clicking on "Output" and "Errors" tabs switches the panel content accordingly
4. **Close via X button**: Clicking the X button in the bottom panel header hides it again
5. **Toggle off via StatusBar**: Clicking "Hide Terminal" in the StatusBar hides the bottom panel

## Selectors
- StatusBar terminal button: Contains text "Show Terminal" or "Hide Terminal" with `Terminal` icon
- Bottom panel: The `<div class="h-full flex flex-col bg-[#E8EDF2] border-t border-border">` inside BottomPanel
- Tab bar: Tab buttons with text "Terminal", "Output", "Errors" (each has a lucide icon)
- Active tab indicator: Tab button with `bg-background text-foreground` classes
- Close button: The X button in the bottom panel header (`X` icon from lucide)
- Terminal content: The `XtermTerminal` component (terminal instance)
- Output content: The `OutputPanel` component
- Errors content: The `ErrorsPanel` component

## Important Notes
- `bottomPanelVisible` defaults to `false` in `useLayoutStore`
- Toggle is via `status-bar.tsx:24`: button with `Terminal` icon and dynamic text "Show Terminal" / "Hide Terminal"
- Close is via `bottom-panel.tsx:173`: X button with `toggleBottomPanel` onClick
- Bottom panel has 3 tabs: terminal (XtermTerminal), output (OutputPanel), errors (ErrorsPanel)
- Output panel has a source dropdown (stdout/debug/notifications)
- Errors panel shows error entries with severity colors
- The bottom panel renders in the layout via App.tsx (likely as a child of the main content area)
