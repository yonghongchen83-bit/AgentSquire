# State — Phase 0.5: UI Layout Design

**Status:** Complete ✅

Layout design document delivered: `ArchitecturePlanning/layout-design.md`

## Decisions Made
- **Layout:** VS Code single-window, resizable panels
- **Title bar:** Custom HTML with window controls (— □ ✕)
- **Menu bar:** File, Edit, View, Help — custom HTML, Alt-key activated
- **Sidebar (activity bar):** Far left, 48px fixed, icon-based view switching
- **Left side panel:** File explorer (momoi) + Search (ripgrep) + Git — tab-switched via sidebar icons
- **Center area:** Monaco editor with scrollable tab bar (max 15 tabs, close via X, pinnable)
- **Welcome screen:** Logo, Open Project, Recent Projects, Model Configuration link
- **Right side panel:** Dedicated chat panel (message list + composer)
- **Bottom panel:** Terminal (xterm.js, multiple tabs) + Output (source-selectable dropdown) + Errors
- **State management:** Zustand (transient UI) + TanStack Query (IPC data) + Rust (authoritative)
- **Resize persistence:** Panel ratios saved to TOML config, debounced 500ms
- **Minimum window size:** 600px
- **Theme:** Light blue (`#F0F4F8` bg, `#4A90D9` accent)
- **Status bar:** LLM connection, notification center, cursor pos, font zoom (− 100% +)
- **File icons:** Icon library (file-icons/seti)
- **Panel collapse:** Hide only, re-expandable, no process kills
- **Context menus:** Standard (file tree + editor tabs)

## Next
Move to Phase 1 (Rust Backbone) or Phase 2 (App Shell implementation).
