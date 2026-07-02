# Prompt

Close the `"memory"`-alias/`system_referential` gap `user-input-chunking` flagged as a
newly-observed, unclaimed follow-up.

The `explore()` tool's `resource_type="memory"` argument is a convenience alias, not a real
token type — it expands to search `concept`-type and `referential`-type tokens. This alias
predates `system_referential` (a third token type introduced later by `user-input-chunking`
for the AI's own chunked-user-input tokens, `USR_T{turn}_{NNN}`). Nobody updated the alias
when that type was added, so a model calling `explore(resource_type="memory", ...)` silently
misses user-input chunks — even though `resource_type="system_referential"` (exact) and
`resource_type="all"` both already find them correctly. Direct user request: fix this gap,
explicitly do not also address the separate nested-`§!`-citation residual `hit-count-fidelity`
flagged ("ignoring nesting feels more right").

Deliverables:

- Locate and fix the duplicated `type_matches` alias-expansion closure in both
  `InMemorySquireStore::explore_memory` (`src-tauri/src/agent/squire.rs`) and
  `LanceDbSquireStore::explore_memory` (`src-tauri/src/storage/squire_lancedb.rs`): add
  `|| t == "system_referential"` to the `"memory"` branch in both.
- Add a unit test in each file confirming `explore(resource_type="memory", ...)` now surfaces
  a `system_referential` token created via `ingest_user_input_chunks`.
- Verify: `cargo build` + `cargo test --lib` clean, all tests passing (221 baseline + 2 new).
- Update `../state.md`'s "Child Nodes" list and `../handoff.md` to reflect this fix, current
  build/test status, and the remaining backlog (only the nested-citation residual, explicitly
  left open).

Out of scope (do NOT change here):
- The nested-`§!`-citation residual `hit-count-fidelity` flagged — explicitly deferred by
  direct user instruction.
- Any other `explore()` alias/resource_type semantics.
- Any frontend/UI work.
