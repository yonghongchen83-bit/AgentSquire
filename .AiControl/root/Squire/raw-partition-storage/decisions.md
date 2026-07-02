# Decisions

## The central question: verbatim-everything, or unmarked-only? — resolved as unmarked-only

The task explicitly flags this as the thing to get right before implementing. Two readings
were considered:

1. **Verbatim-everything**: the raw partition stores every AI response in full, unconditionally,
   as a redundant audit trail alongside the ordinary chat-history table.
2. **Unmarked-only (chosen)**: the raw partition stores specifically the portion of an AI
   response's `content` that was **not** enclosed in a `§^...§^` span — i.e. content the
   model produced but chose not to promote into a structured, addressable memory token via
   the one mechanism (`§^`) the spec defines for "the act of memory creation" (§5.2's own
   phrase).

**Textual basis for choosing (2), read directly from `context_squire_spec_v2.md`:**

- **§4.1 defines the two partitions by direct contrast, not as independent stores of the same
  data:**
  > **Structured partition** — referential tokens created by AI via §^ marking, and system
  > tokens from user input. These have token IDs and graph connections. This is the primary
  > memory partition.
  >
  > **Raw partition** — auto-stored AI output that **was not explicitly marked with §^**.
  > Reachable only by vector similarity. No token representation, no graph connections.
  > Treated as an audit log, not as memory.

  The raw partition's own definition is phrased as "AI output that was not explicitly marked
  with §^" — not "AI output" unconditionally. If the intent were "store everything,
  regardless of §^ marking," this sentence would have no reason to include the qualifying
  clause "that was not explicitly marked" at all. The clause is doing real, load-bearing
  work: it is what distinguishes this partition's contents from the structured partition's.

- **§4.3's Ingestion Rules restate the same distinction even more explicitly, framing it as a
  binary split of a single body of AI output, not two independent copies:**
  > **AI output:** AI owns chunking entirely via §^ sigils. **If the AI does not mark a span,
  > it is stored only in the raw partition.** §^ marking is the act of memory creation.

  "If the AI does not mark a span, it is stored **only** in the raw partition" is the
  strongest textual evidence for reading (2): it describes a conditional, exclusive-or
  relationship — a given piece of output goes to *either* the structured partition (if
  `§^`-marked) *or* the raw partition (if not), not to both, and not entirely to the raw
  partition regardless of marking.

- **§9.4's Turn Close numbered sequence lists "parse §^ spans" (step 2) and "store raw
  response" (step 4) as separate, sequential steps operating on the same source text
  (`content`), not as two unrelated archival operations:**
  > 2. Parse §^ spans in content. For each span: extract token ID and text content, store
  >    chunk in LanceDB **structured** partition, create or update the referential token
  >    record.
  > 3. Expand sigils for display... Output clean prose to user.
  > 4. **Store raw response** in LanceDB **raw** partition. Not tokenized. Audit log only.

  If step 4 meant "store the entire response, `§^` spans included, a second time," step 2 and
  step 4 would both be archiving the exact same span text into two different LanceDB
  partitions — directly contradicting §4.1's own claim that the raw partition has "no token
  representation, no graph connections" (span text *does* get token representation, via step
  2's `upsert_token`-equivalent). The two steps only make coherent sense together if step 4's
  "raw response" means "whatever is left of the response once the §^-marked chunks have
  already been accounted for by step 2" — i.e. the unmarked residual.

