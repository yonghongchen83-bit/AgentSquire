# Prompt

Fix broken tool discovery for natural-language queries by unifying built-in and MCP tool
registration onto ONE store schema, discovered through the SAME path as every other token.
Register-before-explore.

## SIMPLIFIED DESIGN (revised 2026-07-03, after discussion — supersedes the turn-0/LLM-summarization
## approach originally described below)

The original design proposed a separate turn-0 LLM pre-pass to summarize/keyword-extract MCP
tool descriptions, plus a global summary cache. **This is dropped.** The only reason to
summarize/keyword-extract was to make verbose descriptions findable under a lexical/word-level
matcher. Once a real EMBEDDING MODEL (encoder, not a generative LLM) is bundled, LanceDB does
true semantic vector matching on the FULL raw description, so summarization-for-retrieval is
unnecessary.

**Keystone change**: bundle ONE small local embedding model (encoder), e.g. `bge-small-en-v1.5`
(384-dim, ~130MB) or `all-MiniLM-L6-v2` (384-dim, ~90MB), via `fastembed-rs` (ONNX). It ONLY
produces vectors; it does not generate text — no generative LLM is bundled. This replaces the
toy bag-of-words `embed_text` in `src-tauri/src/storage/squire_lancedb.rs:54-72` (bump
`EMBED_DIM` 64 -> 384). `embed_text` is already called at BOTH ingest
(`squire_lancedb.rs:~464`) and query time (`squire_lancedb.rs:~613`), so one swap covers both.
This is the load-bearing change; everything else in this node is scaffolding around it. See
`decisions.md` for the full resolved rationale, the worked "make html" example proving why
lexical matching cannot substitute for real embeddings, and the two remaining open
sub-decisions (which model; manual-cosine vs native vector index).

Core principles carried forward unchanged:
- Register-before-explore: BOTH built-in AND MCP tools are pre-tokenized into the store as
  tokens, same schema, same path. Non-negotiable.
- Built-ins: registered at conversation/session start, idempotent, authored `short_desc`, embed
  the full description. No LLM.
- MCP: registered at connect — embed the FULL raw description into a token; NO summarization,
  NO keyword generation, NO turn-0 request, NO summary cache. For the display `short_desc`, use
  DETERMINISTIC TRUNCATION (e.g. first sentence / ~140 chars). No model needed for display.
- explore: REMOVE the live-registry substring shortcut for tool/tool_skill
  (`src-tauri/src/agent/squire.rs:1032-1053`); route tools through the store's semantic vector
  path like every other token type.
- invoke/dispatch: KEEP `token_id -> endpoint/registry` mapping for execution (discovery moves
  to the store; execution stays registry-wired).
- relationships: ingest writes package -> exposes -> resource triplets (still in scope).
- token types: introduce `tool_package` / `tool_resource` (still in scope).

The rest of this document (below) is the ORIGINAL problem statement and design as first
captured. The verified root-cause section is still accurate and is kept as load-bearing
context. The "Architecture" and "Open decisions" sections below describe the now-superseded
turn-0/LLM/cache approach for MCP tools — see `decisions.md` for what was RESOLVED (dropped)
and what remains genuinely OPEN under the simplified design.

## Problem (verified root cause)

Squire tool discovery is broken for natural-language queries. `SquireExploreTool::execute`
(`src-tauri/src/agent/squire.rs:1032-1053`) serves `resource_type="tool"|"tool_skill"` from the
LIVE `ToolRegistry` using a **contiguous-substring filter**: `d.name.contains(ql) ||
d.description.contains(ql)` where `ql` is the ENTIRE query lowercased. LLMs issue multi-word
descriptive queries, which almost never appear as a contiguous substring, so relevant tools
don't surface even when the concept is in the description.

