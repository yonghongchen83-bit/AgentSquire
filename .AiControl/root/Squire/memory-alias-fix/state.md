# State

Completed 2026-07-03.

Closed the `"memory"`-alias/`system_referential` gap `user-input-chunking` flagged: the
`resource_type="memory"` convenience alias in `explore()`'s `type_matches` closure now expands
to `concept`/`referential`/`system_referential`, in both `InMemorySquireStore` (`squire.rs`)
and `LanceDbSquireStore` (`squire_lancedb.rs`). Previously it only expanded to
`concept`/`referential`, silently missing the AI's own chunked user-input tokens (created by
`user-input-chunking`) when the model used the "memory" shortcut rather than
`resource_type="system_referential"` or `"all"` explicitly (both of which already worked).

Two new unit tests, one per backend, confirm `explore(resource_type="memory", ...)` now
surfaces a `system_referential` token created via `ingest_user_input_chunks`.

`cargo build`: clean, zero warnings. `cargo test --lib`: 223/223 passing (221 baseline + 2
new).

Per direct user instruction, the other remaining backlog item — the nested-`§!`-citation
residual `hit-count-fidelity` flagged — was explicitly NOT addressed here ("ignoring nesting
feels more right"). See `decisions.md` for the full reasoning on both the fix and the
deliberate non-fix.

With this node complete, the Squire epic's residual backlog is now down to exactly one item:
the nested-`§!`-citation residual, which the user has indicated should remain as-is rather than
be closed. This effectively clears the way for the epic to be considered closed at the
`root/Squire` level, pending only that final human sign-off.
