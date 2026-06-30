# Prompt

## Objective
Create and execute a safe, incremental codebase refactor program for MyAgent that improves module boundaries, reduces duplication, and increases maintainability without changing end-user behavior.

## Scope
- Backend Rust command and orchestration boundaries.
- Frontend settings/chat/store decomposition.
- IPC contract consistency and event naming consistency.
- Repository hygiene and coding-standards hardening.

## Non-Goals
- No feature additions.
- No UI redesign.
- No behavior changes unless required to fix contract mismatches.

## Success Criteria
- Refactor delivered in small approved batches.
- Build, lint, and tests stay green after each batch.
- File/module boundaries are clearer and easier to own.

## Inputs
- Existing architecture notes in ArchitecturePlanning.
- Systematic review findings captured in this node documents.

