# Env

- Parent node: root/Squire
- Node path: root/Squire/adapter-core
- Objective: Define the ContextManagerAdapter trait and its insertion seam in send_message orchestration, then extract existing behavior into LegacyContextAdapter with zero semantic change.
- Scope now: adapter trait shape, exact insertion point in send_message, LegacyContextAdapter extraction, parity tests.
- Non-goal now: Squire-specific adapter logic, session-mode persistence, storage, UI (see sibling nodes).
- Depends on: none — architecture is locked (see ../planning/analysis.md and ../planning/implementation-readiness.md).
- Blocks: session-mode, squire-adapter (both require the adapter trait/seam to exist first).