**Why the verbatim-everything reading was rejected, beyond the textual points above:** it
would make the entire raw-partition feature functionally redundant with data the runtime
already durably persists. `finalize_turn`'s existing, unrelated `ConversationStore::
append_message` call already writes a version of every compliant turn's assistant output
(the display-expanded prose) to the ordinary chat-message history, which is trivially
queryable and already displayed to the user. A second table storing the *exact same
information* (or a near-verbatim raw variant of it) end to end for every single turn would
add real storage/write cost for a feature the spec's own careful two-partition wording does
not describe wanting. The task's own framing ("content the model said but chose not to
persist as addressable memory") independently arrived at the same conclusion this textual
reading supports.

## Is this partition ever read back by the model? — resolved as no

§4.1's exact phrase is "Explore() does not search this partition by default." Two readings
were considered: (a) this implies some other, unspecified read/search mechanism might exist
or be added later for this partition, which this node should therefore build; (b) this is
simply a descriptive negative — the one search tool the spec defines does not cover this
partition, full stop, with no implied alternative mechanism anywhere else in the document.

Chosen: (b). Grepped the full spec document for every other candidate read mechanism —
`token_to_detail()` operates only on the structured `squire_tokens` partition (its signature
and every described use are token-id-keyed lookups against token records, which raw-partition
entries by definition do not have, per §4.1's "no token representation"); `invoke()` is
strictly for MCP tool dispatch; no fourth tool or "raw search" mechanism is described anywhere
in §6 (Built-in Tools, the exhaustive list: explore/token_to_detail/invoke) or in the
system-prompt doc. "By default" most naturally reads here as hedging language describing
`explore()`'s own scope (it searches the structured partition; it happens not to search this
other one), not as a forward-looking promise of a togglable or extensible search mode — no
such toggle or extension point is described anywhere. This matches `squire-storage/
decisions.md`'s existing `squire_compliance_failures` precedent exactly: "Append-only,
debugging-only table... never queried for runtime decisions, only inspected for
diagnostics." The new raw-partition table is designed the same way: write-only from the
model's perspective, with no `SquireStore` trait method that reads it back.

## Storage design: one new trait method, one new LanceDB table, no new read path

**New `SquireStore` trait method:**

```rust
/// Persists the unmarked residual of a compliant turn's AI output — the
/// portion of `content` that was NOT enclosed in a `§^...§^` span (spec
/// §4.1/§4.3: "if the AI does not mark a span, it is stored only in the raw
/// partition"). Append-only, write-only from the model's perspective — no
/// SquireStore method reads this back (spec: "Explore() does not search
/// this partition by default"; no other read mechanism is described
/// anywhere in the protocol). A no-op call with empty `content` is valid
/// and permitted (callers should prefer skipping the call entirely when
/// there is nothing to store — see finalize_turn's call site).
async fn record_raw_output(&self, session_id: SessionId, turn: u64, content: String);
```

Follows the trait's existing convention exactly (one narrow async method per storage
operation, matching `record_compliance_failure`'s shape/spirit as the closest precedent).
No return value, no read-back method — deliberately asymmetric with every other table in the
store (`squire_tokens` has `token_exists`/`token_detail`/`explore_memory`;
`squire_relationships` is written via `insert_relationship` and read internally via
`load_relationship_edges` for traversal; even `squire_compliance_failures`, this feature's
closest precedent, has no trait-level read method either — its data is inspected only via
direct table/file access outside the running app, exactly the audit-log posture this node
also wants).

**New LanceDB table**, following `squire_compliance_failures`'s exact schema-simplicity
precedent (plain string/scalar columns, no embedding column):

```
squire_raw_partition
  session_id   string
  turn         uint64
  content      string
  timestamp    string (RFC3339, matching compliance_failures_schema's own convention)
```

No embedding column, deliberately, even though §4.1 says this partition is "reachable only by
vector similarity" — because nothing in this runtime ever performs that vector search (see
the read-back conclusion above). Adding a real embedding column and wiring it into a search
path nobody calls would be speculative, unrequested scope; the existing `embed_text`
placeholder function is not reused here for the same reason `squire_compliance_failures`
never adopted one. If a future node ever needs to make this partition genuinely searchable
(closing the literal "reachable only by vector similarity" clause), it can add an embedding
column via the same additive-schema-change precedent `retrieval-fidelity` already established
for `squire_tokens`'s `accumulated_hits` column — not blocked by this node's choice.

**`InMemorySquireStore` equivalent:** a `Mutex<Vec<RawPartitionRecord>>` field (mirroring
`compliance_failures: Mutex<Vec<ComplianceFailureRecord>>`'s exact existing shape), with a new
`RawPartitionRecord { session_id, turn, content, timestamp }` struct (mirroring
`ComplianceFailureRecord`'s shape). Test code that wants to assert on what was recorded reads
this `Vec` directly (via a small `#[cfg(test)]`-only accessor, matching how existing tests
already inspect `RecordingStore.appended` directly in `squire.rs`'s test module) — no trait
method is added for this, keeping the trait itself symmetric between backends and consistent
with "no read-back method" above; the in-memory backend's test-only visibility is a
test-harness convenience, not a production capability being added.

## Extraction helper: `unmarked_residual`, reusing `extract_spans`'s parsing, not duplicating it

```rust
/// Returns the portion of `content` that falls OUTSIDE every closed
/// `§^...§^` span (spec §4.1/§4.3's "unmarked" AI output — the raw
/// partition's contents). Reuses `extract_spans`'s own parsing to avoid a
/// second, independently-maintained sigil parser; spans is the same value
/// `finalize_turn` already computes via `extract_spans(&parsed.content)`
/// immediately before this is called, so no re-parsing occurs at the call
/// site.
fn unmarked_residual(content: &str, span_count: usize) -> String { ... }
```

Considered re-deriving spans from `content` a second time inside a new free function taking
only `content: &str`. Rejected in favor of reusing the split `content.split("§^")` structure
directly (mirroring `strip_span_markers`'s existing approach, which already solves almost the
identical problem — "reconstruct `content` with `§^` regions handled specially" — for the
opposite goal of *keeping* the non-span text and discarding markers). `unmarked_residual`
is implemented as a close sibling of `strip_span_markers`: instead of keeping and
concatenating the non-span parts (`strip_span_markers`'s job — producing clean prose for
`expand_for_display`), it keeps and concatenates only the non-span parts *excluding* the span
bodies themselves, joined with a single space, trimmed. This is a new, small, pure,
independently unit-tested function — not a parameterization of `strip_span_markers` itself,
since the two functions' outputs (clean-prose-with-spans-inlined vs. only-the-outside-text)
are different enough that a shared implementation with a boolean flag would be less readable
than two small functions sharing the same `split("§^")` traversal shape.

**Edge cases covered by unit tests:**
- No `§^` spans at all in `content` -> the entire `content` is unmarked residual (verbatim).
- One or more closed spans -> only the text outside the spans is residual; span bodies are
  excluded.
- The entire `content` is one single closed span with nothing before/after ->  residual is
  empty (nothing to store — see the "skip empty writes" decision below).
- An unclosed span -> cannot occur at this call site in practice, since
  `validate_squire_response` already rejects any response with an unclosed span before
  `finalize_turn` reaches the point where `unmarked_residual` is called (covered by an
  existing, unmodified rejection test — no new test needed for this case specifically, but
  noted here for completeness).

## Call site: `finalize_turn`, immediately after `extract_spans`, compliant path only

`SquireContextAdapter::finalize_turn` already computes `let (spans, _) =
extract_spans(&parsed.content);` right before the `new_tokens` upsert loop, on the one path
that has already passed `validate_squire_response` (`self.retry_count = 0;` runs just above
this line — the unambiguous "this response is compliant" marker). The new call is inserted
immediately after that line:

```rust
let residual = unmarked_residual(&parsed.content, spans.len());
if !residual.is_empty() {
    self.store.record_raw_output(session_id, turn, residual).await;
}
```

**Why only the compliant path:** a rejected response (malformed JSON, unclosed span,
undisplayable token reference, `ask_user`/`content` conflict) never reaches this line —
`reject_and_record` returns earlier. This is intentional, not an oversight: rejected turns
already get a structured, complete audit record via the pre-existing
`record_compliance_failure` path (`rule`, `reason`, `retry_count`, `failed_content`,
`timestamp`) — a rejected response's *entire* content, not just an "unmarked residual" of it,
is already captured there for debugging. Adding a second raw-partition entry for the same
rejected content would be a second redundant audit trail for exactly the case that already
has the most complete one. The `ask_user` early-return path also never reaches this line for
the same structural reason (it returns before validation/`extract_spans` even runs); an
`ask_user` turn has no `content` to consider "unmarked" in the first place per
`validate_squire_response`'s mutual-exclusion rule.

**Why the empty-residual write is skipped:** a response entirely composed of one or more
`§^`-marked spans, with no other prose, has nothing left over once every span is excluded —
`unmarked_residual` correctly returns an empty string, and writing an empty audit-log row for
every such turn would be pure noise (every table this session inspected — `squire_tokens`,
`squire_relationships`, `squire_compliance_failures` — only ever writes rows carrying real
content; none writes an empty placeholder row "just to have one per turn"). This mirrors
`user-input-chunking/decisions.md`'s own precedent of never creating a token for an empty
chunk.

## Why `session_id`/`turn` are stored on each raw-partition row (not just `content`)

`squire_compliance_failures` stores `session_id` on every row despite being an append-only
audit table with no trait-level read method — precisely so a human operator inspecting the
LanceDB directory directly (the only way this data is ever consumed, per the "no read-back"
conclusion above) can filter/group by session and correlate with the ordinary chat-message
history for the same session. The raw-partition table follows the identical reasoning, adding
`turn` as well (available for free from `finalize_turn`'s own `let turn = self.store.
current_turn(session_id).await;` call, already computed one line above `extract_spans` in the
existing code) so an operator can also correlate a given unmarked-residual entry with the
exact turn number visible in `squire_tokens`' `creation_turn` column for tokens created in the
same turn.

## Verification methodology: unit tests (both backends) plus a headless integration harness —
## no WDIO/GUI spec

Same reasoning as every backend-only node in this epic since `tool-token-ingestion`: this is
a pure backend turn-close write path with zero new user-facing surface. Confirmed via a
repo-wide grep across `src/` for `raw_partition`/`record_raw_output`/"raw partition" (case
insensitive) — zero hits outside this session's own backend changes, confirming no frontend
code references or could reference this feature. A human could only ever observe this
partition's effect by opening the LanceDB directory directly with an external tool — not
through any UI this app exposes — so a WDIO/GUI spec would provide strictly weaker signal
than a direct assertion on the table's rows, the same judgment `tool-token-ingestion` and
`user-input-chunking` both made for their own similarly backend-only changes. Full test
results and the headless harness's transcript are in `state.md`.
