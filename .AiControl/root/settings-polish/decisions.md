# Decisions — Phase 6

| # | Decision | Rationale |
|---|----------|-----------|
| 1 | **Flat config struct** — Config flattened to single flat camelCase struct instead of nested sub-configs | Matches frontend AppConfig interface; avoids double-serialization; serde `rename_all = "camelCase"` handles JS convention |
| 2 | **Tailwind class strategy for dark mode** — `.dark` class toggled on `<html>`, CSS variables swapped via `.dark { ... }` block | Simple, no JS runtime for styling; works with Tailwind v4 `@custom-variant dark` |
| 3 | **Zustand for settings store** — Settings state lives in Zustand for the dialog lifecycle, saved via IPC mutation | Consistent with existing pattern (Rust owns persistent state, frontend caches) |
| 4 | **Error boundary wraps entire app** — Class component catching render errors in `main.tsx` | Simplest reliable approach; class-based `getDerivedStateFromError` required by React |
| 5 | **Splash screen as simple effect-based component** — Timer-driven fade-out, not a loading indicator | Config/SQLite init is fast (~200ms); splash is a branding moment, not a real loading barrier |
| 6 | **Keyboard shortcuts in dedicated component** — Single `KeyboardShortcuts` component at App level with `keydown` listener, not in each panel | Centralized, easy to discover/edit; avoids scattering event listeners |
