# Decisions

## The fix

`explore()`'s `resource_type="memory"` is documented (system prompt, spec) as a convenience
alias standing in for "search my own created memory," not a literal token type. Its expansion
logic lived as a duplicated one-line boolean clause in `type_matches` in both
`InMemorySquireStore::explore_memory` (`squire.rs`) and `LanceDbSquireStore::explore_memory`
(`squire_lancedb.rs`):

```rust
|| (resource_type == "memory" && (t == "concept" || t == "referential"))
```

`system_referential` (the type `user-input-chunking` gave to `USR_T{turn}_{NNN}` chunk tokens)
was added to this codebase after this alias was first written, and nobody updated the alias's
expansion list to include it. Fixed identically in both files:

```rust
|| (resource_type == "memory"
    && (t == "concept" || t == "referential" || t == "system_referential"))
```

## Why this scope, and not more

This was diagnosed precisely in conversation before this node existed — the exact clause, the
exact two call sites, and the exact one-line fix were already known. No further investigation
was warranted; this node exists purely for the epic's own traceability/documentation
convention, not because the fix itself needed a discovery phase.

## Deliberately not addressed here (direct user instruction)

The user was separately asked about the other remaining backlog item — the nested-`§!`-citation
residual `hit-count-fidelity/decisions.md` flagged (a `full_desc` body citing another token via
`§!`, only surfaced when loaded via `token_to_detail`, not itself scanned for embedded
references) — and explicitly said to leave it alone: "ignoring nesting feels more right to me."
This is treated as a final, deliberate product decision, not a deferral pending a future
session. It remains documented in `hit-count-fidelity/decisions.md`/`state.md` as a known,
intentional simplification, not a bug.

## Verification

- `cargo build` (lib): clean, zero warnings, both before and after the fix.
- `cargo test --lib`: 223/223 passing (221 baseline from `token-detail-endpoint` + 2 new: one
  in `squire.rs::tests::explore_memory_alias_includes_system_referential_tokens` against
  `InMemorySquireStore`, one in
  `squire_lancedb.rs::tests::explore_memory_alias_includes_system_referential_tokens` against
  the real `LanceDbSquireStore`).
- No frontend/e2e verification needed — pure backend filter-predicate logic, no user-facing
  surface, consistent with every other `explore_memory` filtering change in this epic.
