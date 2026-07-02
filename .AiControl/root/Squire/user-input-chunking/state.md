# State

## Timeline

- 2026-07-03: Node created as a sibling of `session-creation-ux`/`tool-token-ingestion` under
  `root/Squire`, to close the "user-input auto-chunking into `USR_TN_NNN` tokens" gap first
  flagged by `protocol-doc-sync` (item 11) and carried in `handoff.md`'s residual backlog
  ever since. Repointed `.AiControl/.current` to this node.
- Read context: `context_squire_spec_v2.md` §3.1 (System Referential Token shape + the
  `USR_T2_001` worked example), §3.2 (token record fields — confirmed no `chunk_ref` field
  exists in the runtime schema), §4.1 (structured vs. raw partition), §4.3 (the primary
  "Ingestion Rules" description of this feature plus its "not implemented" runtime-status
  note), §9.1 step 2 (Turn Open sequence — chunking precedes vector search), §11 (Squire
  Responsibilities checklist), §17 (glossary's `[not impl.]`-tagged "System Referential
  Token" entry). Read `protocol-doc-sync/decisions.md`'s item 11 in full — confirmed it only
  identifies the gap and cross-references the same spec sections; it does not itself resolve
  any of the granularity/numbering/reference-usage ambiguity, leaving that to this node's own
  judgment (documented in decisions.md). Read `planning/decisions.md` Q1-Q7 in full — confirmed
  none mentions user-input chunking at all, matching `protocol-doc-sync`'s own conclusion that
  this is unclaimed drift, not an intentional deferral.
- Read the actual code in full: `src-tauri/src/agent/squire.rs` — `SquireStore` trait,
  `NewTokenSpec`, `InMemorySquireStore`/`LanceDbSquireStore`'s `upsert_token` semantics
  (confirmed identical to `tool-token-ingestion`'s prior findings — both stores merge
  `full_desc` and increment `accumulated_hits` on every call), and — most importantly —
  `SquireContextAdapter::build_turn_input` (confirmed exactly as the spec's own
  implementation-status notes describe: reads the latest `MessageRole::User` message's raw
  `content` verbatim into `user_request`, with the bootstrap `explore_memory("all",
  &user_text, 1, 10, current_turn)` call immediately following it in the same function).
- Verified baseline: `cargo build` clean, `cargo test --lib` **173/173 passing** — matches
  `handoff.md` exactly, no drift.
- Designed the chunking scheme in `decisions.md` before implementing, resolving the four
  concrete ambiguities the task called out: (1) trigger — every user message, unconditional,
  no size threshold; (2) chunk granularity — paragraph-then-sentence splitting on plain
  syntactic boundaries (a documented judgment call, not true semantic/NLP chunking, chosen
  as proportionate to the spec's three-word "natural language structure" phrase and to
  avoid a new dependency/model-call for this step); (3) id/numbering scheme —
  `USR_T{turn}_{NNN}`, turn-scoped sequence starting at `001` and resetting each turn,
  matching §3.1's own `USR_T2_001` worked example read literally; (4) referencing —
  chunk tokens are ordinary store tokens the AI can `explore()`/`token_to_detail()`/
  `relationships`-link afterward like any other token; no sigil-parsing of the user's own
  raw input text is added, since sigils are exclusively an AI-output convention per
  §5.1/§5.2 and the spec never describes users typing sigil syntax.

## Decisions

(See `decisions.md` for the full design: the four judgment calls and their rationale, the
`chunk_user_input`/`first_sentence`/`ingest_user_input_chunks` function shapes, the call-site
placement and ordering requirement relative to the bootstrap `explore_memory` call, why no
new `SquireStore` trait method or schema field was needed, why AI-response-parsing code is
untouched, the accepted sentence-split imprecision on abbreviations/decimals, the
`CHUNK_SOFT_LIMIT_CHARS` constant and its rationale, the known but not-claimed
`"memory"`-alias-doesn't-include-`system_referential` follow-up, and the verification
methodology.)

## Implementation

- Added to `src-tauri/src/agent/squire.rs`, in a new "User-input chunking" section placed
  right before "Built-in tools" (mirroring `ingest_tool_registry`'s placement pattern from
  `tool-token-ingestion`):
  - `CHUNK_SOFT_LIMIT_CHARS: usize = 400` — the documented, non-spec-derived paragraph
    size cap above which sentence-splitting kicks in.
  - `chunk_user_input(text: &str) -> Vec<String>` — paragraph split on blank lines, then
    sentence split (via `split_into_sentences`) for any paragraph over the soft limit.
  - `split_into_sentences(paragraph: &str) -> Vec<String>` (private) — splits on
    `.`/`!`/`?` followed by whitespace/end-of-string.
  - `first_sentence(chunk: &str) -> String` (private) — up to the first sentence
    terminator or newline, or the whole chunk; used for `short_desc` per spec §3.1's
    literal field comment.
  - `ingest_user_input_chunks(text: &str, turn: u64, store: &dyn SquireStore)` — the public
    ingestion entry point, backend-agnostic via `SquireStore::upsert_token` only (no new
    trait method), writing `USR_T{turn}_{NNN}`-id `system_referential`-typed tokens with
    `full_desc` = the chunk's own text and `short_desc` = its first sentence.
- Call site: `SquireContextAdapter::build_turn_input`, immediately after resolving
  `user_text` and `current_turn`, and — critically — *before* the existing bootstrap
  `explore_memory("all", &user_text, 1, 10, current_turn)` call, so a turn's own
  freshly-chunked input is bootstrap-discoverable within that same turn (spec §9.1's
  numbered step order: chunking at step 2 precedes vector search at step 3). No other
  function was touched; `finalize_turn`/`validate_squire_response`/sigil-parsing code is
  completely unmodified (see decisions.md for why that's correct, not an oversight).

## Testing

- **17 new unit tests in `squire.rs`** (against `InMemorySquireStore` plus pure-function
  tests with no store dependency): `chunk_user_input` (short message = one chunk, empty/
  whitespace = no chunks, blank-line paragraph splitting, short paragraph with multiple
  sentences NOT split, long paragraph split into sentences, multiple long paragraphs
  handled independently), `first_sentence` (extracts up to terminator, falls back to whole
  chunk, stops at newline), `ingest_user_input_chunks` (id scheme/numbering exactness,
  `system_referential` type + `explore()` discoverability, per-turn numbering reset across
  two calls, `creation_turn`/`accumulated_hits` correctness, empty-text no-op, confirms no
  relationships are ever written per spec §4.3's explicit "no relationships are
  auto-generated"), and two `build_turn_input` integration tests confirming the real call
  site wires in correctly and multi-paragraph messages produce multiple tokens.
- **3 new unit tests in `squire_lancedb.rs`** (against the real `LanceDbSquireStore`):
  id-scheme/content-shape parity, `explore(resource_type="system_referential")`
  discoverability, and persistence of a chunk token across a fresh connection reopen —
  mirroring `tool-token-ingestion`'s precedent of confirming backend-agnostic logic isn't
  incidentally passing only against the in-memory stand-in.
- `cargo build`: clean, zero warnings. `cargo build --bins`: clean, zero warnings.
  `cargo test --lib`: **193/193 passing** (173 baseline + 17 in `squire.rs` + 3 in
  `squire_lancedb.rs`). No existing test needed modification — the new call site is
  additive to `build_turn_input`'s behavior (the pre-existing `user_request` field and
  `prefetched_tokens`/`preserved_tokens` array shapes are unchanged; the pre-existing
  `build_turn_input_ignores_base_tools_and_exposes_only_built_ins` test's single-short-
  message fixture ("hello squire") still passes unmodified since it never asserted an
  empty/exact `prefetched_tokens` array — it only checks `is_array()`).

## Verification methodology: unit tests (both backends) plus a real headless integration
## harness against real LanceDB — no WDIO/GUI e2e spec, same judgment as `tool-token-ingestion`

This node is a pure backend turn-open write path with **no new user-facing surface** —
confirmed via a repo-wide grep for `USR_T`/`system_referential` across `src/` (the
frontend), which found zero references. Its effect is only observable through `explore()`
results a model itself requests, exactly the same category of change as
`tool-token-ingestion`'s ss-9 work, which already established (and this node's own review
confirmed still applies) that a WDIO/GUI spec here would be a strictly weaker, more
indirect verification signal than asserting directly on real store rows — at best it could
ask a real model to `explore()` and hope it mentions a chunk, rather than directly
confirming the rows/ids/discoverability the way a backend check can.

New `src-tauri/examples/user_input_chunking_e2e.rs` (`cargo run --example
user_input_chunking_e2e`, no LLM/network needed — chunking is deterministic Rust, unlike
`ask_user_e2e.rs`'s model-dependent behavior) runs the exact real production call path
(`SquireContextAdapter::build_turn_input` -> `ingest_user_input_chunks` ->
`SquireStore`/`SquireExploreTool`) against a real temp-directory `LanceDbSquireStore`.
**Ran successfully, all assertions passed:** a real two-paragraph user message produced
real `USR_T0_001`/`USR_T0_002` rows via the real adapter (not a direct, isolated call to
`ingest_user_input_chunks`); the same turn's own `prefetched_tokens` in the real request
JSON included one of its own freshly-created chunks (confirming the before-bootstrap
ordering requirement holds in the real adapter, not just in a hand-constructed unit test);
the real `SquireExploreTool` surfaced the chunk via
`explore(resource_type="system_referential", query="proposal")`; and a second turn's
single-sentence message restarted numbering at `USR_T1_001` rather than continuing
`USR_T1_003`, confirmed against the real backend across two separate `build_turn_input`
calls sharing one session. Full console transcript below.

```
===== turn 1: build_turn_input on a two-paragraph user message =====
Confirmed real rows exist: USR_T0_001, USR_T0_002
USR_T0_001 full_desc: Some("Please review my project proposal.")

===== prefetched_tokens includes this turn's own chunk(s): true =====

===== explore(resource_type="system_referential", query="proposal") =====
[{"token_id":"USR_T0_001","type":"system_referential","score":0.5,"short_desc":"Please review my project proposal.","accumulated_hits":1,"hop_distance":0,"via_token_id":null}]

===== turn 2: build_turn_input on a single-sentence follow-up =====
Confirmed: USR_T1_001 exists, numbering restarted (no USR_T1_002)

===== summary =====
All assertions passed:
  - build_turn_input's real call path creates real USR_T{turn}_{NNN} rows
  - a turn's own chunk(s) are bootstrap-discoverable within that same turn
  - explore(resource_type="system_referential") surfaces them via the real tool
  - per-turn numbering reset holds against the real backend across two turns
```

**Not pursued, and why:** a real-model WDIO+tauri-driver run (as `ask-user-loop`/
`session-creation-ux` performed for genuinely new UI/interaction surfaces) was considered
but judged unnecessary for the same reason `tool-token-ingestion` gave for its own,
similarly backend-only change — there is no new UI affordance for a GUI spec to drive, and
a model-driven check would only indirectly infer success by hoping the model happens to
mention or reference a chunk token, a weaker signal than the direct backend assertions
already performed above. The free-tier test LLM provider and WDIO/tauri-driver setup remain
available and working (confirmed still configured per `handoff.md`) for any future session
that wants to observe a real model actually choosing to `explore()`/reference a
`system_referential` chunk mid-conversation — not attempted here as it would not
strengthen confidence beyond what the harness above already confirms deterministically.

## Risks

- Sentence-boundary splitting (`split_into_sentences`, used only for paragraphs over 400
  characters) does not special-case abbreviations ("Dr. Smith") or decimal numbers
  ("3.14") — a real NLP sentence tokenizer would avoid these mis-splits. Accepted as a
  documented, cosmetic imprecision (see decisions.md) rather than a defect: a mis-split
  chunk is still a valid, retrievable token with slightly awkward boundaries, and nothing
  downstream requires grammatically perfect sentence boundaries.
- The `400`-character `CHUNK_SOFT_LIMIT_CHARS` threshold and the choice of
  paragraph-then-sentence splitting as "natural language structure" are both documented
  judgment calls, not values or a granularity derived from the spec itself (the spec gives
  no chunk-size constant and no algorithm detail beyond the three-word phrase). A future
  session could reasonably choose different values/approach without contradicting the spec.
- `type_matches`'s pre-existing `"memory"` alias (both stores) does not include
  `system_referential` in its expansion (only `concept`/`referential`) — so
  `explore(resource_type="memory", ...)` will not surface chunk tokens even though §4.1
  conceptually places system-referential tokens in the same "structured partition...
  primary memory partition." Deliberately not changed by this node (pre-existing,
  already-tested code, and `"memory"` isn't itself a spec-defined `resource_type` enum
  value) — flagged as a known, not-claimed follow-up in decisions.md.
- No relationships are ever auto-generated for chunk tokens (matching the spec's explicit
  "No relationships are auto-generated" instruction) — an unconnected chunk token is only
  as reachable as the AI chooses to make it via `explore()`/its own `relationships` field,
  same as the spec describes and same caveat that already applies to any other
  AI-created token today.

## Closure summary

The user-input auto-chunking gap (`protocol-doc-sync` item 11; spec §3.1/§4.3/§9.1 step
2/§11) is resolved. `SquireContextAdapter::build_turn_input` now auto-chunks the latest user
message by paragraph-then-sentence "natural language structure" (a documented judgment call
— see decisions.md's four resolved ambiguities) into one or more `USR_T{turn}_{NNN}`-id
`system_referential`-typed `SquireStore` tokens, created before the turn's bootstrap vector
search so they are immediately discoverable within the same turn. No new `SquireStore`
trait method or schema field was needed; no relationships are auto-generated (per spec); no
sigil-parsing of user input was added (sigils remain exclusively an AI-output convention).

All verification passed: `cargo build`/`cargo build --bins` clean with zero warnings,
`cargo test --lib` 193/193 (173 baseline + 20 new across both `SquireStore` backends). A new
headless integration harness (`src-tauri/examples/user_input_chunking_e2e.rs`) additionally
confirmed the exact real production call chain (`build_turn_input` -> chunking -> store ->
`explore()`) end to end against a real `LanceDbSquireStore` backend, including the
same-turn bootstrap-discoverability ordering requirement and the per-turn numbering-reset
behavior — no WDIO/GUI e2e spec was built, since this node introduces no new user-facing
surface (confirmed via a repo-wide frontend grep), matching `tool-token-ingestion`'s
precedent and rationale exactly.

## Next Actions

- Node scope complete for its one stated deliverable — ready to be marked complete.
- Remaining Squire-epic backlog after this node: raw-partition audit-log storage; the
  endpoint-carrying `TokenDetail`/`invoke()` extension `tool-token-ingestion` deliberately
  left out of scope; `retrieval-fidelity/todo.json` rf-13 (fuller hit-count-event fidelity);
  the two small optional UX follow-ups `session-creation-ux` surfaced (toggle persistence,
  active-conversation mode indicator). None of these block normal use of Squire mode.
- `src-tauri/examples/user_input_chunking_e2e.rs` is left in the repo as reusable
  verification tooling for any future chunking-related work (e.g. if the `"memory"`-alias
  follow-up flagged above is ever picked up, or if a future session wants a starting point
  for a real-model-driven check of chunk-token referencing).
