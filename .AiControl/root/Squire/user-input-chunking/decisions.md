# Decisions

## Reading the spec's ambiguity honestly before implementing

The spec's own wording for this feature is short and under-specified relative to most other
sections `retrieval-fidelity`/`tool-token-ingestion` worked from — three sentences at §4.3,
restated near-verbatim at §9.1 step 2 and §11, plus one worked example at §3.1. Four
concrete questions have no explicit answer in the spec text and are resolved here by
judgment, each documented separately below: (1) what triggers chunking, (2) what a "chunk"
is, (3) the exact `NNN`/`TN` numbering scheme, (4) how chunk tokens are meant to be
referenced afterward. `protocol-doc-sync/decisions.md`'s item 11 does not resolve any of
these either — it only confirms the gap exists and that no planning decision (Q1-Q7)
descoped it. This is genuinely new design work, not a case of "the spec already said X,
just implement it."

## (1) What triggers chunking: every user message, unconditionally — no size threshold

Considered gating chunking to "long" messages only (e.g. above some character count),
reasoning that a two-word user reply doesn't obviously need its own retrievable memory
token. Rejected: nothing in §4.3/§9.1/§11's wording introduces a threshold or conditional —
every mention reads as an unconditional turn-open step ("Squire auto-chunks the input",
step 2 of a fixed numbered sequence that runs "regardless" every turn, matching how step 3's
vector search and step 4's preserve-list load are also unconditional). Introducing a
threshold would be inventing a knob the spec never asks for, and would create an
inconsistent user-facing experience (some turns get a referenceable token for their input,
others silently don't, with no way for the AI to know in advance which). Chunking runs on
every turn, for every user message, including a "0-length" or trivial one — the chunker
below naturally still produces at least one chunk for any non-empty message, and produces
zero chunks (no tokens created, no error) only for a genuinely empty message, which cannot
happen in practice since `send_message_impl` never dispatches an empty user turn.

## (2) What "chunk" means: paragraph-then-sentence splitting, not sub-sentence/semantic chunking

The phrase "auto-chunks by natural language structure" is the spec's only guidance on
granularity. Three readings were considered:

1. **One token per whole message, no internal splitting at all.** This is the simplest
   possible reading, but it makes "chunk" a synonym for "message," which doesn't match
   §4.1's separate mention of "large pasted documents" as a case this same mechanism is
   meant to handle — a large pasted document is exactly the case where "one token for the
   whole thing" defeats the point of chunking (a token whose `short_desc` is "first sentence
   of the chunk," per §3.1's own worked example field, is a poor summary of a multi-page
   paste if the whole paste is one chunk). Rejected as too little to satisfy "natural
   language structure" as a described mechanism distinct from "the whole message."
2. **True semantic/topic-boundary chunking** (e.g. embedding-similarity-based segmentation,
   or an LLM call to determine topic breaks). Rejected as substantially disproportionate:
   nothing elsewhere in this runtime uses a semantic chunker for anything (§4.3's AI-output
   half of chunking is entirely manual, via the AI's own `§^` markers — the Squire itself
   never semantically segments AI output either), the spec never describes an embedding- or
   model-call-based mechanism for this specific step, and building one would introduce new
   latency (an extra model round-trip or a new embedding dependency) into every single
   turn's open, for a task the spec's own three-word phrase ("natural language structure")
   most naturally reads as referring to syntactic units (paragraphs, sentences) that are
   already trivially recoverable from the raw text with no additional inference step.
3. **Paragraph-then-sentence splitting on plain syntactic boundaries — chosen.** Split the
   user's message on blank-line-delimited paragraph boundaries first; any resulting
   paragraph that is itself very long (see the size-cap note below) is further split on
   sentence boundaries (`.`, `!`, `?` followed by whitespace or end-of-string, with a small
   guard against splitting on common abbreviations' trailing periods being out of scope —
   see "Known imprecision" below). This is "natural language structure" in the most literal,
   unadorned sense the phrase supports, requires no new dependency (`squire.rs` already has
   no NLP library, and this node does not add one), and directly serves the one concrete
   scenario the spec calls out by name (§4.1's "large pasted documents" — a long paste
   naturally has paragraph structure that this splits into separately retrievable chunks,
   while a short, single-sentence chat message naturally produces exactly one chunk, i.e.
   the simple case degrades gracefully to option 1's behavter without needing to special-case
   it).

**Size cap detail:** a paragraph is only further split into sentences if it exceeds
`CHUNK_SOFT_LIMIT_CHARS` (chosen: 400 characters — roughly the size of a short paragraph;
see "Why 400" below). Short paragraphs (the overwhelming common case for ordinary chat
messages, which are rarely even one full paragraph) are kept as a single chunk each,
avoiding the pathological case of a chatty multi-sentence-but-short message being fragmented
into many tiny one-sentence tokens that would clutter `explore()` results for little
retrieval benefit. A message with no blank-line breaks at all (the common single-paragraph
chat message case) is treated as one paragraph, so most ordinary user turns produce exactly
one chunk — matching the "simple case degrades to the obvious answer" goal above.

**Why 400 characters:** no configuration value exists in the spec's §15 table for this
(confirmed by reading it in full — `bootstrap_token_limit`, `bootstrap_top_n`, `max_retries`
are the only tuning constants listed, none about chunk size). Chosen as a round number in
the same rough order of magnitude as `bootstrap_token_limit`'s own 2000-character full-budget
default (§15), scaled down since a single chunk's `short_desc`/`full_desc` should be
meaningfully smaller than the entire prefetch budget it will compete for space and ranking
attention within. Not claimed to be spec-derived — a documented judgment call, kept as a
single named constant (`CHUNK_SOFT_LIMIT_CHARS`) so a future session can retune it without
hunting through the function body.

**Known imprecision, accepted:** sentence-boundary splitting on bare `. ! ?` punctuation will
mis-split on abbreviations ("Dr. Smith", "e.g. this"), decimal numbers ("3.14"), and similar
— a real NLP sentence tokenizer would handle these correctly but would be a new dependency
this node's proportionality judgment (see non-goals) explicitly avoids. Accepted as a minor,
cosmetic imprecision: a mis-split chunk is still a valid, retrievable, correctly-stored
token with slightly awkward boundaries, not a functional defect (nothing downstream requires
grammatically perfect sentence boundaries — `explore()`'s scoring is over `short_desc`/
`full_desc` text regardless of exactly where it was cut). Not flagged as a follow-up; this is
the same category of accepted approximation as `squire-storage/decisions.md`'s
deterministic-hash placeholder embedding function.

## (3) Token ID scheme: `USR_T{turn}_{NNN}`, turn-scoped sequence, zero-padded to 3 digits

Per §3.1's worked example (`USR_T2_001`) and env.md's reading of it: `T{turn}` is the
1-based turn number at which the chunk was created (matching `creation_turn`'s existing
"turn number at first insertion" semantics — this node reads the *current* turn, i.e. the
turn about to open, from `SquireStore::current_turn`, consistent with how `finalize_turn`
elsewhere reads `self.store.current_turn(session_id).await` as "the turn now in progress").
`NNN` is a sequence number starting at `001` for the first chunk of *that specific turn's*
input, incrementing by one per chunk within the same turn, and resetting to `001` at the
start of the next turn's chunking pass (not a session-lifetime monotonic counter) — chosen
because a per-turn reset is the only reading under which the `T{turn}` segment of the id
carries independent information rather than being fully redundant with a global counter
(see env.md's fuller reasoning). Zero-padded to 3 digits (`001`, not `1`) to match the
example's own literal formatting and to keep ids sortable as plain strings for a turn with
up to 999 chunks (a limit this node does not otherwise enforce or need to — no real message
is expected to produce anywhere near that many chunks given the 400-character soft cap).

**The task's own shorthand `USR_TN_NNN` (as opposed to the spec's literal worked example
`USR_T2_001`) is read as the same format, generalized** — `TN` standing for "T" + the
literal turn number placeholder, `NNN` for the zero-padded sequence placeholder — not a
second, different literal token-id template to additionally support. No second format is
implemented.

**Considered and rejected: a purely global, session-lifetime sequence (`USR_001`, `USR_002`,
... never resetting)** — simpler to implement (one counter instead of a turn-keyed reset) but
directly contradicts the spec's own worked example, which explicitly encodes a turn number
distinct from the intra-turn sequence. A global counter alone would also make a chunk's turn
of origin non-recoverable from its id without a separate lookup, whereas the chosen scheme
encodes it directly in the id itself, matching how `§^`-created referential token ids in
this same codebase (e.g. `TRT_FishingSpots_T3` in §3.1's own adjacent example) already
encode a turn number inline as a documented convention.

## (4) How chunk tokens are referenced/used afterward: purely for the model's own later
## reference and for `explore()`/graph participation — not a `§!`-sigil target from user text

The spec is explicit that "the AI is not responsible for" system referential tokens (§3.1)
and that "all relationship building is left to the AI" (§4.3) — the AI can choose to
`explore()` for them, `token_to_detail()` them, and connect them via `relationships` if it
judges a piece of user input worth linking into its own graph, exactly like any other
existing token in the store. There is no mechanism, anywhere in the spec, for a *user's own
raw input* to itself contain `§!`/`§^` sigil syntax that the Squire would need to parse out
of the incoming message — sigils are exclusively an AI-output convention (§5.1/§5.2's
`§!`/`§^` definitions are scoped to "AI output," and §8's response-JSON shape is what the AI
emits, not what a user types). This node does not add any sigil-parsing of user input, and
does not change `validate_squire_response`/`extract_inline_refs`/`extract_spans` (all of
which operate on `parsed.content`, the AI's own response, never on the user's message) — a
user typing a literal `§!` or `§^` substring in their chat message is inert plain text, same
as before this node, and this node's chunking does not scan for or treat that specially.
The chunk tokens' only afterward-use is: (a) discoverable via `explore()`'s ordinary type/
query filtering and graph traversal, same as any other token; (b) readable via
`token_to_detail()`; (c) linkable via the AI's own `relationships` field if it chooses to.

## Implementation: one new backend-agnostic free function, `chunk_user_input`, plus a call
## site in `build_turn_input` — no new `SquireStore` trait method

```rust
/// Splits `text` into "natural language structure" chunks (judgment call —
/// see decisions.md's "(2) What 'chunk' means" section): first by blank-line
/// paragraph boundaries, then, for any paragraph longer than
/// CHUNK_SOFT_LIMIT_CHARS, further by sentence-ending punctuation. Never
/// returns an empty chunk string; returns an empty Vec only for
/// whitespace-only/empty input.
pub fn chunk_user_input(text: &str) -> Vec<String> { ... }

/// First sentence (up to the first `. `/`!`/`?`/newline, or the whole chunk if
/// none found) of `chunk`, used as `short_desc` per spec §3.1's literal
/// worked-example field comment ("first sentence of the chunk").
fn first_sentence(chunk: &str) -> String { ... }

/// Generic chunk ingestor: splits `text` into `{prefix}_T{turn}_{NNN}`-id
/// `system_referential` tokens. Used for both user input (prefix="USR")
/// and model responses (prefix="RESP").
pub async fn ingest_text_chunks(text: &str, turn: u64, prefix: &str, store: &dyn SquireStore) { ... }

/// Backward-compat alias: ingests user input as USR_T{turn}_{NNN} tokens.
pub async fn ingest_user_input_chunks(text: &str, turn: u64, store: &dyn SquireStore) {
    ingest_text_chunks(text, turn, "USR", store).await
}

/// Ingests model response as RESP_T{turn}_{NNN} tokens (called from
/// finalize_turn after storing the response).
pub async fn ingest_response_chunks(text: &str, turn: u64, store: &dyn SquireStore) {
    ingest_text_chunks(text, turn, "RESP", store).await
}
```

Call sites:
- `build_turn_input`: `ingest_user_input_chunks(&user_text, current_turn, store)` — runs
  before the bootstrap vector search so the turn's own input is discoverable immediately.
- `finalize_turn`: `ingest_response_chunks(&parsed.content, turn, store)` — runs after the
  response is stored, so the model's output is also discoverable for future turns.

## (5) Model response chunking: RESP_T{turn}_{NNN} tokens

The same dumb heuristic used for user input is also applied to the model's response at the
end of each turn. Both the user's message and the model's reply become `system_referential`
tokens stored in the same namespace, differentiated only by the `USR_`/`RESP_` prefix.

The AI can place `§^bookmark` markers at byte offsets within its own response text, and can
create referential tokens that define semantic ranges across both `USR_T*` and `RESP_T*`
tokens — see `0012-referential-token-ranges.md` and the `ranges` field on
`NewTokenSpec`.

## (6) NewTokenSpec.type defaults to "concept"

The `type` field on `NewTokenSpec` is now optional. When omitted, it defaults to `"concept"`.
This reduces the burden on the model — most tokens the model creates are conceptual memory
units, and requiring an explicit type for every token was causing unnecessary compliance
failures. Explicit types (`"todo"`, `"decision"`, `"assumption"`, `"workflow"`, `"skill"`,
`"tool"`) are still needed when tools/workflows create them, but the model doesn't need to
specify them for `§^`-span tokens or explicit `new_tokens` entries.

## (7) Referential token ranges — ADR 0012

See `ArchitecturePlanning/adr/0012-referential-token-ranges.md` for the full design. In
summary: a `NewTokenSpec` can carry a `ranges: Vec<TokenRange>` field where each entry
specifies a slice of a `USR_T*` or `RESP_T*` token via bookmark name + optional offset/
length. This lets the AI define semantic groupings over the dumb chunk boundaries without
duplicating text.
                short_desc: first_sentence(&chunk),
                full_desc: Some(chunk),
            },
            turn,
        ).await;
    }
}
```

`SquireStore::upsert_token` is, once again, already exactly the right shape — no new trait
method, following `tool-token-ingestion`'s and `retrieval-fidelity`'s established precedent
of confirming trait sufficiency before assuming a change is needed. `creation_turn` is
passed as the real current turn (unlike `tool-token-ingestion`'s tool tokens, which used a
fixed `0` because tools aren't session-turn-scoped) — user-input chunks *are* session-turn-
scoped by definition, so `effective_priority`'s decay term behaves meaningfully for them
from the start, unlike the accepted tool-token imprecision.

**Call site: inside `SquireContextAdapter::build_turn_input`, immediately after reading
`user_text` and before the bootstrap `explore_memory` call.** This is the only call site
that exists or is needed — `build_turn_input` runs exactly once per turn, is the function
the spec's own runtime-status notes already name, and (critically, per env.md's "hard
requirement" note) must run chunking *before* `explore_memory("all", &user_text, ...)` so
that a long pasted document's freshly-created chunks are themselves immediately
bootstrap-discoverable within the very same turn they were created in, matching §9.1's
numbered step order (step 2 chunking precedes step 3 vector search).

**Considered and rejected: reading back the newly-created chunk ids to add them directly
into `prefetched_tokens` unconditionally, bypassing `explore_memory`'s scoring.** This would
guarantee the current turn's own input is always visible to the model, but was rejected as
unrequested scope — §9.1 step 2's own wording says nothing about guaranteed-inclusion; it
relies on step 3's ordinary vector search to surface them if and when they score well, the
same as any other token, and a freshly-chunked user message searched against itself with
`query = user_text` will trivially score at or near the top of cosine similarity anyway (the
embedding source is a function of the chunk's own text, which is a substring of the query),
so in practice this happens automatically without needing a special-cased bypass.

## Why this node does not touch `validate_squire_response`, `extract_inline_refs`, or any
## AI-response-parsing code

All three operate exclusively on `parsed.content` — the *AI's* response — never on the
user's input message. This node's chunking happens at turn *open* (`build_turn_input`),
strictly before the model has produced any response for this turn to validate. There is no
overlap in code path, and per judgment-call (4) above, no reason to introduce one (chunk
tokens are not meant to make user input "sigil-addressable" from within the user's own
message text — only addressable as ordinary store tokens the AI can look up afterward).

## Why chunk tokens do not participate in `SquireExploreTool`'s live-registry
## `"tool"`/`"tool_skill"` branch

Not applicable — `system_referential` is not `"tool"` or `"tool_skill"`, so chunk tokens
flow through `SquireExploreTool`'s ordinary `self.store.explore_memory(...)` branch
unconditionally, same as `concept`/`referential`/`skill` tokens already do. No special
casing was needed or added; `explore(resource_type="system_referential", ...)` and
`explore(resource_type="all", ...)` (or `"memory"`, once informally, though note
`type_matches`'s existing `"memory"` alias only expands to `concept`/`referential` today —
see "Known follow-up, not claimed" below) both already work against the new token type
purely because `token_type` is a free-form string filtered by exact match, exactly as
`tool-token-ingestion`'s new `"tool"` value needed zero `explore_memory` changes.

**Known follow-up, not claimed by this node:** `type_matches`'s `"memory"` alias (both
`InMemorySquireStore` and `LanceDbSquireStore`) currently expands only to
`concept`/`referential`, not `system_referential` — meaning `explore(resource_type="memory",
...)` will not surface chunk tokens even though they are conceptually memory-partition
content per §4.1 ("structured partition... primary memory partition"). Considered including
`system_referential` in that alias as part of this node's own scope, since it is a small,
arguably in-scope adjustment. Decided against: `"memory"` is not a spec-defined
`resource_type` enum value at all (§3.2/§6.1 list `concept | referential | system_referential
| workflow | tool | skill`, all six as siblings, no umbrella `"memory"` value) — it is a
pre-existing runtime-only convenience alias from before this node (already present prior to
this session, unrelated to this feature), and changing its expansion set is a small but
distinct behavioral change to existing, already-tested code this node did not otherwise need
to touch. The two direct spec-compliant filters, `resource_type="system_referential"` and
`resource_type="all"`, already work correctly and are sufficient for the AI to discover chunk
tokens. Flagged here rather than silently left inconsistent; a future session can decide
whether to widen the alias.

## Verification methodology: unit tests plus a headless integration check — no WDIO/GUI spec

Same reasoning as `tool-token-ingestion`'s session: this is a pure backend turn-open write
path with no new user-facing surface. The only way a human would ever observe its effect
through the UI is indirectly, by asking a real model to `explore(resource_type=
"system_referential", ...)` mid-conversation and hoping it mentions a chunk by name — a
strictly weaker, more indirect signal than asserting directly on the created token rows and
ids. Unit tests (both `InMemorySquireStore` and, where the assertion is backend-specific
enough to matter, `LanceDbSquireStore`) plus a small real-model manual check (see state.md)
were judged the right, proportionate verification tier — full detail and results are in
state.md rather than restated here.
