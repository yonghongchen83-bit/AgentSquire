# State

## Status

Planning node created. Implementation intentionally paused pending clarification of protocol and lifecycle details.

Q1 resolved: canonical explore contract uses hybrid option (system prompt superset is runtime source of truth for v1; spec requires doc sync).

Q2 resolved: support both AskUser paths in v1; tool path for lightweight/simple feedback, response-field loop for complex clarification.

Q3 resolved: session context mode is immutable after first user turn.

Q4 resolved: LanceDB is required from day one for Squire v1 storage.

Q5 resolved: Squire-mode sessions use strict gateway-only tool exposure; no direct external tool exposure to the model.

Q6 resolved: on final compliance failure, expose both failure reason and final failed AI response to user after retry exhaustion.

Q7 resolved: preserve list is strictly next-turn only and is cleared on restart.

## Analysis Summary

### Current code seam for adapter insertion

- Backend context assembly currently happens in send_message flow where all session messages are replayed into LLM request construction.
- This seam allows introducing a ContextManagerAdapter without rewriting provider transport, stream event emission, or tool approval framework.

### Proposed adapter model

- ContextManagerAdapter trait:
  - build_turn_input(...)
  - handle_tool_loop_step(...)
  - finalize_turn(...)
- Implementations:
  - LegacyContextAdapter (existing behavior, no semantic changes)
  - SquireContextAdapter (Squire open/loop/close lifecycle)

### Session-level swap design

- Persist context_mode per session.
- Add global default_context_mode only for new session creation.
- Route send_message by session context_mode.

### Data model impact

- DB migration: sessions.context_mode (default legacy).
- Extend Session/NewSession/IPC types to include context_mode.
- Keep existing messages table unchanged for initial rollout.

### Incremental delivery plan

1. Extract legacy behavior behind adapter first (no behavior change).
2. Add session mode persistence and API.
3. Add Squire adapter skeleton + protocol validation gates.
4. Add Squire storage and retrieval internals.
5. Add mode selector in UI and mode badge.
6. Add tests for both modes.

## Open Questions (to resolve one by one)

None. All initial architecture clarification questions are resolved.

## Node Closed — 2026-07-02

Planning objective (architecture decisions for the ContextManagerAdapter) is complete; see `implementation-readiness.md`. Work is split into sibling implementation nodes under `root/Squire`, each scoped to a distinct technical context so implementation agents don't have to load the whole planning history:

| Node | Scope | Depends on |
|------|-------|------------|
| `../adapter-core` | Adapter trait + insertion seam + LegacyContextAdapter extraction | none |
| `../session-mode` | context_mode persistence + immutability guard + routing | adapter-core |
| `../squire-adapter` | SquireContextAdapter + strict tool surface + validation gates | adapter-core, session-mode |
| `../squire-storage` | LanceDB storage stack (structured/raw partitions, triplets) | squire-adapter |
| `../rejection-ux` | Compliance-failure surfacing + diagnostics + preserve-list lifecycle | squire-adapter |
| `../protocol-doc-sync` | Sync context_squire_spec_v2.md to locked contract (Q1 follow-up) | none, non-blocking |

plan-2 was cancelled here and carried forward as `adapter-core` task `ac-1`. This node's remaining todos are all DONE or CANCELLED.
