# Decisions

Node not yet started (implementation not begun as of this revision). This file was revised
2026-07-03 to reflect a SIMPLIFIED design agreed after discussion — see `state.md` timeline for
the revision entry, and `prompt.md`'s "SIMPLIFIED DESIGN" section for the summarized rationale.
The four originally-open decisions carried in from `prompt.md`'s first draft are now RESOLVED
per this discussion; two NEW sub-decisions are introduced and remain genuinely OPEN.

## RESOLVED — MCP summarization mechanism

**Resolution: NONE.** Dropped entirely — no turn-0 pre-pass, no in-request embedded tool call,
no summarization or keyword-extraction step of any kind for MCP tool descriptions.

**Rationale.** The original open decision (`prompt.md`'s decision 1) only ever asked *how* to
summarize MCP descriptions — the underlying assumption, never itself questioned at the time,
was that summarization was necessary at all. That assumption was wrong. The ONLY reason to
summarize or keyword-extract a verbose tool description was to make it findable under a
lexical/word-level matcher (the toy bag-of-words `embed_text` in `squire_lancedb.rs:54-72`
scores shared words, not meaning — a description has to contain the right words for a query to
match it). Once a real embedding model (encoder) is bundled, LanceDB does true semantic vector
matching on the FULL raw description directly. Semantic matching does not care whether the
description is verbose or how it's worded — it matches by meaning, not shared vocabulary — so
the entire reason to summarize disappears. Embedding the full raw MCP description is not just
adequate, it's *better* than embedding a lossy summary, since summarization can only discard
information the encoder might otherwise have used.

## RESOLVED — generative LLM / turn-0 pre-pass

**Resolution: DROPPED.** Direct consequence of the summarization-mechanism resolution above —
if no summarization is needed, no LLM pre-pass is needed either. No generative LLM is bundled
or invoked anywhere in this node's design. The only model shipped is a small local embedding
encoder (see the embedding-model-scope resolution below), which produces vectors, not text.

## RESOLVED — cache location (the summary cache)

**Resolution: MOOT / REMOVED.** The global persistent cache keyed by `hash(raw_desc)` existed
solely to avoid re-paying LLM summarization cost across conversations. With summarization
itself dropped, there is nothing to cache — embedding a description is a cheap, fully local,
deterministic, offline ONNX inference call (no network round-trip, no per-call cost worth
memoizing across conversations the way an LLM API call would be). No cache table, no
`squirecli.db` schema addition, no cache-first registration logic.

## RESOLVED — embedding-model scope (REVERSED)

**Resolution: NOW IN SCOPE — this is the keystone of the entire node.** This reverses
`prompt.md`'s original decision 4 ("real embedding model: OUT OF SCOPE here, separate future
node; keep the toy hash for now").

**Rationale for the reversal.** The original scoping treated tool-discovery routing
(register-before-explore) and embedding-model quality as separable, independently-schedulable
concerns — fix routing now, upgrade the matcher later. Working through the design in discussion
showed this separation doesn't hold once summarization is dropped: without either (a) a
summarized/keyword-enriched description under the toy lexical matcher, or (b) a real semantic
embedding, MCP tool descriptions (verbose, unpredictably worded, from third-party authors) are
not reliably matchable against natural-language queries by ANY mechanism this node would
otherwise ship. The embedding model isn't an optional quality multiplier on top of a
self-sufficient routing fix here — for the MCP half of the problem specifically, it is now the
only mechanism doing the semantic work at all, which makes it load-bearing, not a follow-up.

**Worked example confirming this (why lexical/word-level matching cannot substitute).** Query:
"make html". A relevant token might be a "coding"/HTML-related tool or skill whose description
never contains the words "make" or "html" verbatim (e.g. it discusses "generating markup",
"authoring web pages", "producing structured documents") — genuinely no shared words with the
query at all. A word-level bag-of-words matcher (the current toy `embed_text`) scores zero
overlap and cannot retrieve it, no matter how the description is phrased or truncated. Only a
real embedding model, which places "make html" and "generate markup"/"author web pages" close
together in vector space by MEANING rather than by shared tokens, can retrieve it. This is a
stronger and more general case than the original `web_fetch` root-cause example (which at least
shared literal words like "fetch"/"web"/"html" with the failing queries, just not as a
contiguous substring) — it demonstrates semantic matching is doing genuinely new work beyond
what word-level matching could ever do, confirming the embedding model is this design's core
mechanic, not a nice-to-have.

**What ships.** ONE small local embedding model (encoder only — it produces vectors, it does
not generate text), e.g. `bge-small-en-v1.5` (384-dim, ~130MB) or `all-MiniLM-L6-v2` (384-dim,
~90MB), via `fastembed-rs` (ONNX runtime, no external service/network call at inference time).
This replaces the toy bag-of-words `embed_text` (`src-tauri/src/storage/squire_lancedb.rs:
54-72`); `EMBED_DIM` bumps from 64 to 384. `embed_text` is already the single function called at
both ingest (`squire_lancedb.rs:~464`) and query time (`squire_lancedb.rs:~613`), so this is one
swap point covering both paths — no separate ingest-embedding vs. query-embedding logic to keep
in sync.

## OPEN — sub-decision 1: which embedding model

`bge-small-en-v1.5` (384-dim, ~130MB, stronger retrieval quality in general benchmarks) vs.
`all-MiniLM-L6-v2` (384-dim, ~90MB, lighter footprint, still widely used and adequate for many
retrieval tasks). Both are 384-dim, both are supported by `fastembed-rs`, so this is a
pure quality-vs-footprint tradeoff, not an architectural one — switching later only requires a
re-embed pass, not a schema change (dimension is unchanged either way).

**Recommendation:** `bge-small-en-v1.5`, unless bundle/download-footprint is confirmed to be a
hard constraint for this application's distribution model (e.g. installer size limits) — in
which case `all-MiniLM-L6-v2` is the fallback. Left OPEN for confirmation before/at
implementation start; either choice is compatible with everything else in this design.

## OPEN — sub-decision 2: distance computation (manual cosine vs. native vector index)

Keep the current manual, full-scan cosine-similarity computation in Rust
(`squire_lancedb.rs:673-690`) vs. switch to LanceDB's native vector index / ANN (approximate
nearest neighbor) search.

**Recommendation:** ship with the existing manual full-scan cosine for this node — it already
works, requires no new indexing infrastructure, and is simplest to reason about and test at the
current, small per-conversation token-store scale (this is a per-conversation `SquireStore`,
not a global corpus). Correctness of retrieval comes entirely from the encoder producing
meaningful vectors, not from which distance-computation strategy is used to rank them — a
manual scan and a native ANN index return the same nearest neighbors for a small table, just at
different speeds. Treat switching to a native vector index as a follow-up performance/
sequencing optimization once (if) table sizes grow large enough for a full scan to matter, not
a prerequisite for this node's correctness goal. Left OPEN for confirmation before/at
implementation start.

## Reaffirmed (unchanged by the simplification) — still-open item carried from the original list

- **Server/package-level summary token.** `prompt.md`'s original decision 3 (whether to also
  generate a server/package-level summary token, leaning yes) is not substantially affected by
  the simplification — if implemented, such a token would also just have its own description
  text embedded like every other token, no LLM or cache involved either way. Left open for
  resolution alongside the `tool_package`/`tool_resource` token-type work; not blocking the
  keystone embedding-model change.
