# State

## Timeline

- 2026-07-03 Node created from a direct task specification (debug/observability facilities for
  the Squire semantic loop, motivated by the `testing` node's need for real instrumentation to
  assert against instead of eyeballing `provider-wire.log`). Before seeding this node's files,
  verified the task's cited code facts against the real repository: confirmed the real embedding
  model (`fastembed BGESmallENV15`, 384-dim, `src-tauri/src/storage/embedding.rs`) is genuinely
  live with a fallback bag-of-words path; confirmed `LanceDbSquireStore::explore_memory`'s
  scoring loop (`src-tauri/src/storage/squire_lancedb.rs:592-706`) computes `score = cosine +
  substr_boost` and discards near-misses at the `score<=0.0` cut (line 704); confirmed the
  `provider-wire.log`/`verbose` gating pattern in `src-tauri/src/llm/openai.rs`; confirmed
  function/line locations in `src-tauri/src/agent/squire.rs` for `SquireExploreTool`,
  `SquireTokenToDetailTool`, `SquireInvokeTool`, `build_turn_input`, `finalize_turn`,
  `insert_relationship`. `prompt.md`/`env.md`/`decisions.md`/`todo.json` seeded from this
  verified design. Not started yet — no implementation code read or written beyond what was
  needed to confirm the cited facts.
- 2026-07-03 (same session) **Write-back to parent.** Found `../env.md`'s existing "Vector
  search uses a deterministic hash-based embedding, not a real embedding model" claim is now
  stale — `tool-token-registry` has since landed the real `fastembed BGESmallENV15` swap this
  node's own embedding-path-tagging design depends on. Corrected `../env.md` in place (see
  "Conflicts" below) rather than leaving the drift for a future node to rediscover, per the
  write-back obligation. Registered this node in `../state.md`'s Child Nodes list (item 20) and
  `../handoff.md`'s status table, matching how `testing` (item 18) and `tool-token-registry`
  (item 19) were registered.
- 2026-07-03 (docs-only update, same day, no code touched) **RESOLVED decision: at-a-glance
  per-turn digest, "Digest surface = BOTH, log first."** Recorded in `decisions.md` (new
  "RESOLVED — at-a-glance per-turn digest surface" section). `squire-trace.log` (JSONL) remains
  the sole machine-readable source of truth and is unchanged by this decision. A new
  human-readable `squire-turn.log` will be produced by a **projector** that reads the existing
  JSONL trace and groups it by turn into a readable block (user request; explored search terms
  tagged semantic-vs-substring with top results/near-misses; tokens DEFINED; tokens PRESERVED;
  RELATIONSHIPS linked; tools INVOKED) — this is explicitly a projection, not a new/extra
  emission point. An in-app React/Tauri debug panel rendering the same digest live is a deliberate
  follow-up, sequenced after the log projector. Added `obs-11` (projector -> `squire-turn.log`)
  and `obs-12` (in-app debug panel, follow-up on obs-11) to `todo.json`, both OPEN, continuing the
  existing obs-N sequence. **Sequencing/dependency note:** obs-11 cannot be implemented (or even
  meaningfully started) until `obs-3` (RETRIEVAL TRACE event — supplies the explored/search-term
  data), `obs-4` (TOKEN LIFECYCLE events — supplies defined/preserved/relationships), and `obs-5`
  (FUNNEL events — supplies invoked) all exist and are emitting into `squire-trace.log`; the
  digest has no data to project before then. obs-12 additionally depends on obs-11 being done and
  trustworthy, plus some mechanism (new Tauri command/event) exposing trace/digest data to the
  frontend — that plumbing does not exist yet and is itself part of obs-12's scope, not a
  precondition met elsewhere. No code or `.AiControl/.current` was touched in this update — docs
  only (`decisions.md`, `todo.json`, this `state.md` entry).

## Conflicts

- **Parent `env.md` vs. verified current code state.** `../env.md`'s "Stable facts —
  architecture" section stated (as of `memory-alias-fix`, before this node) that "Vector search
  uses a deterministic hash-based embedding, not a real embedding model — there is no
  embedding-model provider in this codebase. Documented as an explicit, swappable placeholder
  (`squire-storage`), not a bug." This was accurate when written but is now contradicted by
  `tool-token-registry`'s completed real-embedding-model swap (confirmed directly against
  `src-tauri/src/storage/embedding.rs` this session: `fastembed BGESmallENV15`, 384-dim, is the
  real, live default path, with a bag-of-words fallback only for offline/init-failure). Resolved
  by correcting `../env.md`'s bullet in place to describe the current, real state plus the
  fallback, rather than treating this node's own files as the only place this fact is recorded
  (per the "write-back obligation, upward" rule — a durable, epic-wide fact belongs in the
  parent, not just a leaf node). No other conflicts identified between this node's scope and its
  parent chain's settled assumptions.

## Decisions

See `decisions.md`. Three open decisions recorded, each with a stated recommendation: debug-flag
mechanism (recommend a dedicated `SQUIRE_TRACE`-style flag, not reusing `verbose_logging`),
trace format (recommend JSONL), query-probe surface (recommend a Tauri command, with an
acceptable fallback sequencing of building a headless example harness first if new IPC plumbing
would otherwise block earlier, higher-value instrumentation work).

## Risks

- `explore_memory`'s near-miss capture will likely require either a return-shape change or a
  trace-sink callback threaded through both `SquireStore` backends — whichever shape is chosen
  must be implemented and tested identically in `InMemorySquireStore` and `LanceDbSquireStore`
  per the epic's non-negotiable two-backend parity convention (see `decisions.md`'s "Reaffirmed
  context" section).
- Timing instrumentation (embed-inference latency, model init duration) touches
  `src-tauri/src/storage/embedding.rs`, a file `tool-token-registry` may still be actively
  changing (model choice / distance-computation follow-ups are recorded as open in that node's
  own `decisions.md`) — re-check that node's current status before instrumenting, to avoid
  conflicting in-flight edits to the same file.
- This node is a stated DEPENDENCY of `testing`. If `testing` starts before this node completes,
  it will still only have `provider-wire.log` to work with for anything touching semantic
  retrieval correctness — flag this sequencing risk to whoever picks up either node next if both
  end up in flight concurrently.

## Next Actions

- Read `../env.md` (as corrected by this node) and `../decisions.md` for settled architecture
  facts and testing-methodology conventions.
- Read `../tool-token-registry/env.md`/`decisions.md` in full for the real-embedding-model swap
  this node's embedding-path tagging depends on; confirm that node's current implementation
  status before touching `embedding.rs`.
- Read the actual code in full: `src-tauri/src/agent/squire.rs` (`SquireExploreTool`,
  `SquireTokenToDetailTool`, `SquireInvokeTool`, `build_turn_input`, `finalize_turn`,
  `insert_relationship`), `src-tauri/src/storage/squire_lancedb.rs` (`explore_memory`'s scoring
  loop and near-miss cut), `src-tauri/src/storage/embedding.rs` (real/fallback embedding
  provenance), `src-tauri/src/llm/openai.rs` (the `verbose`/wire-log gating pattern to mirror),
  `src-tauri/src/state/config.rs` (`AppConfig`, where a new trace flag would live).
- Verify baseline: `cargo build` + `cargo test --lib` from `src-tauri/`; re-confirm the live pass
  count against `../handoff.md`'s last recorded figure.
- Confirm (or overturn) the three OPEN decisions in `decisions.md` before writing implementation
  code.
- Proceed through `todo.json` in order (obs-1..obs-10).
