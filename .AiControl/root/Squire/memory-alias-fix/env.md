# Env

- Parent node: root/Squire
- Node path: root/Squire/memory-alias-fix
- Objective: close the `"memory"`-alias/`system_referential` gap `user-input-chunking` flagged
  — the `resource_type="memory"` convenience alias in `explore()` expands to `concept`/
  `referential` token types only, predating `system_referential` (introduced later by
  `user-input-chunking`), so a model using the "memory" shortcut cannot discover the AI's own
  chunked user-input tokens even though `resource_type="system_referential"` and
  `resource_type="all"` both already find them correctly.
- Scope: the `type_matches` alias-expansion closure in `explore_memory`, duplicated in both
  `InMemorySquireStore` (`src-tauri/src/agent/squire.rs`) and `LanceDbSquireStore`
  (`src-tauri/src/storage/squire_lancedb.rs`) — add `system_referential` to the `"memory"`
  branch in both. Unit tests confirming the alias now surfaces `system_referential` tokens in
  both backends.
- Non-goal: the nested-`§!`-citation residual `hit-count-fidelity` flagged (explicitly left
  open per direct user instruction — "ignoring nesting feels more right"); any other alias/
  resource_type semantics beyond this one omission; any frontend/UI work (pure backend filter
  logic, no user-facing surface).
- Depends on: `user-input-chunking` (introduced `system_referential` as a token type, and
  originally flagged this gap in its own decisions.md as a newly-observed, unclaimed
  follow-up).
- Status: completed, 2026-07-03.

## Durable facts (read this session)

- The alias-expansion logic is a one-line boolean clause duplicated verbatim in two places:
  `InMemorySquireStore::explore_memory` (squire.rs) and `LanceDbSquireStore::explore_memory`
  (squire_lancedb.rs). Both needed the identical fix — this epic's established convention is
  parity between the two `SquireStore` implementations for every method.
- Before the fix: `resource_type == "memory" && (t == "concept" || t == "referential")`.
- After the fix: `resource_type == "memory" && (t == "concept" || t == "referential" || t ==
  "system_referential")`.
- `resource_type == "system_referential"` (exact match) and `resource_type == "all"` both
  already worked correctly before this fix — this was purely a gap in the "memory" convenience
  alias's own expansion list, not a broader discoverability defect.
- This was a direct user-requested fix (small enough not to need the full node-creation
  ceremony's read-heavy investigation phase — the gap was already fully diagnosed and located
  in a prior conversation turn before this node was created).