- Evidence: `web_fetch`'s description is "Fetch a web page and return its HTML content. Useful
  for reading documentation, checking APIs, or scraping web content." (`src-tauri/src/agent/
  mod.rs:513-515`). In a real session the model queried phrases like "web scraping fetch url
  html", "scrape website data", "fetch url web page http request" — none match as contiguous
  substrings (the description even has "scraping web", reversed from "web scraping"). Bare
  "fetch" or "scraping" would have matched; the multi-word phrases did not.
- Tools bypass the store entirely for discovery, so no keyword/semantic retrieval applies to
  them.
- Only two built-ins are registered (`TerminalTool`, `WebFetchTool`); others are commented out
  in `agent/mod.rs` (~660-672).
- Built-in tools ARE ingested into LanceDB but lazily, every turn, via `ingest_tool_registry` at
  `src-tauri/src/commands/streaming_cmd.rs:342` — and that ingested copy is unused for discovery
  because of the shortcut. There is NO startup/session-start registration.
- MCP tools get a verbose wrapped description ("MCP tool 'X' from server 'Y': {full desc}",
  `streaming_cmd.rs:279-282`); no short description, no keywords.
- Token schema (`src-tauri/src/storage/squire_lancedb.rs:74-106`): `token_id`, `token_type`,
  `short_desc`, `full_desc`, `creation_turn`, `accumulated_hits`, `embedding`
  (`FixedSizeList<Float32,64>`), `endpoint`. A triplet store `squire_relationships` (subject/
  predicate/object) exists and is traversed when `num_hops>0`, but nothing creates
  package -> resource "exposes" edges; the only relationship writer is model-emitted
  relationships (`squire.rs:~1624`). All tools are a flat `token_type="tool"`; no package/
  resource distinction.
- Embedding is a toy 64-dim bag-of-words hash (`squire_lancedb.rs:54-72`), and `explore_memory`
  filters out zero-score rows (`squire_lancedb.rs:~695`).

## Key insight / rationale

`explore_memory` matches at the WORD level (bag-of-words), not contiguous-substring.
`web_fetch`'s tokenized description shares words (fetch/web/html/scraping) with the failing
queries, so **simply routing tool discovery through the store would have fixed the reported
failure with NO new models.** Keyword enrichment and a real embedding model are multipliers,
not prerequisites — this lowers the risk of the core change.

## Goal — unified tool-token registration

Built-in and MCP tools converge on ONE store schema and are discovered through the SAME path as
all other tokens. Register-before-explore. This folds four problems into one change: empty/
failed tool discovery, MCP short-description gap, package/resource granularity, and the
built-in <-> MCP asymmetry.

## Architecture (ORIGINAL — turn-0/LLM/cache approach; SUPERSEDED, see decisions.md)

- **Built-ins**: registered at conversation/session start (the store is per-conversation),
  idempotent, with authored `short_desc` + keyword tags. No LLM. (Still accurate under the
  simplified design.)
- ~~**MCP/external**: registered at connect via a SEPARATE turn-0 pre-pass LLM request (NOT an
  in-conversation tool call) that BATCHES all of a server's tools into one structured request
  returning `[{tool, short_desc, keywords[]}]`. Results cached in a GLOBAL persistent cache
  keyed by `hash(raw_desc)` (e.g. a table in `squirecli.db`), so summaries are reused across
  conversations; registration is cache-first and only genuinely-new descriptions hit the LLM.
  Async with truncation fallback so turn 1 is never blocked; cached so only the first
  conversation to touch a server ever waits.~~ DROPPED — see decisions.md. Under the simplified
  design, MCP tools are registered at connect by embedding the FULL raw description directly
  (no LLM pre-pass, no cache); the display `short_desc` is produced by deterministic truncation.
- **explore**: REMOVE the tool/tool_skill live-registry substring shortcut
  (`squire.rs:1032-1053`); route tools through `explore_memory` using dedicated token types
  (e.g. `tool_package` / `tool_resource`), so SEMANTIC VECTOR matching applies (word-level
  bag-of-words matching is itself superseded by the real embedding model — see decisions.md).
  Keep `explore_memory`'s existing substring boost.
- **invoke/dispatch**: KEEP the `token_id -> endpoint/registry` mapping for actual execution.
  Discovery moves to the store; execution stays wired to the registry. (Keep this separation
  explicit.) Still accurate under the simplified design.
- **relationships**: ingest writes package -> exposes -> resource triplets so a package token
  can be found first and drilled into via hops. Still accurate under the simplified design.

## Open decisions (ORIGINAL list — see decisions.md for current resolution status)

1. ~~MCP summarization mechanism: separate turn-0 pre-pass (RECOMMENDED) vs. in-request embedded
   tool call.~~ RESOLVED as NONE (dropped) — see decisions.md.
2. ~~Cache location: global persistent (RECOMMENDED) vs. per-conversation.~~ RESOLVED as moot/
   removed — see decisions.md.
3. Whether to also generate a server/package-level summary token (lean yes). Still open; not
   substantially affected by the simplification (a package-level token, if added, would also
   just embed its own description text like everything else).
4. ~~Real embedding model: OUT OF SCOPE here (separate future node); keep the toy hash for
   now.~~ RESOLVED — REVERSED. Now IN SCOPE as the keystone of this node. See decisions.md for
   the full rationale (semantic matching is the design's core mechanic).

Two NEW open sub-decisions introduced by the simplified design (see decisions.md):
5. Which embedding model: `bge-small-en-v1.5` (~130MB, stronger) vs `all-MiniLM-L6-v2` (~90MB,
   lighter).
6. Distance computation: keep the current manual full-scan cosine in Rust
   (`squire_lancedb.rs:673-690`) vs switch to LanceDB's native vector index / ANN search.

## Deliverables

- Read `../env.md` and `../decisions.md` for the parent epic's settled architecture facts and
  testing-methodology conventions this node must respect (two-backend `SquireStore` parity, the
  proportionate-verification tiering system, the "no new trait method unless truly necessary"
  discipline established by `tool-token-ingestion`).
- Read `../tool-token-ingestion/decisions.md` in full — the existing `ingest_tool_registry`
  trigger point, token-ID scheme, content shape, and no-active-cleanup staleness decision are
  all direct precedent/prior art this node builds on and partially supersedes (registration
  moves from "lazy, every turn, unused for discovery" to "at session/connect start, authoritative
  for discovery").
- Read the actual code in full before implementing: `src-tauri/src/agent/squire.rs`
  (`SquireExploreTool::execute`'s tool/tool_skill branch at ~1032-1053, `SquireStore` trait,
  `ingest_tool_registry`), `src-tauri/src/storage/squire_lancedb.rs` (token schema, embedding
  hash, `explore_memory`'s substring boost and zero-score filtering, `squire_relationships`
  table/traversal), `src-tauri/src/agent/mod.rs` (`ToolRegistry`, `ToolDefinition`, the
  commented-out built-ins ~660-672), `src-tauri/src/commands/streaming_cmd.rs` (the per-turn
  `ingest_tool_registry` call site at ~342, the MCP tool wrapped-description construction at
  ~279-282).
- Verify baseline first: `cargo build` + `cargo test --lib` from `src-tauri/` (confirm current
  pass count against `../handoff.md`'s last recorded figure — re-confirm the live number).
- Design and document in `decisions.md`, before writing code, resolving the open decisions
  (see decisions.md's current, simplified-design list) with your own judgment and textual/
  code-level justification (matching every sibling node's practice of writing the decision and
  its rationale before implementing, not after).
- Implement per the suggested todo breakdown below (SIMPLIFIED — see decisions.md and
  todo.json for the current, authoritative list; the numbered list immediately below is kept
  for historical reference only and is superseded by todo.json).
- Add real unit tests in the same style as sibling nodes' suites (two-backend parity for every
  `SquireStore`-touching change).
- Verify manually/e2e: confirm the recorded failing queries ("web scraping fetch url html",
  "scrape website data", "fetch url web page http request", "make html") now surface the right
  tokens (`web_fetch`, a coding/HTML token) via the store's SEMANTIC VECTOR path — a headless
  integration harness in the style of `src-tauri/examples/tool_token_ingestion_e2e.rs` is the
  likely right verification tier for the backend discovery fix; a repo-wide frontend grep
  should be done first to confirm this remains backend-only, matching the epic's established
  tiering discipline.
- Update `../state.md`'s "Child Nodes" list and `../handoff.md` to reflect this node's work and
  final status.

## Suggested todo breakdown (ORIGINAL, turn-0/LLM-era list — SUPERSEDED; see todo.json for the
## current, authoritative, simplified-design todo list)

1. Add `short_description` + `keywords` fields to `ToolDefinition`; author them for the
   built-in tools.
2. Introduce `tool_package` / `tool_resource` token types (or equivalent) in the store schema/
   type mapping.
3. Register built-ins at conversation/session start (idempotent), replacing/supplementing the
   per-turn lazy ingest.
4. Remove the live-registry substring shortcut in `SquireExploreTool`; route tool/tool_skill
   through `explore_memory`.
5. Verify (test) that the recorded failing queries now surface `web_fetch` via the store path.
6. ~~MCP: implement the batched turn-0 pre-pass LLM summarization request (structured output:
   `short_desc` + `keywords`).~~ DROPPED.
7. ~~MCP: implement the global persistent summary cache keyed by `hash(raw_desc)`, cache-first
   registration, async + truncation fallback.~~ DROPPED.
8. Write package -> resource "exposes" relationship triplets during ingestion.
9. Keep invoke/dispatch resolving `token_id -> endpoint` for execution; confirm still works
   after discovery moves to the store.
10. Update the Squire spec/system-prompt docs if the tool-discovery contract changes.

## Out of scope (do NOT fix here)

- ~~Real embedding model integration (separate future node; keep the toy 64-dim bag-of-words
  hash for now).~~ REVERSED — a real local embedding model is now IN SCOPE as this node's
  keystone change. See decisions.md.
- A generative LLM / turn-0 pre-pass for MCP tool summarization, and any associated global
  summary cache — DROPPED under the simplified design (see decisions.md); do not reintroduce.
- Unrelated Squire deferrals: `retrieval-fidelity/todo.json` rf-13 (already resolved by
  `hit-count-fidelity`; not this node's concern either way), the nested-`§!`-citation residual
  `hit-count-fidelity`/`memory-alias-fix` deliberately left as a permanent simplification per
  direct user instruction — do not reopen it.
- Any frontend/UI work, unless a genuinely new user-facing surface turns out to be required
  (not expected — this is a discovery/registration backend change, matching the category of
  `tool-token-ingestion`'s own scope).
- Switching from manual cosine to LanceDB's native vector index/ANN search — recommended as a
  follow-up, not required for this node to ship (see decisions.md sub-decision 2).
