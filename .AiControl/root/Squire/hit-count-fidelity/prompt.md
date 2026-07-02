# Prompt

Close `retrieval-fidelity/todo.json` rf-13: fuller hit-count-event fidelity to spec §3.3's
four-event table.

> **FOLLOW-UP (not in this node's scope, not claimed):** spec 3.3's "Token appears in
> explore() results that AI acts on" hit event is only partially covered (via
> `token_to_detail` calls, not via a full context-composition scan for embedded
> section-reference sigils inside loaded `full_desc` bodies). Documented as a deliberate
> narrow reinterpretation in decisions.md, not a silent gap. Left open for a future node if
> stricter fidelity to the literal spec wording is wanted.

Spec §3.3's exact four hit-count-increment events:

| Event | Delta |
|---|---|
| Token appears in explore() results that AI acts on | +1 |
| Token in preserve list loaded at turn open | +1 |
| §! reference found in a chunk loaded into context | +1 |
| Token listed in new_tokens at turn close | +1 |

`retrieval-fidelity` wired events 2 and 4 directly (`preserved_tokens`, `upsert_token`). Events
1 and 3 were both approximated by a single proxy: crediting a hit whenever
`SquireTokenToDetailTool::execute` is called on a store-backed token. `retrieval-fidelity/
decisions.md`'s own text explains why: "acting on" an explore() result can't be known at
explore-call time (it happens later in the turn or not at all), and the literal "chunk loaded
into context" event would require a full context-composition audit scan that doesn't exist as
a concept in this codebase.

Deliverables:

- Read `../retrieval-fidelity/decisions.md` and `todo.json` in full (rf-13's exact wording,
  and the "Hit-count increment events wired: 2 of the spec's 4, with rationale for the other
  2" section) — this is the precise prior reasoning and constraint set this node must engage
  with, not re-derive from scratch.
- Read `../context_squire_spec_v2.md` §3.2/§3.3 (and the §5.1/§9.4-step-7/§10.1-step-7 cross
  references to the same events) as reconciled by `../protocol-doc-sync`, to confirm the exact
  wording of the two still-approximated events and rule out any misreading.
- Read the actual code in full: `src-tauri/src/agent/squire.rs` — `SquireStore::record_hit`,
  `SquireExploreTool`, `SquireTokenToDetailTool`, `SquireInvokeTool`,
  `SquireContextAdapter::finalize_turn`/`build_turn_input`/`expand_for_display`, and the sigil
  parsers (`extract_inline_refs`, `extract_spans`, `unmarked_residual`,
  `strip_span_markers`) — to find the natural, already-existing integration points rather than
  inventing new infrastructure.
- Verify baseline: from `src-tauri/`, `cargo build` + `cargo test --lib` (expect clean,
  206/206).
- Design and document, before implementing, exactly what "explore results acted on" and "§!
  reference in a loaded chunk" mean operationally in this codebase's existing call graph, and
  which lighter-weight (but still real, not silently-dropped) mechanism will wire each one.
  Keep scope proportionate — do not build a full audit trail of every token's exposure
  history if a narrower, still-faithful mechanism is defensible; document any remaining
  approximation explicitly rather than silently declaring full fidelity.
- Implement the chosen wiring in both `InMemorySquireStore` and `LanceDbSquireStore` (parity
  is this epic's established convention for every `SquireStore` method).
- Add unit tests for the newly-wired event(s) in both `squire.rs` and `squire_lancedb.rs`,
  matching `retrieval-fidelity`'s existing test style.
- Verify via unit tests plus, if genuinely useful, a headless example harness (following
  `tool_token_ingestion_e2e.rs`/`user_input_chunking_e2e.rs`/`raw_partition_storage_e2e.rs`).
  Confirm via a repo-wide frontend grep that no WDIO/GUI spec is warranted before assuming so
  (this epic's established practice) — expected outcome: none needed, this is backend scoring
  logic with no frontend surface.
- Update `../state.md`'s "Child Nodes" list and `../handoff.md` to reflect this session's
  work, current build/test status, and the remaining backlog (the endpoint-carrying
  `TokenDetail` extension, the memory-alias gap — rf-13 now resolved).

Out of scope (do NOT change here):
- The endpoint-carrying `TokenDetail`/`invoke()` extension `tool-token-ingestion` deliberately
  left out of scope
- The `"memory"`-alias/`system_referential` gap `user-input-chunking` flagged
- Any frontend/UI work — this is pure backend scoring logic with no user-facing surface
- Graph traversal / `explore_memory`'s existing ranking mechanics (already complete, do not
  re-litigate `retrieval-fidelity`'s design)
