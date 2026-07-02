# Prompt

Use this node to implement the two spec-flagged retrieval-quality gaps judged most load-bearing
by `protocol-doc-sync` — the ones the spec explicitly calls out as core differentiators from
"a RAG wrapper" (`context_squire_spec_v2.md` §7.3):

1. **Graph traversal (`num_hops`)** — `explore()`'s `num_hops` parameter is accepted end-to-end
   by the tool schema and by both `SquireStore` implementations (`InMemorySquireStore` in
   `src-tauri/src/agent/squire.rs` and `LanceDbSquireStore` in
   `src-tauri/src/storage/squire_lancedb.rs`), but neither actually traverses the
   `squire_relationships` triplet store. Implement real traversal: given the set of directly
   vector/type-matched tokens, walk the relationship graph outward up to `num_hops` hops and
   include connected tokens in the result set, with hop-distance/provenance metadata the caller
   can use to understand why a token was included.

2. **`accumulated_hits`/`effective_priority` scoring** — spec §3.2/§3.3 describes a hit-count/decay
   model for ranking and prioritizing tokens. This does not exist anywhere in the runtime today.
   Add a real field, a real computation, and integrate it into `explore()`'s result ordering.

Deliverables:
- Both features implemented in both `InMemorySquireStore` and `LanceDbSquireStore`, plus whatever
  adapter/tool-schema code passes data through.
- Real unit tests for both, in the same style as the existing test suites in `squire.rs` and
  `squire_lancedb.rs`.
- Document the scoring formula decision (decay/recency) in `decisions.md` since the spec doesn't
  fully pin down implementation-level details.

Out of scope (do NOT fix here — separately tracked, deliberately deferred):
- `squire-adapter/todo.json` sa-4 (raw stream sigil leak before finalize_turn expands sigils)
- `squire-adapter/todo.json` sa-5 (ask_user response-field UI round-trip loop)
- `squire-storage/todo.json` ss-9 (real tool-token ingestion) — noted as possibly related to
  scoring/traversal but not required; do not feel obligated to fix it here if genuinely separate.
- User-input auto-chunking (`USR_TN_NNN` tokens)
- Raw-partition audit-log storage

Reference: `../context_squire_spec_v2.md` §3.2, §3.3, §6.1, §7.3; `../planning/decisions.md` Q1;
`../squire-adapter/decisions.md`; `../squire-storage/decisions.md`.
