# Env

- Parent node: root/Squire
- Node path: root/Squire/tool-token-registry
- Objective: fix broken tool discovery for natural-language queries by unifying built-in and
  MCP tool registration onto one `SquireStore` schema, discovered through the same store-backed
  path as every other token, instead of the live-registry contiguous-substring shortcut
  `SquireExploreTool::execute` currently uses for `resource_type="tool"|"tool_skill"`.
  Register-before-explore: registration happens at conversation/session start (built-ins) and
  at MCP-server-connect time (external tools), not lazily/reactively inside `explore()` itself.
- **SIMPLIFIED DESIGN (revised 2026-07-03, after discussion).** The originally-planned MCP
  summarization pipeline (a separate turn-0 LLM pre-pass batching tool descriptions into
  `[{tool, short_desc, keywords[]}]`, plus a global persistent cache keyed by `hash(raw_desc)`)
  is DROPPED. Rationale: the only reason to summarize/keyword-extract was to make verbose
  descriptions findable under a lexical/word-level matcher. Once a real embedding model
  (encoder) is bundled, LanceDB does true semantic vector matching on the FULL raw description,
  making summarization-for-retrieval unnecessary. See decisions.md for the full resolution and
  the worked "make html" example. The keystone of the simplified design: bundle ONE small local
  embedding model (encoder only, not generative), e.g. `bge-small-en-v1.5` (384-dim, ~130MB) or
  `all-MiniLM-L6-v2` (384-dim, ~90MB), via `fastembed-rs` (ONNX), replacing the toy bag-of-words
  `embed_text` (`squire_lancedb.rs:54-72`, bump `EMBED_DIM` 64 -> 384). `embed_text` is already
  called at both ingest (`squire_lancedb.rs:~464`) and query time (`squire_lancedb.rs:~613`), so
  one swap covers both — this is the load-bearing change; everything else is scaffolding
  around it.
- Scope: swap `embed_text` for the chosen local embedding encoder (fastembed-rs/ONNX), bump
  `EMBED_DIM` to 384, re-embed on both ingest and query; `ToolDefinition` gains an authored
  `short_description` for built-in tools (embed the FULL description, not the short one); new
  `tool_package`/`tool_resource` (or equivalent) token types in the store schema/type mapping;
  built-in registration moved to conversation/session start (idempotent), replacing/
  supplementing the existing per-turn lazy `ingest_tool_registry` call in `streaming_cmd.rs`;
  removal of `SquireExploreTool`'s live-registry substring shortcut for tool/tool_skill,
  rerouted through the store's semantic vector path; MCP tools registered at connect by
  embedding the FULL raw description directly (no LLM, no summarization, no cache) with a
  DETERMINISTIC-TRUNCATION display `short_desc` (e.g. first sentence / ~140 chars); package ->
  resource "exposes" relationship-triplet writes during ingestion so a package token is
  findable first and drillable via `num_hops`.
- Non-goal: a generative LLM / turn-0 pre-pass for MCP summarization, and any associated global
  summary cache — DROPPED (see decisions.md); do not reintroduce. Changing how `invoke()`/
  dispatch resolves `token_id -> endpoint` for actual tool execution (discovery moves to the
  store, execution stays wired to the registry/`ToolEndpoint` — see `token-detail-endpoint`'s
  prior work, which this node does not alter); reopening the nested-`§!`-citation residual
  `hit-count-fidelity`/`memory-alias-fix` deliberately left as a permanent simplification; any
  frontend/UI work unless a genuinely new user-facing surface turns out to be required (not
  expected); switching from manual cosine to LanceDB's native vector index (recommended
  follow-up, not required for this node — see decisions.md sub-decision 2).
