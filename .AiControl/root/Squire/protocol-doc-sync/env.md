# Env

- Parent node: root/Squire
- Node path: root/Squire/protocol-doc-sync
- Objective: Sync context_squire_spec_v2.md and context_squire_system_prompt_v2.md with the locked/actual runtime contract (Q1's system-prompt superset, plus reconciling Q6/Q7's real implementations from rejection-ux, plus any other drift found by a systematic diff).
- Scope now: doc updates only — no runtime code changes. Both `.md` protocol docs live under `.AiControl/root/Squire/` (planning workspace, not a build input — never included by any Rust source file).
- Non-goal now: changing runtime behavior; this node documents what other nodes already implement/lock, and flags (without fixing) genuine implementation gaps found along the way.
- Depends on: none blocking — Q1 is already resolved; ran after rejection-ux landed so Q6/Q7's real behavior could be documented accurately.
- Status: implemented and landed 2026-07-02.

## Durable facts (added this session)

- `context_squire_spec_v2.md` and `context_squire_system_prompt_v2.md` now carry inline `Implementation status (runtime v1)` annotations everywhere their described behavior was checked against the actual runtime — both intentional adaptations (documented as the real contract) and genuine unimplemented gaps (documented as open gaps, not silently normalized). See `decisions.md` for the full inventory and the reasoning behind each judgment call.
- Five genuine, previously-unflagged implementation gaps were found by this node's systematic diff and are not yet claimed by any node's todo.json: (1) `accumulated_hits`/`effective_priority` scoring model does not exist anywhere in the runtime; (2) `num_hops` graph traversal is accepted but not implemented by either `SquireStore` impl (confirmed for both `InMemorySquireStore` and the production `LanceDbSquireStore`); (3) user-input auto-chunking into `USR_TN_NNN` tokens does not happen — `build_turn_input` uses the raw latest user message directly; (4) there is no raw-partition audit-log table anywhere in `squire_lancedb.rs`; (5) `explore(resource_type="tool_skill")` returns a flat type-tagged array in runtime, not the nested `{"tool": [...], "skill": [...]}` dict the system-prompt doc originally described (this one was resolved as a doc update, not flagged as a bug — see `decisions.md`). Items (1)-(4) remain open, unassigned gaps for a future node to pick up if desired.
