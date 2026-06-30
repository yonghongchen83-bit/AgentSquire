# Findings Baseline

## Contract and Integration
- File watcher event mismatch between backend emit and frontend listen path.
- rename_item invoke payload key mismatch risk between frontend and backend command signature.
- Stale frontend IPC exports for output and errors with no matching backend command handlers.

## Module Boundaries
- src-tauri/src/commands/mod.rs mixes unrelated concerns and exceeds manageable scope.
- src/components/settings-dialog.tsx mixes static provider catalog, theme bootstrap, dialog shell, and tab content.
- src/stores/chat-store.ts mixes parsing, streaming listeners, persistence helpers, and view state orchestration.

## Duplication and Similar Logic
- normalize_level implemented in both OpenAI and Anthropic providers.
- Provider connectivity and model listing logic centralized in commands instead of provider or service adapters.

## Large File Candidates
- src-tauri/src/commands/mod.rs
- src/components/settings-dialog.tsx
- src-tauri/src/agent/mod.rs
- src-tauri/src/llm/openai.rs
- src/stores/chat-store.ts
- src/components/file-tree.tsx
- src/components/mcp-panel.tsx

## Standards and Practices
- Existing lint warnings indicate unused variables/imports and hook dependency issues.
- Fast refresh warnings indicate mixed exports in component files.
