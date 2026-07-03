# Prompt

Build debug/observability facilities to see what happens inside the Squire "semantic loop" —
because the test suite (`testing`) is going to grow more complex and span other parts of the
design spec, and correctness of semantic retrieval / token accretion cannot be verified by
eyeballing `provider-wire.log`. This node is a DEPENDENCY of `testing` (it provides the
instrumentation the tests will assert against). It is closely related to `tool-token-registry`
(which just landed real embeddings — `fastembed BGESmallENV15`, 384-dim — the exact thing this
node needs to distinguish from the bag-of-words fallback path in every trace event).

## Confirmed current state (context, verified against real code before this node was created)

- Real embedding model is live: `src-tauri/src/storage/embedding.rs` initializes `fastembed`'s
  `BGESmallENV15` model (384-dim) via `TextInitOptions::new(EmbeddingModel::BGESmallENV15)`,
  logging `"Squire embedding: initialized fastembed BGESmallENV15 ({EMBED_DIM}-dim) for semantic
  search"` on success. A fallback bag-of-words hash path (`embed_text_fallback`, still
  `EMBED_DIM`-wide so the schema never varies) exists for offline/init-failure, logging
  `"Squire embedding: failed to initialize fastembed model ({e}); ..."` when it's taken. This
  supersedes the parent epic's `env.md` note describing embedding as "a deterministic hash-based
  embedding, not a real embedding model" — that note is now stale and is corrected as part of
  this node's write-back (see `../env.md`).
- `provider-wire.log` (`src-tauri/src/llm/openai.rs`, `append_wire_log`/`verbose` gating) already
  shows the explore query args (assistant tool_call) and the result returned to the LLM
  (tool-role message), but it does NOT contain the internal scoring, near-misses, or which
  embedding path ran for a given query — that is the gap this node fills. `provider-wire.log` and
  the new `squire-trace.log` this node adds are deliberately separate files/mechanisms; this node
  does not touch `provider-wire.log`'s own format or gating.

## Design to capture (be specific)

Deliver a dedicated structured trace: **`squire-trace.log` as JSONL** (separate from
`provider-wire.log`), one event per line `{turn, tool_call_id?, event, payload, ts}`, gated
behind a debug flag/config. Events, correlated by `turn` + `tool_call_id`:

1. **RETRIEVAL TRACE (highest value, build first).** Per `explore()` call — log `query`,
   `resource_type`, `num_hops`, `max_results`, `turn`; and for EACH candidate returned:
   `token_id`, `token_type`, `score` broken into `cosine` vs `substr_boost` components,
   `hop_distance`, `via_token_id`, `accumulated_hits`. ALSO log the filtered-out NEAR-MISSES
   (candidates that were scored but dropped by the `score<=0` cut, or below the returned top-N)
   with their scores — this is what reveals "coding scored 0.31, cut". Tag the event with which
   EMBEDDING PATH scored it (real `bge-small` vs fallback hash) — critical because a word-overlap
   hit looks identical under both.
2. **TOKEN LIFECYCLE.** Per turn — tokens CREATED (auto `USR_*` input chunks + model-emitted:
   id/type/short_desc/creation_turn), tokens PRESERVED (which ids), RELATIONSHIPS written
   (subject/predicate/object). Lets us watch the store accrete as intended.
3. **FUNNEL.** `token_to_detail` calls (token_id, detail_level, returned) and `invoke` calls
   (token_id, params, result, is_error) — reconstructs the AI's explore -> detail -> invoke
   decision chain per turn.
4. **PER-TURN STORE SNAPSHOT.** Token counts by type + accumulated_hits distribution + total
   store size, to track growth/reinforcement over turns.
5. **TIMING.** Embed-inference latency per call, explore latency, model init/download duration
   (also surfaces the blocking-in-async concern).
6. **QUERY-PROBE DEV COMMAND/endpoint.** Run an ARBITRARY query against the current store and
   dump ranked scores (incl. near-misses + embedding path) WITHOUT driving the whole agent — the
   fastest way to answer "why didn't 'make html' retrieve the coding token?". This is a key
   tuning tool.

## Relevant code locations (verified this session; for the todos/env durable facts)

- `src-tauri/src/agent/squire.rs`:
  - `SquireExploreTool` struct at line 991, `impl Tool for SquireExploreTool` (`execute`) at
    line 1002 — the explore entry point, including the live-registry tool branch and the
    `explore_memory` store branch.
  - `SquireTokenToDetailTool` struct at line 1076, `impl Tool` at line 1082.
  - `SquireInvokeTool` struct at line 1157, `impl Tool` at line 1178.
  - `finalize_turn` at line 1510 — parses `SquireResponse` and writes tokens/relationships
    (`insert_relationship` trait method declared at line 148, `InMemorySquireStore` impl at
    line 385).
  - `build_turn_input` at line 1437, calling `ingest_user_input_chunks` (free function at line
    925) and the bootstrap `explore_memory` call for same-turn discoverability.
