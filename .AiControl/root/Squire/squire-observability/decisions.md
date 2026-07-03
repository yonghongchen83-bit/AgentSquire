# Decisions

Node not yet started (implementation not begun as of this revision). This file records the
three open decisions `prompt.md` flagged, with a recommendation for each — following sibling
nodes' convention of writing the decision and its rationale before implementing, not after.
Populate further as real design/scope judgment calls are made during implementation.

## OPEN — debug-flag mechanism

Reuse the existing `verbose_logging`/wire-log gating (`AppConfig::verbose_logging`,
`src-tauri/src/state/config.rs:38`, the same flag `provider-wire.log` checks) vs. a dedicated
`SQUIRE_TRACE` env/config flag.

**Recommendation: a dedicated flag.** Wire logging (`provider-wire.log`) and semantic-loop
tracing (`squire-trace.log`) answer different questions — "what did the LLM API exchange look
like" vs. "what did the retrieval/scoring/lifecycle machinery actually do internally." A future
test author debugging retrieval correctness shouldn't have to also turn on (and wade through)
full request/response wire logging, and someone debugging an API/provider issue shouldn't need
semantic-loop internals cluttering their log. A dedicated flag also makes the trace safe to
leave on in a test harness without bloating `provider-wire.log`'s own size/verbosity contract.
Left OPEN for confirmation before/at implementation start; either choice is mechanically simple
(one new boolean field + config key, following `verbose_logging`'s exact existing shape).

## OPEN — trace format

JSONL (one self-contained `{turn, tool_call_id?, event, payload, ts}` object per line) vs.
pretty/human-readable text.

**Recommendation: JSONL.** The stated purpose is enabling `testing` (and any future test) to
assert against trace output programmatically — "correctness of semantic retrieval / token
accretion cannot be verified by eyeballing `provider-wire.log`" is the node's own motivating
complaint, and a pretty-text log has exactly the same eyeballing problem, just with more
detail. JSONL is trivially parseable line-by-line (no multi-line record boundaries to get
wrong), greppable for a quick human look when needed, and matches how the query-probe command's
own output (dev-facing structured scores) should probably look too, for consistency. Left OPEN
for confirmation before/at implementation start.

## OPEN — query-probe surface

A Tauri command (UI-invokable, usable from a future dev panel) vs. a CLI/test-only entry point
(e.g. a new `cargo run --example` headless harness, matching this epic's existing precedent:
`ask_user_e2e.rs`, `tool_token_ingestion_e2e.rs`, `user_input_chunking_e2e.rs`,
`raw_partition_storage_e2e.rs`).

**Recommendation: a Tauri command**, per the task's own stated preference ("so it's usable from
a dev panel later"). This is the one piece of this node with genuine new user-facing-surface
potential (even if no dev panel UI is built yet, per `prompt.md`'s explicit non-goal) — a
`#[tauri::command]` costs little more than a plain function and keeps the door open for a future
dev panel without requiring a second implementation later. If, during implementation, wiring a
new command turns out to need disproportionate new plumbing (e.g. a new IPC response type, new
frontend types) before the retrieval-trace and lifecycle events themselves are even built, it is
acceptable to build a headless example harness FIRST as an interim verification tool (matching
this epic's own "read the actual code before assuming a 'full' implementation is
disproportionate" practice — `../decisions.md`'s "Proportionality" section) and add the Tauri
command as a fast-follow within the same node, documenting that sequencing choice here rather
than silently dropping the command. Left OPEN for final confirmation at implementation start.

## RESOLVED — at-a-glance per-turn digest surface

**Decision: Digest surface = BOTH, log first.** The JSONL `squire-trace.log` remains the
machine-readable source of truth (unchanged by this decision — nothing about the RETRIEVAL
TRACE / TOKEN LIFECYCLE / FUNNEL / SNAPSHOT / TIMING event schema above changes). On top of it,
ship a second, human-readable artifact: a per-turn digest that is a **projection** over the same
JSONL trace data, not a new/extra emission point and not a parallel source of truth. Two
surfaces, sequenced:

1. **First — `squire-turn.log` (a log file).** A projector reads `squire-trace.log`, groups
   events by `turn`, and writes one readable block per turn to a new `squire-turn.log`,
   summarizing: the user's request for that turn, the exploration search terms used (with
   semantic-vs-substring tagging per hit and top results/near-misses), tokens DEFINED, tokens
   PRESERVED, RELATIONSHIPS created, and tools INVOKED. Example shape (user-approved):

   ```
   ━━ Turn 3 ━━  user: "find officeworks wifi and make html"
     explored:
       "web scraping fetch url html"  [tool·substring]  → web_fetch          (+2 near-miss)
       "make html"                    [skill·semantic]   → coding 0.71, html_gen 0.68
     defined:   USR_T3_001 "officeworks wifi" · CONCEPT_HTML "html output page"
     preserved: web_fetch · CONCEPT_HTML
     linked:    CONCEPT_HTML --requires--> coding
     invoked:   web_fetch({url:…}) → ok
   ```

2. **Follow-up — in-app Squire debug panel (React/Tauri).** Once the log projector exists and is
   trustworthy, build a debug panel in the app that renders the same per-turn digest live, reading
   from the same underlying trace data (not a third independent format) — e.g. via a Tauri command
   or event that exposes trace/digest data to the frontend.

**Rationale.** The log-first sequencing lets the digest be validated against real trace data
(and used immediately for manual debugging / test-authoring) without waiting on any frontend
plumbing (new Tauri command, new IPC types, React component work) — consistent with this node's
existing "headless-harness-first-if-blocked" precedent for the query-probe surface. Because the
digest is explicitly a projection, not a new emission point, it has a hard dependency on the
underlying trace events actually existing and being complete: it cannot show tokens
DEFINED/PRESERVED, RELATIONSHIPS, or tools INVOKED before the TOKEN LIFECYCLE (obs-4) and FUNNEL
(obs-5) events are implemented, and it cannot show exploration search terms/hits before the
RETRIEVAL TRACE event (obs-3) is implemented. See `todo.json` (obs-11, obs-12) for the concrete
work items and `state.md`'s timeline entry for the sequencing note.

## Reaffirmed context (not itself an open decision, recorded here for continuity)

- The real-embedding-model swap this node's embedding-path tagging depends on (`fastembed
  BGESmallENV15`, 384-dim) is `tool-token-registry`'s scope, already landed per `../env.md`'s
  "Confirmed current state" — this node consumes that fact (tag real-vs-fallback per trace
  event) rather than re-deciding anything about the model itself.
- This node's own near-miss-capture instrumentation of `explore_memory` will very likely need to
  either (a) return additional data (near-miss candidates + score breakdown) alongside the
  existing `Vec<TokenSummary>` result, or (b) accept a trace-sink callback/handle so scoring can
  emit events inline without changing the return type. Which of these is chosen is an
  implementation-time decision, not listed as a separate top-level "open decision" here since it
  is more of a mechanical API-shape question than a judgment call with competing tradeoffs — but
  whichever is chosen, per `../env.md`'s non-negotiable convention, it must be implemented and
  tested identically in both `InMemorySquireStore` and `LanceDbSquireStore`.
