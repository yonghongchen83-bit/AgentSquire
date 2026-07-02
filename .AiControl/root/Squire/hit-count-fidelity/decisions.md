# Decisions

## What exactly is missing, restated precisely

Spec §3.3's table lists four hit-count events. `retrieval-fidelity` wired events 2 (preserve-
list load) and 4 (new_tokens at close) directly. For events 1 and 3, it installed one proxy —
crediting a hit on every `SquireTokenToDetailTool::execute` call against a store-backed token
— and documented that proxy as covering *both* remaining events, reasoning that spec §6.1's
own gloss ("calls token_to_detail **or** references in output") treats them as one underlying
mechanism restated twice.

That gloss is real, but it is a disjunction ("or"), and the prior session's proxy only
implements the first disjunct. The second disjunct — a token being *referenced in the AI's own
output* without necessarily being loaded via `token_to_detail` first — was left entirely
unwired. This is the concrete gap rf-13 flags, and it is real: the AI can perfectly legitimately
cite a token via `§!TokenID` in its final `content` purely from having seen it in the
`prefetched_tokens`/`preserved_tokens`/explore() short_desc lists, without ever calling
`token_to_detail` on it — the whole point of `short_desc` being cheap-to-scan-and-cite output
per §10.2 ("the bootstrap surfaces candidates cheaply, and the AI decides what to actually
read"). Under the current proxy, that citation earns the referenced token zero hit credit.

## Operationalizing the two remaining events

**Event 1, "Token appears in explore() results that AI acts on":** already partially covered
by the `token_to_detail`-call proxy (one of the two ways to "act on" a result, per §6.1's own
disjunctive gloss). The missing half is "act on" via `§!`-referencing the token in the
response's `content` — which is directly and reliably detectable at `finalize_turn` time,
independent of whether the AI ever called `token_to_detail` on that same token earlier in the
turn. No cross-call state about "was this token id present in a previous explore() result set"
is needed to detect this — the credit is simply due whenever a `§!` reference to an
**already-existing** store token appears in the final compliant response, which is exactly
the literal event 3 wording below.

**Event 3, "§! reference found in a chunk loaded into context":** the most literal reading is
that *any* content — the AI's own final response, prefetched short_descs, a `full_desc` body
returned by `token_to_detail`, a tool result — that contains an embedded `§!TokenID` marker
should credit the referenced token when that content is "loaded into context." Read narrowly
but faithfully: the one place in this codebase where content unambiguously "loads into
context" in a way this session can detect without new infrastructure is the AI's own response
`content` at turn close — it is quite literally expanded and loaded into the user-visible
context by `expand_for_display` immediately after this same point in `finalize_turn`. This is
also, not coincidentally, the exact same text `finalize_turn` already scans via
`extract_inline_refs` to build the `known` set for `§!` validation — so wiring event 3 here is
free of new parsing.

**Conclusion: events 1 (second disjunct) and 3 are the same wiring point.** Both are satisfied
by one mechanism: in `finalize_turn`, for every `§!TokenID` reference in the compliant
response's `content` that resolves to a token that **already existed in the store before this
turn's `new_tokens` upsert loop runs** (i.e. it is not simply being created for the first time
in this same turn — those tokens already get their turn-of-creation hit from `upsert_token`'s
"regardless" +1 per event 4, so crediting them again here would double-count the exact same
citation), call `record_hit(token_id)`.

