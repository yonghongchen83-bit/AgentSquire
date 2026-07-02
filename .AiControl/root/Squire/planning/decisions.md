# Decisions

## Q1 - Canonical explore contract

- Decision: Option 3 (hybrid policy).
- Canonical runtime contract for v1 will be the system prompt superset:
  - resource_type supports tool_skill, concept, referential in addition to base types.
  - max_results behavior is supported.
- Governance note: spec document is now lagging and must be updated to match implementation contract.
- Compatibility rule:
  - Existing base types remain supported.
  - Superset types are additive and non-breaking.

## Follow-up action

- Update context_squire_spec_v2.md in a dedicated doc-sync pass before implementation freeze.

## Q2 - AskUser behavior in v1

- Decision: support both AskUser paths in v1.
- Usage policy:
  - TOOL_AskUser via invoke() is preferred for lightweight/simple feedback.
  - ask_user response-field loop is preferred for complex clarifications and multi-step disambiguation.
- Guardrail:
  - ask_user and content remain mutually exclusive per response.
  - Workflow instructions can bias path choice, but runtime supports both.

## Q3 - Session mode mutability

- Decision: Option 1.
- Rule: context mode is immutable after the first user turn in a session.
- Rationale:
  - avoids ambiguous mid-session semantics,
  - removes migration complexity in v1,
  - keeps adapter behavior deterministic for testing and debugging.
- Operational behavior:
  - mode may be selected at session creation,
  - mode change is blocked once at least one user message exists,
  - if a different mode is needed, create a new session.

## Q4 - Storage backend for v1

- Decision: Option 2.
- Rule: implement LanceDB from day one for Squire storage.
- Scope implication:
  - structured partition and raw partition are first-class in v1,
  - triplet store and vector search path should ship with production parity,
  - no SQLite-only stopgap for Squire memory internals.
- Delivery note:
  - keep storage interfaces abstracted so test doubles and future backend flexibility remain possible.

## Q5 - Tool exposure in Squire mode

- Decision: Option 1 (strict Squire boundary).
- Rule:
  - For sessions created in Squire mode, the model is exposed only to Squire built-ins and gateway behavior.
  - No direct exposure of external tools to the model.
  - All tool capability access must be mediated by Squire as the sole gateway.
- Enforcement implication:
  - Runtime must build distinct tool surfaces per session mode.
  - Squire-mode orchestration rejects or omits non-Squire direct tool registrations.

## Q6 - Rejection and retry semantics

- Decision: Option 3 (user-transparent final failure path).
- Runtime behavior:
  - Keep internal validation retries up to configured max_retries.
  - If retries are exhausted, surface a compliance-failure error to the user with a clear reason.
  - Also display the final AI response payload/content that failed compliance checks.
- UX intent:
  - user can inspect the failed response and adjust next prompt/direction to avoid repeated failure.
- Persistence/diagnostics:
  - store structured failure metadata (rule violated, validator reason, retry count, timestamp) for debugging.

## Q7 - Preserve lifecycle across restart

- Decision: Option 1 (strict next-turn only).
- Rule:
  - preserve list applies only to the immediate next turn.
  - restart clears pending preserve carryover state.
- Design implication:
  - preserved_tokens bootstrap input is an ephemeral handoff, not long-lived continuity state.
