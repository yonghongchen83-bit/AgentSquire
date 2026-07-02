# Decisions

## Scope framing: two independent features sharing one `explore_memory` call site

Graph traversal and hit-count scoring are functionally independent (traversal expands the
*candidate set*, scoring changes the *ordering* of whatever set is being returned), but both
land inside the same function (`SquireStore::explore_memory`) in both backends, and the spec
itself couples them at the seams: §6.1 says "Results are ordered by score descending. Ties
broken by effective_priority," and §7.1 says a well-connected concept token is a retrieval hub
reached via `num_hops`. Implementing them together in one pass avoids touching
`explore_memory`'s signature/call sites twice. Each is still documented and tested as its own
concern below.

## `effective_priority` formula: exactly the spec's equation, not a reinterpretation

Spec §3.3 fully pins down the formula — this is not actually "under-specified" the way the task
description guessed it might be:

```
effective_priority = accumulated_hits - (current_turn - creation_turn)
```

Implemented literally as signed 64-bit arithmetic (`i64`) since the result is explicitly allowed
to go negative ("a never-referenced token drifts negative"). `current_turn` for this computation
is the *session's* current turn counter (already tracked per-session via
`SquireStore::current_turn`/`increment_turn`) — `explore_memory` is not itself session-scoped
(no `session_id` parameter in the trait today), so a `current_turn: u64` parameter is added to
`explore_memory`'s signature, threaded from the one call site that has a session in scope
(`SquireContextAdapter::build_turn_input`) and from `SquireExploreTool::execute` (which needs a
session_id added to its args to know which session's turn counter to read — see below).

Rejected alternative: computing `effective_priority` using a *global* turn counter instead of
per-session. Rejected because `current_turn`/`increment_turn` are already per-session in this
codebase (each session has its own turn counter, consistent with each session having its own
preserve-list and conversation), and a token's `creation_turn` is meaningful only relative to the
session that created it in the non-persistent `InMemorySquireStore` test-double model. (In the
real `LanceDbSquireStore`, tokens are actually process-global — not session-scoped — but turn
numbers are still tracked per session, so "current turn" for priority purposes is defined as the
requesting session's own turn count. This matches how `build_turn_input` already calls
`explore_memory` once per turn, on behalf of one session.)

## `SquireExploreTool` needs a `session_id` — added as a required constructor field, not a tool argument

`effective_priority` needs "current turn," which is per-session. `SquireExploreTool` (the model-
facing `explore()` tool) is constructed once per turn in `send_message_impl` alongside
`SquireTokenToDetailTool`/`SquireInvokeTool` — added `session_id: SessionId` as a new field on
`SquireExploreTool`, populated from the same `session.session.id` already in scope at tool-
registry construction time, rather than asking the model to pass a `session_id` argument (the
model has no legitimate reason to know or supply its own session id — that would leak an
orchestration-internal concept into the protocol surface Q5 deliberately kept narrow).

## Hit-count field: `accumulated_hits: u64` on the stored token record, exposed on `TokenSummary`

Added `accumulated_hits: u64` to:
- `InMemorySquireStore`'s internal `StoredToken` struct,
- `LanceDbSquireStore`'s `squire_tokens` Arrow schema (new `UInt64` column, matching the existing
  `creation_turn` column's type/nullability pattern) and its `StoredTokenRow` read struct,
- `TokenSummary` (the type both `explore_memory` and `preserved_tokens` return to callers), as a
  new field so the model-facing JSON the tool returns can, if useful later, show the hit count —
  and so tests can assert on it directly without a second store round-trip.

`TokenSummary` gaining a field is additive/non-breaking for JSON consumers (`serde` struct,
existing fields unchanged, new field serializes as an additional key) — no existing test asserts
on the exact key-set of a serialized `TokenSummary`, only specific field values, so this doesn't
break `squire-adapter`'s or `squire-storage`'s existing coverage.

Schema-migration note for `LanceDbSquireStore`: adding a new required, non-nullable column to an
existing Arrow schema is a breaking change for any *already-persisted* LanceDB table from a prior
run (old rows don't have the column). Since there is no shipped migration story anywhere in this
codebase yet (Squire storage is pre-release, no production data exists to migrate), this is
accepted as a schema-version bump with no migration path, consistent with how `squire-storage`
and `rejection-ux` both added new tables/columns without migration handling for the same reason
(pre-release, dev-machine-only LanceDB directories). Documented here rather than silently
assumed.

## Hit-count increment events wired: 2 of the spec's 4, with rationale for the other 2

Spec §3.3's table lists four hit-count-increment events. Wired two, deferred two with reasoning
(not silently dropped):

1. **"Token in preserve list loaded at turn open" (+1)** — **wired**. `preserved_tokens()` is the
   natural, already-existing call site; incrementing here is a pure addition inside the existing
   store method, no new call site needed in the adapter.
2. **"Token listed in new_tokens at turn close" (+1)** — **wired**, but reinterpreted slightly:
   spec literally says increment for *every* `new_tokens` entry "regardless" (per §9.4 step 5,
   even for brand-new tokens never seen before). Implemented as: `upsert_token` increments
   `accumulated_hits` by 1 on *every* call (both the create path and the update path), matching
   §9.4 step 5's literal "increment by 1 regardless" and also §5.2's "If the token already
   exists... `accumulated_hits` increments" for the §^-span-reuse case — one code path serves
   both spec references since `finalize_turn` already funnels both `new_tokens` entries and
   §^-created tokens through the same `upsert_token` call.
3. **"Token appears in explore() results that AI acts on" (+1)** — **deferred, not wired**. This
   requires knowing which of the returned candidates the AI *subsequently* acted on (i.e. called
   `token_to_detail` on, or referenced via `§!` in its response) — `explore()` itself cannot know
   this at call time, since "acting on" happens later in the same turn or not at all. The spec's
   own wording under §6.1 ties this to the *same* mechanism as event 4 below ("Hit count: Squire
   increments accumulated_hits for every token in the returned list that the AI subsequently
   acts on (calls token_to_detail or references in output)") — i.e. this event and the §5.1 `§!`
   scan-on-load event are really the same underlying mechanism restated in two places. Given
   that, "acts on" is operationalized as **event 4 below** (real §! reference scanning) plus a
   **direct hit-increment on every `token_to_detail` call**, which together cover both restated
   forms of this event without needing to track "was this ID present in a previous explore()
   result set," a piece of cross-call state this store has no mechanism for today. Documented as
   a deliberate reinterpretation, not silently dropped — `token_to_detail`'s hit-count increment
   (spec §6.2: "accumulated_hits increments by 1 on each call") is wired for exactly this reason.
4. **"§! reference found in a chunk loaded into context" (+1)** — **wired, via `token_to_detail`
   proxy, not full context-scan**. The literal event ("any chunk containing a §!TokenID reference
   is loaded into context") would require scanning *every* piece of context assembled anywhere
   (prefetched short_descs, full_desc bodies returned by `token_to_detail`, tool results) for
   embedded `§!` markers — a materially larger feature (a context-composition audit pass) that
   doesn't exist as a concept in this codebase's `build_turn_input`/tool-execution flow today.
   Reinterpreted narrowly: `SquireTokenToDetailTool::execute` (the actual point where a token's
   content enters context) increments the target token's hit via `record_hit`, which is both a
   direct implementation of spec §6.2's `token_to_detail` hit rule and a reasonable proxy for "a
   chunk was loaded into context" for the store-backed (concept/referential) token path — it does
   not cover the case of a `§!` reference appearing *inside* a full_desc body that's itself loaded
   (a chunk citing another chunk), which is deferred as a smaller residual gap, noted in
   `state.md`'s Risks/Next Actions rather than silently treated as complete.

`token_to_detail`'s hit-count increment: added a `record_hit(token_id: &str)` method to
`SquireStore` (rather than overloading `token_detail` itself to mutate state on a read call,
keeping read/write concerns separated at the trait level) and called it from both
`SquireTokenToDetailTool::execute` and `preserved_tokens()`'s internal bootstrap-load path.

## Ranking integration: `effective_priority` as a secondary sort key, not primary

Spec §6.1: "Results are ordered by score descending. Ties broken by effective_priority." Kept
`score` (cosine-similarity + substring boost in `LanceDbSquireStore`; flat 1.0/traversal-decayed
value in `InMemorySquireStore`) as the primary sort key exactly as specified, and used
`effective_priority` as a secondary key for ties. Because raw floating-point cosine scores rarely
tie exactly, added a small epsilon-bucket comparison (scores within `1e-6` are treated as tied)
so `effective_priority` actually gets a chance to matter in the realistic case of near-identical
scores, rather than being dead code that only fires on bit-exact float equality.

## Graph traversal: BFS over `squire_relationships`, hop-distance-decayed score, capped by `max_results`

`num_hops` (spec §4.2/§6.1/§7.1): given the set of tokens that directly matched the vector/type
filter (hop 0), BFS-walk the relationship triplet store outward, treating `subject`/`object` as
an undirected adjacency for traversal purposes (a relationship `A --predicate--> B` makes `B`
reachable from `A` and, symmetrically, `A` discoverable when starting from `B` — spec §7.3's
worked example explicitly traverses in whatever direction reaches connected memory, not strictly
subject-to-object; the Squire's job per §4.2 is graph *connectivity*, not directed-edge-only
reachability, and "no vocabulary enforcement" on predicates makes a directionality distinction
between e.g. `instanceOf` and `relatedTo` not meaningful for reachability purposes). Traversal
depth-limited to `num_hops` (0 = no expansion, matching "vector search only" exactly).

Newly-discovered tokens at hop `h` (`1 <= h <= num_hops`) are added to the result set if not
already present (a token found at multiple hop-distances keeps its shortest-hop-distance record)
with:
- `score`: no query-similarity score of their own (they weren't matched by the query at all —
  spec §7.3's whole point is that graph-connected tokens "might not score well on raw vector
  similarity alone"), so a decayed placeholder score is assigned: `base_score * 0.5^hop_distance`,
  where `base_score` is the *originating* hop-0 match's score that led to this token's discovery
  (or the highest such score if reachable from multiple hop-0 matches). This keeps traversal-
  found tokens ranked below direct hits at the same hop-distance-normalized quality, decaying
  further for tokens further from any direct match, while still being visible/orderable rather
  than a flat constant.
- `via_token_id` / `hop_distance`: new fields on `TokenSummary` (see below) carrying the
  provenance the task asks for ("hop-distance/provenance metadata that makes sense for the
  caller to understand why a token was included") — `hop_distance: 0` for direct matches,
  `hop_distance: N` for traversal-discovered tokens, and `via_token_id: Option<String>` naming
  one concrete direct-match token that led to this token's discovery (the nearest one found
  during BFS, i.e. the parent in the BFS tree), `None` for hop-0 entries.

`max_results` still applies as the final cap on the combined (direct + traversal-expanded) result
list, applied *after* sorting by score/effective_priority — so traversal-discovered tokens can
still be trimmed away by a tight `max_results` budget rather than always guaranteeing hop-1
tokens survive regardless of count, consistent with §6.1's `max_results` description ("caps the
number of results returned") applying to the tool's actual output, not just the vector-search
stage.

Rejected alternative: only traversing from the *single best-scoring* hop-0 match rather than the
full matched set. Rejected because §7.2's retrieval-path diagram shows traversal happening on the
*candidate list* from vector search (plural), and §7.3's worked example ("A query that lands on
CONCEPT_FishingLocation can reach...") frames each matched token as its own traversal root — a
query could plausibly land near-simultaneously on two different concept tokens, and both are
legitimate expansion roots.

Rejected alternative: directed-only traversal (subject → object only). Considered because RDF
triplets are conventionally directional and the schema's field names (`subject`/`predicate`/
`object`) suggest direction matters. Rejected for the reason given above (§7.3's framing plus "no
vocabulary enforcement" on predicates) — revisit if a future node finds a concrete case where
undirected traversal produces surprising/unwanted results; nothing in the current spec text
requires strict directionality for `explore()`'s discovery purpose (as opposed to, hypothetically,
a future "list what A requires" style directed query, which isn't part of `explore()`'s contract
today).

`InMemorySquireStore` and `LanceDbSquireStore` implement the identical BFS algorithm structure
(same hop-distance/decay math) against their respective relationship storage — the in-memory
store BFS-walks its `Vec<Relationship>` in a loop; the LanceDB store loads the full
`squire_relationships` table into memory once per `explore_memory` call (consistent with how
`explore_memory` already loads the full `squire_tokens` table per call — no pagination/indexing
exists in this store for either table today, a pre-existing characteristic of this module, not a
new limitation introduced here) and BFS-walks the resulting adjacency map the same way.

## `TokenSummary` gains three new fields: `accumulated_hits`, `hop_distance`, `via_token_id`

All three are additive (new struct fields, `#[serde(default)]` not needed on the Rust side since
these are always populated by the producing code, but harmless either way for any external
deserializer). `hop_distance: u32` (0 for direct matches), `via_token_id: Option<String>` (`None`
for direct matches). No existing test constructs a `TokenSummary` literal that would need every
field named (checked — all existing construction sites in `squire.rs`/`squire_lancedb.rs` are
within this node's own edits), so this is a mechanical, non-breaking extension.

## Not addressed here: ss-9 (real tool-token ingestion)

Per the task's explicit note, checked whether ss-9 is actually entangled with this node's two
features. It is not: ss-9 is about giving `invoke()`/`explore(resource_type="tool_skill")` a real
persisted-token backing for MCP/local tools (a *content* gap — tool tokens don't exist as
`SquireStore` rows at all yet), whereas this node's traversal and scoring work operates on
whatever tokens already exist in the store regardless of type. A future tool-token-ingestion
feature would automatically inherit both traversal and scoring for free once it starts writing
real rows via `upsert_token`/`insert_relationship` — no coupling requires ss-9 to be fixed here.
Left exactly where it was (`squire-storage/todo.json` ss-9, `status: open`, unclaimed).
