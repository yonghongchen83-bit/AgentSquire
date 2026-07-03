# State

## Timeline

- 2026-07-03 Node created from a verified, pre-diagnosed root-cause report (tool discovery via
  `SquireExploreTool`'s live-registry contiguous-substring filter fails for multi-word
  natural-language queries; `explore_memory`'s existing word-level bag-of-words matching would
  have surfaced the failing real-session queries had tools been routed through it). Full
  problem statement, architecture, and suggested todo breakdown captured verbatim in
  `prompt.md`; durable code-location facts (file:line citations) captured in `env.md`. Not
  started yet — no code read or written this session beyond what was needed to seed the node's
  files faithfully from the verified design handed off.
- 2026-07-03 (later same day) **Design SIMPLIFIED after discussion.** The originally-planned MCP
  summarization pipeline (separate turn-0 LLM pre-pass batching descriptions into
  `[{tool, short_desc, keywords[]}]`, plus a global persistent summary cache keyed by
  `hash(raw_desc)`) is DROPPED. Reasoning surfaced in discussion: the only reason to
  summarize/keyword-extract descriptions was to make them findable under the toy lexical/
  word-level matcher; once a real local embedding model (encoder) is bundled, LanceDB does true
  semantic vector matching directly on the FULL raw description, making summarization
  unnecessary. The design pivots around one keystone change instead — replacing the toy 64-dim
  bag-of-words `embed_text` (`squire_lancedb.rs:54-72`) with a real local embedding encoder
  (e.g. `bge-small-en-v1.5` or `all-MiniLM-L6-v2`, 384-dim, via `fastembed-rs`/ONNX), bumping
  `EMBED_DIM` 64 -> 384. This single swap covers both existing call sites (ingest ~464, query
  ~613). Real-embedding-model integration, previously explicitly OUT OF SCOPE for this node, is
  now IN SCOPE as the load-bearing keystone — confirmed via a worked "make html" example (a
  relevant coding/HTML token with zero shared words with the query) showing lexical matching
  cannot succeed here even in principle, only real semantic embeddings can. All four
  originally-open decisions in `decisions.md` are now RESOLVED (three dropped/moot, one
  reversed to in-scope); two new sub-decisions (which embedding model; manual-cosine vs. native
  vector index) are recorded as OPEN. `prompt.md`, `env.md`, `decisions.md`, and `todo.json`
  revised in place to reflect the simplified design while preserving the still-accurate
  verified root-cause section (substring filter, file:line evidence, `web_fetch` example)
  unchanged. Parent `root/Squire/handoff.md`'s "What tool-token-registry is for" section
  updated to stop describing the dropped turn-0/summarization/cache approach. No code changed
  this session — this was a design-document revision only, per direct instruction; the node
  remains not-yet-started at the implementation level.

## Decisions

See `decisions.md`. All four originally-open decisions from `prompt.md`'s first draft are now
RESOLVED (MCP summarization mechanism -> NONE/dropped; generative LLM/turn-0 pre-pass ->
dropped; cache location -> moot/removed; embedding-model scope -> REVERSED, now in scope as the
keystone). Two new sub-decisions are OPEN (which embedding model: `bge-small-en-v1.5` vs.
`all-MiniLM-L6-v2`; distance computation: manual cosine vs. LanceDB native vector index). One
item from the original list (server/package-level summary token) remains open, unaffected by
the simplification.

## Risks

- The two OPEN sub-decisions (embedding model choice; manual-cosine vs. native index) should be
  confirmed before/at implementation start, though both have a stated recommendation in
  `decisions.md` and neither blocks the other — implementation can proceed with the
  recommended defaults (`bge-small-en-v1.5`, manual cosine) and revisit later if needed.
- `fastembed-rs`/ONNX is a new runtime dependency this node introduces — verify it builds
  cleanly cross-platform (this repo targets Windows primarily; confirm no native-toolchain
  surprises) before committing to it as the encoder-loading mechanism.

## Next Actions

- Read `../env.md` and `../decisions.md` (parent) for settled architecture facts and
  testing-methodology conventions this node must respect.
- Read `../tool-token-ingestion/decisions.md` in full for the existing `ingest_tool_registry`
  trigger-point/token-ID/content-shape/staleness precedent this node builds on and partially
  supersedes.
- Read the actual code in full: `src-tauri/src/agent/squire.rs` (`SquireExploreTool::execute`
  ~1032-1053, `SquireStore` trait, `ingest_tool_registry`), `src-tauri/src/storage/
  squire_lancedb.rs` (token schema, embedding hash at 54-72, ingest call ~464, query call ~613,
  manual cosine at 673-690, `explore_memory`, `squire_relationships`), `src-tauri/src/agent/
  mod.rs` (`ToolRegistry`, `ToolDefinition`, commented-out built-ins), `src-tauri/src/commands/
  streaming_cmd.rs` (ingestion call site, MCP wrapped-description construction).
- Verify baseline: `cargo build` + `cargo test --lib` from `src-tauri/`; re-confirm the live
  pass count against `../handoff.md`'s last recorded figure.
- Confirm (or overturn) the two OPEN sub-decisions in `decisions.md` before writing any
  implementation code: embedding model choice, distance-computation approach.
- Proceed through `todo.json` in order (revised for the simplified design — turn-0/LLM/cache
  items removed, embedding-swap items added).
