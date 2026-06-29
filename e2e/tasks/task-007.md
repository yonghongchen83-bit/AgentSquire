---
id: task-007
title: Settings LLM Model Configuration Flow
priority: high
status: verified (v3)
---

## Description

Verify that the settings dialog's LLM model configuration flow works correctly:
1. Opening settings from welcome screen navigates to LLM tab (not General)
2. Provider and model dropdowns inside the dialog don't accidentally close the settings dialog
3. The "Test Connection" flow is accessible and functional

## Bug Context

**Bug 1**: When doing LLM model config without choosing a provider, clicking the models dropdown and closing it quits the settings page completely. Root cause: Radix Select portal events propagate to the Dialog's onOpenChange, causing the dialog to close when interacting with nested Select elements. Fix: prevent all pointer-down-outside on DialogContent, guard Escape key when a Select is open, and use setTimeout guard in onOpenChange to let Select DOM state settle before checking if dialog should close.

**Bug 2**: When clicking "Model Configuration" or "Test Connection" from the welcome screen, settings opens but stays on the General tab instead of the LLM tab.

**Bug 3**: The test connection flow was broken because the UI didn't ensure a model was selected, and the Test button was disabled when no API key was present (preventing testing local providers like Ollama).

**Bug 4 (design)**: The `name` field was set to the first model ID (e.g. "gpt-4o") instead of the category label. This made adding additional models under the same name nonsensical. Fixed: `name` defaults to the provider category label (e.g. "ChatGPT"), and the multi-model tag list+add/remove functionality is properly retained.

## Test Cases

1. **Welcome screen buttons navigate to LLM tab**: Clicking "Model Configuration" or "Test Connection" opens settings with the LLM tab active
2. **Provider dropdown does not close dialog**: Opening and closing the provider dropdown does not close the settings dialog
3. **Models dropdown does not close dialog**: Opening and closing the models dropdown (even without a provider selected) does not close the settings dialog
4. **Test connection button is accessible**: When a provider with API key is configured, the Test button is clickable

## Selectors
- Welcome Screen "Model Configuration": Button with text "Model Configuration"
- Welcome Screen "Test Connection": Button with text "Test Connection"
- Settings Dialog: The Dialog component (`[role="dialog"]`)
- LLM Tab: TabsTrigger with value "llm" inside settings dialog
- Provider Select: Select component inside the LLM tab
- Models Select: Select component showing "Add model..." placeholder
- Test Connection Button: Button with text "Test" in the API Key row

## Important Notes
- The Tauri backend (`get_config` / `save_config`) may not be available in browser-only tests
- Tests should work against the Vite dev server at `http://localhost:5173/`
- Zustand stores can be manipulated via `browser.execute()` if needed