**Why "already existed before this turn" is the right condition, not "not in new_tokens":** a
token can appear in `new_tokens` in the same turn it is also `§!`-referenced (the ordinary
"define and cite" pattern the system prompt describes: "new_tokens: definitions for every
token you reference via §! that isn't already in the store"). For such a token, `upsert_token`
already credits it once via event 4's "regardless" rule. Also crediting it again via this
node's new event-1/3 wiring would give a freshly-created, same-turn-cited token 2 hits instead
of 1 for a single citation — inconsistent with how a token that already existed and gets cited
again should also get exactly 1 additional hit, not credited twice for one act. The
distinguishing check is `self.store.token_exists(token_id)` evaluated **before** this turn's
`new_tokens` loop runs (`finalize_turn` already computes exactly this via the existing `known`
set, built from `extract_inline_refs` + `token_exists`, before the `new_tokens` loop) — so a
brand-new token defined and cited in the same turn is correctly excluded (it is not yet
`token_exists` at the time `known` is computed), and a pre-existing token that is merely cited
(not re-defined) is correctly included.

## What is deliberately still not wired: chunk-citing-chunk (nested §! inside a `full_desc`)

The literal event-3 wording ("any chunk containing a §!TokenID reference is loaded into
context") also covers the case where a `full_desc` body itself contains an embedded `§!`
reference, and that body is subsequently loaded into context via `token_to_detail`. For
example: token `TRT_A`'s `full_desc` contains the text "see §!TRT_B for details" — if the AI
calls `token_to_detail("TRT_A", "full")`, per the literal spec wording `TRT_B` should also earn
a hit, purely because its citation was inside content that got loaded, even though the AI never
directly referenced `TRT_B` in its own turn-closing output.

**This case is deliberately left unwired, as a documented, bounded approximation — not
silently dropped.** Reasons:

1. **Disproportionate infrastructure for the benefit.** Closing this fully requires scanning
   *every* piece of content that ever enters context for embedded `§!` markers — not just the
   AI's final response (a single, already-parsed string at a single well-defined point) but
   also `full_desc` bodies returned by `token_to_detail`, prefetched `short_desc` values in the
   bootstrap, and (in principle) any future context-composition source. That is a materially
   larger feature — a genuine context-composition audit pass — which is exactly what
   `retrieval-fidelity/decisions.md` already declined to build for the same reason, and which
   the task's own instructions call out as a case where "full fidelity requires infrastructure
   disproportionate to the benefit."
2. **The task's own explicit guidance authorizes exactly this kind of scoped tradeoff** — "if
   full fidelity requires infrastructure disproportionate to the benefit... consider whether a
   lighter-weight approximation is defensible and document that tradeoff explicitly." The
   `§!`-in-`content` wiring above is a materially fuller, more literal implementation than the
   prior session's proxy (it now covers the disjunction's second half, and does so via a
   direct, unambiguous reading of "loaded into context" rather than an indirect analogy) —
   this remaining nested case is a narrower, deeper residual than what rf-13 originally
   flagged.
3. **Secondary-signal risk tolerance.** `accumulated_hits`/`effective_priority` is documented
   throughout this epic (`retrieval-fidelity/decisions.md`, `handoff.md`) as a *secondary,
   tie-breaking* ranking signal, not a primary retrieval mechanism (traversal and vector
   similarity are the primary mechanisms; `effective_priority` only matters within an
   already-narrow near-tie score bucket). A residual undercount specifically for nested
   chunk-to-chunk citations — a comparatively rare authoring pattern (a token's own `full_desc`
   citing a different token) — has a bounded, secondary-signal impact, unlike the
   already-fixed gap (a token cited directly in the AI's visible output earning zero credit at
   all, which is a far more common pattern and now fully fixed by this node).

This tradeoff is logged here explicitly, and flagged as a new, smaller, unclaimed follow-up in
`state.md`'s Risks/Next Actions and in `root/Squire/handoff.md` — not silently treated as full
fidelity to the spec's literal wording. A future node could close it fully by extending
`token_to_detail`'s execute path to itself scan the returned `full_desc`/`short_desc` for
embedded `§!` references and call `record_hit` on each resolved one — the same `extract_inline_
refs` helper this node reuses would apply directly there too, so the primitive work is already
in place; only the additional call site would need to be added.

## Implementation: one new call site in `finalize_turn`, no new trait method, no signature changes

```rust
// After validate_squire_response succeeds and `known` (pre-existing §! refs already
// confirmed via token_exists) is available, before the new_tokens upsert loop:
for token_id in &known {
    self.store.record_hit(token_id).await;
}
```

Placed immediately after `self.retry_count = 0;` (the "this response is compliant" marker,
matching where `raw-partition-storage`'s own new call site was placed) and before the
`new_tokens` loop — order matters for the "already existed before this turn" distinction above:
`known` must be evaluated before any `upsert_token` call in this turn could newly insert a
token that would otherwise make `token_exists` spuriously true for a same-turn-created token.
`finalize_turn` already computes `known` earlier (during the `validate_squire_response` setup,
well before the `new_tokens` loop), so no reordering of existing code is needed — only a new
loop over the value that already exists at the right point.

No `SquireStore` trait changes — `record_hit` already exists and does exactly what's needed
(is a no-op for a token that doesn't exist, which cannot occur here anyway since `known` is
already filtered by `token_exists`). No `TokenSummary`/`NewTokenSpec` field changes. No
`explore_memory`/traversal changes. This keeps the change minimal and fully backend-agnostic:
because `record_hit` is a trait method both `InMemorySquireStore` and `LanceDbSquireStore`
already implement identically in spirit (increment-by-1, persist), this one new call site in
`finalize_turn` (which is backend-agnostic — it holds `Arc<dyn SquireStore>`) automatically
exercises both backends without any per-backend code being touched. Unit tests are still added
against both backends directly (per the task's explicit ask) to confirm the wiring end to end,
not just at the trait-mock level.

## Why not multiple hits for multiple references to the same token in one response

If a response's `content` cites the same existing token via `§!TokenID` more than once (e.g.
`§!TRT_A ... §!TRT_A` twice in one response), `known` is a `HashSet<String>` (already built
this way by pre-existing code, unchanged by this node) — so the token is only recorded once
per turn regardless of how many times it's cited in that single response. Considered whether
event 3's literal wording ("reference found in a chunk") implies per-occurrence counting
rather than per-turn-per-token counting. Rejected: `known`'s existing dedup shape was already
established as the correct interpretation for *validation* purposes by `squire-adapter`, and
reusing it for hit-crediting keeps one occurrence-counting policy throughout the response-
parsing pipeline rather than introducing an inconsistent second one (multiplicity within a
single response is a stylistic/repetition artifact of one generation, not four independent
"uses" of the token by four different callers) — consistent with how `upsert_token`'s event-4
credit is also exactly +1 per turn regardless of how many `new_tokens` entries or spans
reference the same id in one turn.

## Verification methodology: unit tests in both backends, no new headless harness needed

This node's change is a single new loop inside `finalize_turn`, already covered end to end by
existing `finalize_turn`-integration-style tests in `squire.rs` (see
`finalize_turn_persists_expanded_content_on_compliant_response` and siblings) and by
`InMemorySquireStore`/`LanceDbSquireStore` unit tests for `record_hit` (already exist from
`retrieval-fidelity`). New tests added: (1) a `finalize_turn` integration test confirming a
response that cites a pre-existing token via `§!` (without calling `token_to_detail`) earns
that token exactly one hit; (2) a test confirming a token defined in `new_tokens` and cited via
`§!` in the same turn earns exactly one hit total, not two (the double-count guard above); (3)
a test confirming multiple `§!` citations of the same existing token in one response still
credit exactly one hit (the dedup-via-`HashSet` behavior). A repo-wide frontend grep (matching
this epic's established practice) confirms zero frontend surface — no headless example harness
was judged necessary beyond the existing `raw_partition_storage_e2e.rs`/
`user_input_chunking_e2e.rs` precedents, since this change doesn't introduce a new call chain
across process/storage boundaries the way those nodes' features did (no new table, no new
trait method, no new backend-crossing data flow) — the existing unit-test-level coverage
against both real backends (`InMemorySquireStore` directly, `LanceDbSquireStore` via its own
test module) already exercises the real storage write path end to end.
