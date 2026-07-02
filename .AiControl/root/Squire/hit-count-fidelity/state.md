# State

## Timeline

- 2026-07-03 Node created. Read `retrieval-fidelity/decisions.md` and `todo.json` in full
  (rf-13's exact wording and the "Hit-count increment events wired: 2 of the spec's 4, with
  rationale for the other 2" section), `context_squire_spec_v2.md` §3.2/§3.3 plus cross
  references (§5.1, §9.4 step 7, §10.1 step 7), and the full `squire.rs` source
  (`SquireStore::record_hit`, `SquireExploreTool`, `SquireTokenToDetailTool`,
  `SquireInvokeTool`, `SquireContextAdapter::finalize_turn`/`build_turn_input`/
  `expand_for_display`, and the sigil parsers `extract_inline_refs`/`extract_spans`/
  `unmarked_residual`/`strip_span_markers`). Confirmed baseline: `cargo build` clean,
  `cargo test --lib` 206/206 passing.

- 2026-07-03 Identified the precise, narrower gap: spec §6.1's own gloss on event 1 ("calls
  token_to_detail **or** references in output") is a disjunction, and `retrieval-fidelity`'s
  proxy only wired the first disjunct. The second disjunct — a token cited via `§!` in the
  AI's own response content without ever being loaded via `token_to_detail` — earned zero hit
  credit under the prior wiring. Determined that this same wiring point also satisfies event 3
  ("§! reference found in a chunk loaded into context") for the AI's own response content,
  since that content is unambiguously "loaded into context" via `expand_for_display`
  immediately afterward. See `decisions.md` for the full operationalization.

- 2026-07-03 Implemented: a new loop in `SquireContextAdapter::finalize_turn`, immediately
  after `self.retry_count = 0` (the compliant-response marker) and before the `new_tokens`
  upsert loop, calling `self.store.record_hit(token_id)` for every id in the pre-existing
  `known` set (already computed by `finalize_turn` as every `§!`-referenced id that
  `token_exists` **before** this turn's `new_tokens` are upserted). This correctly excludes a
  token that is both defined in `new_tokens` and cited in the same turn (it already gets its
  one hit from `upsert_token`'s "regardless" +1 rule), and correctly includes a pre-existing
  token merely cited without redefinition or a `token_to_detail` call. No `SquireStore` trait
  changes were needed — `record_hit` (added by `retrieval-fidelity`) already does exactly what
  was needed. No signature changes to any existing method. Updated the `record_hit` trait doc
  comment and the `SquireTokenToDetailTool::execute` inline comment to reflect the fuller
  wiring and cross-reference this node.

- 2026-07-03 Documented, explicitly and not silently, the one narrower residual left
  unaddressed: a `§!` reference nested *inside* a `full_desc` body itself (a chunk citing
  another chunk, only surfaced when that body is loaded via `token_to_detail`) is not scanned
  for embedded references. Closing this fully would require a genuine context-composition
  audit pass scanning every piece of content that ever enters context — the same
  disproportionate-infrastructure concern `retrieval-fidelity/decisions.md` already flagged
  for the original gap, now narrowed to a smaller, rarer authoring pattern (a token's own
  content citing a different token) rather than the far more common "AI cites a token directly
  in its visible output" pattern this node's change now fully covers. See `decisions.md`'s
  "What is deliberately still not wired" section for the full tradeoff argument.

- 2026-07-03 Added 4 new unit tests: 3 in `squire.rs` —
  `finalize_turn_credits_a_hit_for_a_preexisting_token_cited_via_sigil_without_token_to_detail`
  (confirms the exact gap rf-13 flagged is now closed: citing without calling
  `token_to_detail` still earns a hit), `finalize_turn_does_not_double_credit_a_token_defined_
  and_cited_in_the_same_turn` (confirms the double-count guard), and
  `finalize_turn_credits_exactly_one_hit_for_repeated_citations_of_the_same_token` (confirms
  the existing `HashSet`-based dedup semantics extend correctly to hit-crediting, not just
  validation) — plus 1 in `squire_lancedb.rs`,
  `record_hit_composes_with_upsert_matching_the_cite_without_redefine_pattern`, confirming the
  real LanceDB-backed `record_hit`/`upsert_token` primitives compose identically to what
  `finalize_turn`'s new call site now performs (`finalize_turn` itself is backend-agnostic —
  it holds `Arc<dyn SquireStore>` — so its own integration tests already exercise the new call
  site through `InMemorySquireStore`; this test confirms the real backend's storage-level
  arithmetic matches, the same "primitive-level parity, not adapter-level duplication"
  pattern `raw-partition-storage/decisions.md` established for its own real-backend coverage).
  One initial mistake caught and fixed during this session: two draft tests initially used
  `§!TokenId,` with punctuation immediately after the token id, which
  `take_token_id`/`extract_inline_refs`'s whitespace-only termination rule (spec §5.1's exact
  format: "terminated by the next whitespace or the next §") captures as part of the id
  itself, causing `token_known` to correctly reject the malformed reference and the turn to be
  rejected rather than closed — fixed by rephrasing the test content so citations are followed
  by a space before any punctuation, matching every other existing test's sigil-citation style
  in this file.

- 2026-07-03 Full verification: `cargo build`/`cargo build --bins`/`cargo build --examples`
  all clean, zero warnings. `cargo test --lib`: **210/210 passing** (206 baseline + 4 new: 3 in
  `squire.rs`, 1 in `squire_lancedb.rs`). Repo-wide frontend grep
  (`accumulated_hits`/`hit-count`/`hit_count`/`record_hit`, case-insensitive, across `src/`):
  zero hits — confirms this feature has no frontend surface whatsoever, matching every other
  backend-only node in this epic since `tool-token-ingestion`. No new headless example harness
  was built (see `decisions.md`'s verification-methodology section for why: this change is a
  single new loop inside an already-integration-tested function, calling an already-tested
  primitive method, with no new table, no new trait method, and no new cross-process/storage
  data flow the way `raw_partition_storage_e2e.rs`/`user_input_chunking_e2e.rs` each needed to
  exercise for their own new call chains).

## Decisions

See `decisions.md` for the full writeup: the precise restatement of what was missing (the
disjunction's second half), the operationalization of events 1/3 as one shared wiring point,
the double-count guard reasoning, the deliberately-deferred nested chunk-citing-chunk case and
its explicit tradeoff, the implementation approach (no trait/signature changes), and the
verification-methodology reasoning.

## Risks

- The one deliberately deferred residual (nested `§!` references inside a `full_desc` body
  are not scanned/credited when that body is loaded via `token_to_detail`) remains. This is a
  narrower, rarer gap than what rf-13 originally flagged (which covered the much more common
  "AI cites a token directly in its own output" pattern, now fully fixed) — not silently
  dropped, but not closed either. A future node could close it by extending
  `SquireTokenToDetailTool::execute` to scan the returned `full_desc`/`short_desc` via the same
  `extract_inline_refs` helper this node already uses and call `record_hit` on each resolved
  reference found inside it. Not claimed by any todo.json as of this writing.

## Next Actions

- None for this node — all todo.json items done. The nested chunk-citing-chunk residual above
  is noted for a future session to pick up independently if stricter fidelity is ever wanted.

## Closure summary

Closed `retrieval-fidelity/todo.json` rf-13 (fuller hit-count-event fidelity to spec §3.3's
four-event table). `SquireContextAdapter::finalize_turn` now credits a hit (via the
pre-existing `record_hit` method — no new `SquireStore` trait surface was needed) for every
already-existing token `§!`-referenced in a compliant response's content, closing the
"references in output" disjunct of event 1 ("Token appears in explore() results that AI acts
on") and event 3 ("§! reference found in a chunk loaded into context") for the AI's own
response content — the prior session's proxy only covered the "calls token_to_detail" half of
event 1's disjunction. Guarded against double-crediting a token that is both defined in
`new_tokens` and cited in the same turn (already credited once via `upsert_token`'s existing
event-4 rule). One narrower residual — nested `§!` references inside a `full_desc` body itself
— is deliberately left unwired and explicitly documented as a disproportionate-infrastructure
tradeoff, not a silent gap, per this epic's established practice (mirroring
`retrieval-fidelity`'s own original reasoning for the broader gap this node now substantially
narrows). Verified via 4 new unit tests (3 in `squire.rs` against `InMemorySquireStore`
through real `finalize_turn` integration tests, 1 in `squire_lancedb.rs` confirming the real
`LanceDbSquireStore`'s underlying primitives compose identically). `cargo build`/`cargo build
--bins`/`cargo build --examples`: all clean, zero warnings. `cargo test --lib`: 210/210
passing. Confirmed via repo-wide grep that this change has zero frontend surface — no
WDIO/GUI spec or new headless example harness was needed. Status: completed, 2026-07-03.
