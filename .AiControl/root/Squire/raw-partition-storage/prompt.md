# Prompt

Close the last unclaimed item from `protocol-doc-sync`'s 4 newly-discovered gaps
(decisions.md item 12): **raw-partition audit-log storage** (spec §4.1/§4.3/§9.4 step 4/§11,
glossary).

> **Raw partition** — auto-stored AI output that was not explicitly marked with §^. Reachable
> only by vector similarity. No token representation, no graph connections. Treated as an
> audit log, not as memory. Explore() does not search this partition by default.
>
> **Implementation status (runtime v1): not implemented.** `squire_lancedb.rs` has no
> raw-partition table (its five tables are `squire_tokens`, `squire_relationships`,
> `squire_turns`, `squire_preserve_lists`, `squire_compliance_failures` — none of which hold
> unmarked AI output). `finalize_turn` only persists the display-expanded, sigil-stripped
> `content` string to `ConversationStore` (the ordinary chat-message history) — unmarked
> prose is not separately archived to any Squire-owned audit log the way this section
> describes. A genuine gap, not a deliberate simplification; flagged here rather than
> silently glossed over. See `decisions.md`.

Deliverables:

- Read `../context_squire_spec_v2.md` (protocol-doc-sync-reconciled) in full for every place
  this gap is mentioned: §4.1 (structured vs. raw partition — the primary definition), §4.3
  (Ingestion Rules — "if the AI does not mark a span, it is stored only in the raw
  partition"), §5.2 (§^ named span — the mechanism that determines what counts as "marked"),
  §9.4 step 4 (Turn Close numbered sequence), §11 (Squire Responsibilities), §17 (glossary).
  **Resolve, using careful reading and your own judgment, the central ambiguity the task
  flags: is the raw partition (a) a full, verbatim audit trail of every AI response
  (redundant with the ordinary chat-history table), or (b) specifically the unmarked portions
  of AI output that were not promoted into a `§^`-marked memory token?** Document the exact
  textual basis for whichever reading you land on in `decisions.md` before writing any code.
  Also resolve: is this partition ever read back by the model itself (e.g. via a tool), or
  is it purely for human/operator audit outside the model's own context? (§4.1's "Explore()
  does not search this partition by default" is the load-bearing clue here — read closely
  whether "by default" implies an optional/future read path this node should still consider,
  or whether no read-back mechanism exists anywhere in the spec at all.)
- Read `../protocol-doc-sync/decisions.md` (item 12, "Raw-partition audit-log storage") for
  exactly how this gap was first scoped/described, and whether it flagged any spec ambiguity
  of its own.
- Read `../squire-storage/decisions.md` for the existing `SquireStore` trait/LanceDB table
  design patterns (five tables as of `rejection-ux`: `squire_tokens`,
  `squire_relationships`, `squire_turns`, `squire_preserve_lists`,
  `squire_compliance_failures`) so a new table/trait method fits the same shape and
  conventions — pay particular attention to `squire_compliance_failures`'s "append-only,
  debugging-only, never read back" design, the closest existing precedent.
- Read the actual code: `src-tauri/src/agent/squire.rs` in full, especially
  `SquireContextAdapter::finalize_turn` (the exact function every implementation-status note
  for this gap names) and wherever the raw model response string (`parsed.content`) is
  available before/after `extract_spans`/sigil parsing; and
  `src-tauri/src/storage/squire_lancedb.rs` in full, for the existing table
  schema/open/query/write patterns to match exactly.
- Verify baseline first: `cargo build` + `cargo test --lib` from `src-tauri/` (expect clean,
  193/193 passing). `protoc` may be needed for a cold build — see `../handoff.md`.
- Implement the raw-partition store per whatever the spec's actual intent turns out to be
  (from the reading above): a new `SquireStore` trait method, a new LanceDB table, and an
  `InMemorySquireStore` equivalent, wired into `finalize_turn` at the point where the raw
  response/parsed spans are available. Keep scope proportionate — a straightforward
  append-only write path matching the existing tables' style is almost certainly sufficient;
  do not build a full-text-search audit UI, a read-back/query API the model itself can call,
  or a retention/rotation policy unless the spec text actually calls for one.
- Add real unit tests for the new storage path in both `squire.rs` (against
  `InMemorySquireStore`) and `squire_lancedb.rs` (against the real `LanceDbSquireStore`),
  matching the existing test suites' style.
- Verify manually/e2e if practical. A repo-wide grep for any related frontend surface should
  be done first (the same check `user-input-chunking` performed for its own backend-only
  change) — if none is found, a headless integration harness in the style of
  `src-tauri/examples/tool_token_ingestion_e2e.rs` / `user_input_chunking_e2e.rs` is almost
  certainly the right verification tier; do not build a WDIO/GUI spec unless that assumption
  turns out to be wrong.
- Update `../state.md`'s "Child Nodes" list and `../handoff.md` to reflect this session's
  work, current build/test status, and the remaining backlog (rf-13, the endpoint-carrying
  `TokenDetail` extension, the `"memory"`-alias/`system_referential` gap, the two small
  session-creation-ux UX follow-ups — raw-partition storage now resolved).

Out of scope (do NOT change here):
- `retrieval-fidelity/todo.json` rf-13 (fuller hit-count-event fidelity)
- The endpoint-carrying `TokenDetail`/`invoke()` extension `tool-token-ingestion` deliberately
  left out of scope
- The `"memory"`-alias/`system_referential` gap `user-input-chunking` flagged
- Any frontend/UI work — this is a pure backend turn-close write path, same category as
  `tool-token-ingestion`/`user-input-chunking`
- Any read-back/query mechanism for this data invoked by the model itself, unless careful
  reading of the spec genuinely requires one (expected outcome: it does not)