- Depends on: `squire-adapter` (`SquireStore` trait, `SquireExploreTool`/`SquireInvokeTool`, the
  Q5 strict tool boundary), `squire-storage` (`LanceDbSquireStore`, the six-table LanceDB
  layout), `retrieval-fidelity` (`accumulated_hits`/`effective_priority`, the `num_hops` graph
  traversal over `squire_relationships` this node's package/resource "exposes" edges will be
  traversed by), `tool-token-ingestion` (the existing `ingest_tool_registry` free function, its
  per-turn trigger point in `streaming_cmd.rs`, its token-ID scheme and no-active-cleanup
  staleness precedent — this node changes *when*/*how* registration happens, not the underlying
  `upsert_token`-based mechanism), `token-detail-endpoint` (the `endpoint: Option<ToolEndpoint>`
  field on `TokenDetail`/`NewTokenSpec` that keeps real MCP dispatch working — must remain
  correct once discovery moves to the store), `memory-alias-fix` (the `"memory"` resource_type
  alias's token-type expansion list — a durable pattern to check/update if new token types are
  added here, matching the exact gap that node fixed for `system_referential`).
- Status: planned, created 2026-07-03; design simplified 2026-07-03 (see decisions.md/state.md
  for the revision). Implementation not yet started.

## Durable facts (read this session, verified against real code before node creation)

- `SquireExploreTool::execute` (`src-tauri/src/agent/squire.rs:1032-1053`) serves
  `resource_type="tool"|"tool_skill"` from the LIVE `ToolRegistry` using a contiguous-substring
  filter: `d.name.contains(ql) || d.description.contains(ql)`, where `ql` is the entire query
  string lowercased, unsplit. This is categorically different matching behavior from
  `explore_memory`'s word-level bag-of-words matching used for every other token type — the
  root cause of the reported discovery failure, since multi-word natural-language queries
  almost never appear as a contiguous substring of a tool's description even when every
  individual word matches.
- Confirmed failure evidence: `web_fetch`'s description ("Fetch a web page and return its HTML
  content. Useful for reading documentation, checking APIs, or scraping web content.",
  `src-tauri/src/agent/mod.rs:513-515`) shares words with real failing queries ("web scraping
  fetch url html", "scrape website data", "fetch url web page http request") but none matched
  as a contiguous substring. Routing tool discovery through `explore_memory`'s existing
  word-level matcher would fix this class of failure with no new models — keyword enrichment
  and a real embedding model are multipliers on top of that fix, not prerequisites for it.
- Only two built-in tools are currently registered (`TerminalTool`, `WebFetchTool`); others are
  present but commented out in `agent/mod.rs` (~660-672).
- `ingest_tool_registry` (added by `tool-token-ingestion`, called from
  `src-tauri/src/commands/streaming_cmd.rs:342`) already writes built-in + MCP tools into the
  store every turn, lazily — but this ingested copy is currently unused for discovery, since
  `SquireExploreTool` bypasses the store entirely for tool/tool_skill. There is no startup/
  session-start registration path today.
- MCP tools get a verbose wrapped description at registration time ("MCP tool 'X' from server
  'Y': {full desc}", `streaming_cmd.rs:279-282`) — no short description, no keywords. Under the
  simplified design this verbose full description is exactly what gets EMBEDDED (semantic
  matching does not require a short/summarized form to work); only the DISPLAY `short_desc`
  needs shortening, produced by deterministic truncation (no model needed for that).
- Token schema (`src-tauri/src/storage/squire_lancedb.rs:74-106`): `token_id`, `token_type`,
  `short_desc`, `full_desc`, `creation_turn`, `accumulated_hits`, `embedding`
  (`FixedSizeList<Float32,64>` — to become `FixedSizeList<Float32,384>` under the simplified
  design's keystone change), `endpoint` (added by `token-detail-endpoint`). A triplet store
  `squire_relationships` (subject/predicate/object) exists and is traversed when `num_hops>0`
  (added by `retrieval-fidelity`), but nothing today writes package -> resource "exposes" edges
  — the only relationship writer is model-emitted relationships (`squire.rs:~1624`). All tools
  are currently a flat `token_type="tool"`; no package/resource distinction exists yet.
- The embedding function is a toy 64-dim bag-of-words hash (`squire_lancedb.rs:54-72`), called
  at both ingest (`squire_lancedb.rs:~464`) and query time (`squire_lancedb.rs:~613`), and
  `explore_memory` filters out zero-score rows (`squire_lancedb.rs:~695`). Distance is computed
  via a manual full-scan cosine in Rust (`squire_lancedb.rs:673-690`), not a LanceDB native
  vector index. **REVISED 2026-07-03: replacing this toy hash with a real local embedding model
  is now this node's keystone/in-scope change** (previously deliberately out of scope — see
  decisions.md for the reversal and rationale). Both call sites funnel through the same
  `embed_text` function, so swapping its implementation and bumping `EMBED_DIM` is a single
  change point covering ingest and query alike. The manual-cosine-vs-native-index question is
  a separate, independent, lower-priority follow-up (see decisions.md sub-decision 2).

## Useful commands

- `cd src-tauri && cargo build && cargo build --bins && cargo build --examples && cargo test --lib`
- `cd src-tauri && cargo run --example tool_token_ingestion_e2e` — existing headless harness for
  the current (pre-this-node) ingestion path; a natural starting point/reference for a new
  harness verifying the fixed discovery path.
- See `../env.md` for the full parent-level command reference (protoc build prerequisite,
  frontend test/typecheck commands, WDIO e2e commands).
