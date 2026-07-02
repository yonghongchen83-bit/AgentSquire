# Implementation Readiness Snapshot

## Clarification status

All seven architecture clarification questions are resolved and documented.

## Locked decisions

1. Canonical explore contract: hybrid policy, system-prompt superset is runtime truth for v1; spec doc must be synced.
2. AskUser behavior: support both paths.
   - invoke TOOL_AskUser for lightweight/simple feedback.
   - ask_user response loop for complex clarifications.
3. Session mode mutability: immutable after first user turn.
4. Squire storage backend: LanceDB from day one.
5. Squire mode tool exposure: strict Squire-only gateway, no direct external tools.
6. Rejection semantics: retry up to max_retries; on exhaustion, show compliance failure reason and final failed AI response; keep diagnostics.
7. Preserve lifecycle: strictly next-turn only, cleared on restart.

## Build-sequencing implications

1. Extract LegacyContextAdapter first with behavior parity tests.
2. Add per-session context_mode persistence and immutable lifecycle guard.
3. Introduce SquireContextAdapter with strict tool surface separation.
4. Implement Squire storage stack with LanceDB partitions and triplet traversal.
5. Implement rejection visibility UX path and diagnostics persistence.
6. Add protocol doc-sync pass to align spec with locked runtime contract.

## Ready-to-start implementation scope

- The architecture boundary and policy decisions are now stable enough to begin coding with low redesign risk.
