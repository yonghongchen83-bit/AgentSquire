# CodeRefactor Plan

## Overview
This plan executes a low-risk refactor in approved batches. The sequence is designed to reduce breakage risk by stabilizing contracts first, then decomposing large files and modules.

## Batch 1: Contract Safety
### Goals
- Align frontend and backend command argument names for rename operation.
- Align file watcher event naming across frontend and backend.
- Remove or implement stale IPC API surface to avoid dead contracts.

### Deliverables
- Consistent rename command payload and backend handler parameter mapping.
- Consistent watcher event emitted and listened name.
- IPC surface cleaned for unused get_output and get_errors calls (or implemented if required).

### Validation
- pnpm lint
- pnpm build
- cargo build --manifest-path src-tauri/Cargo.toml

## Batch 2: Backend Command Decomposition
### Goals
- Break src-tauri/src/commands/mod.rs into coherent command modules by domain.

### Target Modules
- commands/config.rs
- commands/conversations.rs
- commands/streaming.rs
- commands/files.rs
- commands/search.rs
- commands/git.rs
- commands/terminal.rs
- commands/providers.rs
- commands/mcp.rs
- commands/setup.rs

### Validation
- cargo build --manifest-path src-tauri/Cargo.toml
- cargo test --manifest-path src-tauri/Cargo.toml

## Batch 3: Frontend Settings Decomposition
### Goals
- Split settings-dialog into composable parts by concern.

### Target Modules
- components/settings/provider-catalog.ts
- components/settings/theme-utils.ts
- components/settings/GeneralTab.tsx
- components/settings/LlmTab.tsx
- components/settings/SearchTab.tsx
- components/settings/TerminalTab.tsx
- components/settings/ProviderCard.tsx

### Validation
- pnpm lint
- pnpm test
- pnpm build

## Batch 4: Chat Store Decomposition
### Goals
- Isolate stream listeners, block parsing, and storage helpers from chat-store.

### Target Modules
- stores/chat-store/core.ts
- stores/chat-store/stream-listeners.ts
- stores/chat-store/block-parser.ts
- stores/chat-store/preferences.ts

### Validation
- pnpm lint
- pnpm test
- pnpm build

## Batch 5: Provider Logic Deduplication
### Goals
- Consolidate shared thinking-level normalization and common provider capability helpers.
- Keep provider-specific transport details in provider files.

### Validation
- cargo build --manifest-path src-tauri/Cargo.toml
- cargo test --manifest-path src-tauri/Cargo.toml

## Batch 6: Repo Hygiene and Standards
### Goals
- Move local logs or environment artifacts into proper ignored locations.
- Resolve existing lint warnings in touched areas.

### Validation
- pnpm lint
- pnpm build
- cargo build --manifest-path src-tauri/Cargo.toml

## Rollout Notes
- Complete one batch at a time.
- Do not start next batch until current batch validation is green.
- Keep commits focused and reversible.
