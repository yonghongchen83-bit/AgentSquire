# Context Manager Adapter Analysis

## Goal

Replace single hard-wired history replay context construction with a pluggable, per-session context strategy.

## Why adapter is the right abstraction

- Legacy and Squire differ mainly in context construction and memory ingestion lifecycle.
- Both still use the same outer runtime concerns:
  - provider selection
  - streaming transport
  - tool approval policy
  - event emission and persistence hooks
- Adapter boundary isolates context logic while preserving stable orchestration infrastructure.

## Recommended boundaries

### Keep in orchestration core

- Provider/model selection.
- Stream handling and UI events.
- Tool watchdog and approval signaling.
- Final assistant persistence transaction hook.

### Move into adapter

- Turn-open input preparation.
- Context retrieval policy.
- In-turn special protocol behavior (Squire tool gateway rules).
- Turn-close ingestion/preserve bookkeeping.

## Risks to avoid

- Partial hybrid mode where Squire semantics are mixed with direct unrestricted tool calls.
- Mode switching mid-session without explicit migration rules.
- Implementing storage before finalizing protocol surface.

## Suggested implementation order

1. Refactor-only extraction (legacy adapter) with parity tests.
2. Session mode plumbing.
3. Squire protocol validator first.
4. Squire storage and graph traversal.
5. UI controls.