- `src-tauri/src/storage/squire_lancedb.rs`:
  - `LanceDbSquireStore::explore_memory` at line 592 — the scoring loop. Per-candidate score is
    `sim + substr_boost` (line 699: `sim` = `cosine_similarity(qe, &row_vec)`, `substr_boost` =
    0.5 if `token_id`/`short_desc` case-insensitively contains the raw query else 0.0, computed
    at lines 681-699); the `score <= 0.0` cut is at line 704 (`if query_embedding.is_some() &&
    score <= 0.0 { continue; }`) — this is exactly where near-misses must be captured before
    they're discarded, and where the final ranked/returned set is assembled afterward.
  - `embed_text` provenance: imported from `crate::storage::embedding` (line 40).
- `src-tauri/src/storage/embedding.rs`: `EMBED_DIM = 384` (line 32); `TextEmbedding::try_new`
  init at line 44 logging success/failure; `embed_text` at line 83 dispatches to the real model
  or the fallback (`embed_text_fallback`, hash-based, from line ~111) — reuse this file's
  existing success/failure signal to tag every trace event with which path produced a score,
  rather than re-deriving it independently.
- Config/flag: follow how existing config/verbose flags work — `src-tauri/src/state/config.rs`
  (`AppConfig::verbose_logging` at line 38) and `src-tauri/src/llm/openai.rs`'s
  `append_wire_log`/`verbose` gating pattern (`wire_log_path` at line 31, `append_wire_log` at
  line 35, `verbose: bool` field at line 21, checked at each of ~10 call sites before writing).
  A new trace flag/module should follow this same shape (a boolean gate checked before every
  write, a dedicated log-file-path helper under `config_dir()`) rather than inventing a
  different logging idiom.

## Open decisions to record (resolve in `decisions.md` before implementing, matching sibling-node practice)

- **Debug-flag mechanism**: reuse the existing `verbose_logging`/wire-log gating vs. a dedicated
  `SQUIRE_TRACE` env/config flag. Recommend a dedicated flag so the trace can be enabled without
  also turning on full provider-wire verbose logging (they answer different questions — wire
  logging is "what did the LLM API exchange," trace is "what did the semantic loop actually
  score/do").
- **Trace format**: JSONL (recommended, one self-contained event object per line, easy to
  `jq`/parse in tests) vs. pretty text.
- **Query-probe surface**: a Tauri command (UI-invokable, usable from a future dev panel) vs. a
  CLI/test-only entry point (e.g. a `cargo run --example` harness, matching this epic's existing
  headless-harness precedent). Recommend a Tauri command so it's usable from a dev panel later,
  but a headless example harness may be the right *first* increment if the Tauri command turns
  out to need new plumbing this node shouldn't block on.

## Deliverables

- Read `../env.md` and `../decisions.md` (parent) for the settled architecture facts and
  testing-methodology conventions this node must respect (two-backend `SquireStore` parity, the
  proportionate-verification tiering system, the "no new trait method unless truly necessary"
  discipline, the corrected embedding-model fact this node itself is write-backing).
- Read `../tool-token-registry/decisions.md`/`env.md` in full — the real-embedding-model swap
  (fastembed `BGESmallENV15`, 384-dim, replacing the toy bag-of-words hash) is the load-bearing
  context this node's embedding-path tagging depends on; do not re-litigate that node's own
  scope (model choice, distance computation) here.
- Read the actual code in full before implementing (see "Relevant code locations" above,
  file:line citations verified this session) rather than relying solely on this prompt's
  summary, since line numbers drift as sibling nodes land.
- Design and document in `decisions.md`, before writing code: the debug-flag mechanism, trace
  format, and query-probe surface (see "Open decisions" above), plus the exact JSONL event
  schema (field names/types per event kind).
- Implement per `todo.json`'s ordered breakdown (obs-1..obs-10).
- Add real unit tests in the same style as sibling nodes' suites (two-backend parity for any
  `SquireStore`-touching instrumentation — e.g. if near-miss capture requires a return-shape
  change to `explore_memory`, both `InMemorySquireStore` and `LanceDbSquireStore` need it and
  need tests).
- Verify manually: run the two recorded real queries this epic already has evidence for —
  "web scraping fetch url html" (word-overlap case) and "make html" (NO word overlap, the true
  semantic-matching test from `tool-token-registry/decisions.md`'s worked example) — and confirm
  the trace shows the expected scores/embedding path for each.
- Update `../state.md`'s "Child Nodes" list and `../handoff.md` to reflect this node's work and
  final status (already done as part of this node's creation — see below).

## Out of scope (do NOT fix here)

- Anything in `tool-token-registry`'s own scope (embedding model choice, distance-computation
  strategy, `tool_package`/`tool_resource` token types, MCP registration timing) — this node
  only consumes/tags that work's output, it does not re-implement or re-decide it.
- Anything in `testing`'s own scope (auditing/closing existing test-coverage gaps, the e2e
  flakiness audit) — this node builds the instrumentation `testing` will assert against, it does
  not itself perform that consolidation pass.
- Reopening the nested-`§!`-citation residual `hit-count-fidelity`/`memory-alias-fix`
  deliberately left as a permanent simplification per direct user instruction — do not add trace
  events implying this should be revisited.
- Any frontend/UI work beyond the query-probe surface decision itself — a full dev panel UI to
  visualize `squire-trace.log` is explicitly not required by this node (the log is JSONL,
  greppable/parseable directly; a future node could add a viewer).
