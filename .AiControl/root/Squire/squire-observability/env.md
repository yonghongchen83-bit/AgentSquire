# Env

- Parent node: root/Squire
- Node path: root/Squire/squire-observability
- Objective: build dedicated debug/observability facilities for the Squire "semantic loop" —
  structured tracing of retrieval scoring, token lifecycle, the explore/detail/invoke funnel,
  per-turn store snapshots, and timing — plus a query-probe dev surface for ad hoc tuning,
  because `provider-wire.log` shows only the explore call args and its result, not the internal
  scoring/near-misses/embedding-path that actually explain why a query did or didn't retrieve a
  token.
- Scope: a new `squire_trace` module + JSONL appender writing `squire-trace.log`, gated by a
  dedicated debug flag; instrumentation of `explore_memory` (both backends) to surface
  per-candidate score breakdown (`cosine`/`substr_boost`) and near-misses filtered by the
  `score<=0.0` cut; trace-event emission from `SquireExploreTool::execute`,
  `SquireTokenToDetailTool::execute`, `SquireInvokeTool::execute`, `finalize_turn`
  (token-lifecycle events), and a per-turn store-snapshot event; timing instrumentation (embed
  latency, explore latency, model init duration); a query-probe dev command/entry point that
  runs an arbitrary query against the current store and dumps ranked scores + near-misses +
  embedding path without driving the full agent loop.
- Non-goal: anything in `tool-token-registry`'s own scope (embedding model choice, distance
  computation, `tool_package`/`tool_resource` token types) — this node only tags/consumes that
  work's output; anything in `testing`'s own scope (auditing/closing existing coverage gaps) —
  this node produces the instrumentation `testing` will assert against, it does not itself
  perform the consolidation pass; reopening the nested-`§!`-citation residual
  `hit-count-fidelity`/`memory-alias-fix` deliberately left as a permanent simplification; a
  full dev-panel UI to visualize the trace log (JSONL is directly greppable/parseable; a viewer
  is a future node's concern if wanted).
- Depends on: `tool-token-registry` (the real embedding model swap — `fastembed BGESmallENV15`,
  384-dim, replacing the toy bag-of-words hash in `embed_text` — this node's embedding-path
  tagging exists specifically to distinguish real-model scores from the fallback hash path, and
  its "make html" no-word-overlap worked example is this node's own verification target too),
  `squire-storage` (`LanceDbSquireStore`, `explore_memory`'s scoring loop this node instruments),
  `retrieval-fidelity` (`accumulated_hits`/`effective_priority`, `num_hops` graph traversal — the
  retrieval-trace event's `hop_distance`/`via_token_id`/`accumulated_hits` fields surface exactly
  what that node added), `tool-token-ingestion` (`ingest_tool_registry`, the token-creation path
  the token-lifecycle event observes). Is itself a DEPENDENCY of `testing` — the instrumentation
  this node builds is what a growing test suite needs to assert against instead of eyeballing
  `provider-wire.log`.
- Status: planned, created 2026-07-03. Implementation not yet started.

## Durable facts (verified against real code this session, before node creation)

- **Real embedding model is live** (`src-tauri/src/storage/embedding.rs`): `fastembed`'s
  `BGESmallENV15` model (`EMBED_DIM = 384`, line 32) is initialized via
  `TextInitOptions::new(EmbeddingModel::BGESmallENV15)` (line 44), logging `"Squire embedding:
  initialized fastembed BGESmallENV15 ({EMBED_DIM}-dim) for semantic search"` on success. A
  fallback bag-of-words hash path exists for offline/init-failure (logging `"Squire embedding:
  failed to initialize fastembed model ({e}); ..."`), still producing an `EMBED_DIM`-wide vector
  so the LanceDB schema never varies by path. `embed_text` (line 83) is the single dispatch point
  used at both ingest and query time — this is the one place a trace tag needs to hook to know
  which path scored a given call. **This corrects a stale claim in `../env.md`** ("Vector search
  uses a deterministic hash-based embedding, not a real embedding model — there is no
  embedding-model provider in this codebase") — that was accurate when written but is now
  superseded by `tool-token-registry`'s real-embedding-model swap; `../env.md` is updated as
  part of this node's write-back.
- **`LanceDbSquireStore::explore_memory`** (`src-tauri/src/storage/squire_lancedb.rs:592`): the
  scoring loop computes, per candidate row (lines 681-699), `sim = cosine_similarity(qe,
  &row_vec)` and `substr_boost` (0.5 if the token's `token_id` or `short_desc`, lowercased,
  contains the raw lowercased query, else 0.0); final `score = sim + substr_boost`. The filter
  `if query_embedding.is_some() && score <= 0.0 { continue; }` (line 704) is exactly where a
  near-miss is currently silently discarded before being appended to `scored` — this is the one
  code path both the retrieval-trace event and the query-probe command need to instrument/reuse
  to surface near-misses, since today nothing downstream of this function ever sees a
  filtered-out candidate's score.
- **`provider-wire.log` gating pattern** (`src-tauri/src/llm/openai.rs`): a `verbose: bool`
  field on `OpenAIProvider` (line 21), checked at ~10 call sites before calling `append_wire_log`
  (line 35), which writes to `wire_log_path()` (line 31, `config_dir().join("provider-wire.log")`)
  via `OpenOptions::new().create(true).append(true)`. `AppConfig::verbose_logging`
  (`src-tauri/src/state/config.rs:38`) is the existing config-level flag this pattern reads from.
  A new trace flag/appender should follow this exact shape (boolean gate, dedicated log-path
  helper under `config_dir()`, append-mode file open) — this node deliberately keeps
  `squire-trace.log` and `provider-wire.log` as two separate files/mechanisms rather than merging
  them, since they answer different questions (wire log = raw LLM API exchange; trace = internal
  semantic-loop scoring/lifecycle) and a dedicated flag lets trace be enabled without forcing
  full wire-log verbosity.
- **Function/line map as of this session** (`src-tauri/src/agent/squire.rs`): `SquireExploreTool`
  struct at 991, `impl Tool` (`execute`) at 1002; `SquireTokenToDetailTool` struct at 1076,
  `impl Tool` at 1082; `SquireInvokeTool` struct at 1157, `impl Tool` at 1178; `build_turn_input`
  at 1437 (calls `ingest_user_input_chunks`, free function at 925); `finalize_turn` at 1510;
  `insert_relationship` trait method declared at 148, `InMemorySquireStore` impl at 385. Line
  numbers drift as sibling nodes land — re-grep before relying on exact numbers at implementation
  time.

## Useful commands

- `cd src-tauri && cargo build && cargo build --bins && cargo build --examples && cargo test --lib`
- `cd src-tauri && cargo run --example tool_token_ingestion_e2e` (or any other existing
  `*_e2e.rs` harness) — reference shape for a new headless harness or query-probe CLI entry, if
  that route is chosen over a Tauri command (see `decisions.md`).
- See `../env.md` for the full parent-level command reference (protoc build prerequisite,
  frontend test/typecheck commands, WDIO e2e commands, the free-tier test LLM provider already
  configured for real end-to-end verification).
