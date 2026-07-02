# Context Squire — Specification v2

**Scope:** Memory management, resource discovery, and conversational context curation.  
Sandboxes, cross-session continuity, and cleanup mode are deferred.

> **Runtime reconciliation pass (`protocol-doc-sync`, 2026-07-02):** this document has been annotated inline with "Implementation status (runtime v1)" notes wherever the shipped implementation (`src-tauri/src/agent/squire.rs`, `src-tauri/src/storage/squire_lancedb.rs`) diverges from what's specified here. Two categories of annotation appear:
> - **Intentional, judged adaptations** (e.g. the system-prompt transport not literally matching the illustrative request-JSON shape in §8.1; the `tool_skill` explore() return shape; the retry-exhaustion/rejection-visibility behavior in §8.3/§14, which is now *more* than this document originally specified, not less) — these describe what runtime v1 actually and deliberately does.
> - **Genuine unimplemented gaps** (`accumulated_hits`/`effective_priority` scoring, graph traversal via `num_hops`, user-input auto-chunking into `USR_` tokens, raw-partition audit storage, the `ask_user` response-field loop) — these are called out explicitly as not implemented rather than silently glossed over. None of these were deliberately descoped in the planning decisions (`../planning/decisions.md` Q1–Q7); they are drift that emerged during implementation, tracked here for future work rather than treated as done.
>
> Search for `Implementation status (runtime v1)` throughout this document for every annotated point. See `../protocol-doc-sync/decisions.md` for the full reasoning behind each judgment call.

---

## 1. Overview

Most AI agent frameworks accumulate context as a continuously growing text buffer. As conversations grow, the model receives irrelevant history alongside every new request and performance degrades.

This system takes a different approach. All information is stored as a typed, connected graph of resources managed by a lightweight component called the Context Squire. The Main AI receives only what is relevant to the current task, actively explores its own memory when it needs more, and explicitly marks what it wants to remember. The context is never a dump of history — it is an actively curated working set designed by the AI itself.

**The central principle:** like Linux treats everything as a file, this system treats everything as a resource.

**The intelligence boundary:** the Squire is a dumb script. It stores, retrieves by vector similarity, traverses graph relationships, and executes protocol commands. It does not reason, classify, or build relationships. All structural decisions belong to the Main AI.

---

## 2. Resource Hierarchy

Resources are organised into four levels. This is a logical distinction for discovery and tool registration, not a storage distinction — all resources live in the same underlying stores.

### Level 1 — Workflow

A pattern of working mode that the AI follows to handle a category of tasks. Examples: simple Q&A, waterfall design flow, interactive friendly chat, RPG simulator, custom user-defined flow. Workflows are discovered and selected by the AI; they are not hardcoded or agent-mode constructs.

### Level 2 — Tools and Skills

Defines the AI's capability surface. Tools are registered via MCP and discovered dynamically. Skills are markdown instruction sets describing how to perform a class of task. Neither tools nor skills are enumerated upfront — the AI discovers what exists when it needs it.

### Level 3 — Memory

The AI's internal structured knowledge: concept tokens, referential tokens, and their relationships. This is the only level the Squire manages directly. Memory is not conversation history — it is what the AI has explicitly chosen to retain and structure.

### Level 4 — External World

Everything not indexed in memory: files on disk, websites, databases, APIs, project source trees. The AI accesses these via discovered tools (readfile, webfetch, shell commands, etc.). Content from the external world does not become memory unless the AI explicitly ingests it.

---

## 3. Token Model

A **token** is the fundamental unit of memory. Every piece of information in the system is referenced through a token. The token is the handle; the content is always retrievable via the handle.

### 3.1 Token Types

**Concept Token**  
A pure semantic node. Has no text body. Exists only to serve as a connection point in the relationship graph. The AI creates concept tokens to represent ideas, entities, categories, or any abstraction it wants to use as a retrieval hub.

```
id:          CONCEPT_FishingLocation
type:        concept
short_desc:  "Locations relevant to fishing discussions"
full_desc:   (optional extended display text)
```

**Referential Token**  
A named pointer to a specific chunk of text in the vector store. Created when the AI marks a span of its output with `§^`. The token's content is the text of that span.

```
id:          TRT_FishingSpots_T3
type:        referential
short_desc:  "Fishing location suggestions generated turn 3"
chunk_ref:   (LanceDB chunk identifier)
```

**System Referential Token**  
Auto-generated by the Squire when ingesting user input or large pasted documents. The Squire creates one token per chunk. The AI is not responsible for these but can connect them via relationships.

```
id:          USR_T2_001
type:        system_referential
short_desc:  (first sentence of the chunk)
chunk_ref:   (LanceDB chunk identifier)
```

**Resource Tokens**  
Workflows, tools, and skills are also stored as tokens. Their full description format is type-enforced:

| Resource type | Full description format |
|---|---|
| Workflow | Structured prose describing the working pattern |
| Tool | Standard MCP tool schema (name, description, input schema) |
| Skill | Markdown instruction set |
| File reference | `{filename, offset, length, date, encoding}` |

### 3.2 Token Record Fields

```
id              string    unique identifier, no spaces or § characters
type            enum      concept | referential | system_referential | workflow | tool | skill
short_desc      string    one or two sentences; shown in explore() results and prefetch lists
full_desc       string    type-enforced full content; returned by token_to_detail()
chunk_ref       string    LanceDB reference (referential and system_referential types only)
accumulated_hits integer  strictly additive; never decremented
creation_turn   integer   turn number at first insertion
```

### 3.3 Effective Priority

At any sort point, effective priority is computed lazily:

```
effective_priority = accumulated_hits - (current_turn - creation_turn)
```

A new token scores 0 at birth. A never-referenced token drifts negative. A frequently referenced token stays near zero or positive. No active sweep is required — the decay is implicit in the formula.

**Hit count increment events:**

| Event | Delta |
|---|---|
| Token appears in explore() results that AI acts on | +1 |
| Token in preserve list loaded at turn open | +1 |
| §! reference found in a chunk loaded into context | +1 |
| Token listed in new_tokens at turn close | +1 |

**Implementation status (runtime v1): not implemented.** No token record anywhere in the runtime (`TokenSummary`, `TokenDetail`, or the LanceDB `squire_tokens` schema) carries an `accumulated_hits` field, and `effective_priority` is not computed. `creation_turn` is stored but is not consumed for ranking. `explore_memory` ranks results by cosine similarity against a placeholder hash-based embedding plus a substring-match boost (see `squire-storage/decisions.md`'s "Embedding function" note) — ties are not broken by `effective_priority` because it does not exist. This is a real, unflagged (until this pass) gap between spec and runtime, not a documented simplification — flagged here rather than silently treated as done. No node has claimed this as follow-up work yet; see `decisions.md`.

---

## 4. Storage Architecture

### 4.1 Vector Store (LanceDB)

All text content lives here. Every chunk has an embedding vector used for semantic search. The vector store is the entry point for all retrieval — `explore()` starts here.

Two partitions exist:

**Structured partition** — referential tokens created by AI via §^ marking, and system tokens from user input. These have token IDs and graph connections. This is the primary memory partition.

**Raw partition** — auto-stored AI output that was not explicitly marked with §^. Reachable only by vector similarity. No token representation, no graph connections. Treated as an audit log, not as memory. Explore() does not search this partition by default.

**Implementation status (runtime v1): not implemented.** `squire_lancedb.rs` has no raw-partition table (its five tables are `squire_tokens`, `squire_relationships`, `squire_turns`, `squire_preserve_lists`, `squire_compliance_failures` — none of which hold unmarked AI output). `finalize_turn` only persists the display-expanded, sigil-stripped `content` string to `ConversationStore` (the ordinary chat-message history) — unmarked prose is not separately archived to any Squire-owned audit log the way this section describes. A genuine gap, not a deliberate simplification; flagged here rather than silently glossed over. See `decisions.md`.

### 4.2 Triplet Store

An RDF-style graph of typed relationships between tokens. Used for graph traversal in `explore()` when `num_hops > 0`.

```
subject     string    token_id
predicate   string    any string; no vocabulary enforcement
object      string    token_id
```

Relationships are directional. The AI may use any predicate names it chooses. Relationship naming consistency is a cleanup concern, not enforced by the Squire.

**Implementation status (runtime v1): write path only.** `SquireStore::insert_relationship` is implemented (both `InMemorySquireStore` and the LanceDB `squire_relationships` table) and the AI's `relationships` field is persisted every compliant turn close — but nothing currently reads the triplet store back for graph expansion. `explore()`'s `num_hops` parameter is accepted end-to-end (tool schema → `SquireStore::explore_memory`) but has no effect on the result set at any `num_hops` value. See the traversal note under §6.1 and `decisions.md`.

### 4.3 Ingestion Rules

**User input:** Squire auto-chunks by natural language structure. Generates USR_TN_NNN tokens per chunk. No relationships are auto-generated. All relationship building is left to the AI.

**Implementation status (runtime v1): not implemented.** `SquireContextAdapter::build_turn_input` takes the raw text of the most recent user message directly as `user_request` — no auto-chunking occurs, and no `USR_`-prefixed (or any) tokens are created for user input. This is a genuine gap, not an intentional deferral captured anywhere in the planning decisions; flagged here. See `decisions.md`.

**AI output:** AI owns chunking entirely via §^ sigils. If the AI does not mark a span, it is stored only in the raw partition. §^ marking is the act of memory creation.

**Implementation status (runtime v1):** the §^ half of this rule is implemented as described (`finalize_turn` creates/updates a referential token per closed span and captures the span text as `full_desc` when not otherwise supplied). The "stored only in the raw partition" half is not, per the §4.1 note above — content that is not marked with §^ is not persisted as memory or as an audit-log entry; it is simply not retained by Squire once the turn's display message has been shown.

**External world content:** Content arrives in context via tool results (readfile, webfetch, invoke). It is ephemeral — gone at turn end unless the AI explicitly ingests it by marking relevant spans with §^ in its response.

**Schema note:** Raw partition chunks are stored with their prose text as the embedding. Structured partition chunks store the same text but also carry a token ID for retrieval by handle.

---

## 5. Sigil Notation

Sigils are internal system markers embedded in AI output. They are never visible to the user. The Squire parses and processes them at turn close.

The sigil base character is `§`. The modifier character following it indicates type. These markers exist for one reason: to let the AI annotate its own output during generation without a second pass.

### 5.1 §! — Inline Token Reference

Used in AI output to reference an existing token by ID instead of writing out its full description.

**Format:** `§!TokenID` — terminated by the next whitespace or the next `§`.

**Example:**
```
The best approach here follows §!WF_WaterfallDesign, starting with requirements.
```

**Effect at display:** Squire expands `§!TokenID` to the token's `short_desc` (or `full_desc` if configured) before showing output to the user. User sees clean prose.

**Effect at ingestion:** When any chunk containing a `§!TokenID` reference is loaded into context, the referenced token's `accumulated_hits` increments by 1. This means heavily-cited tokens accumulate priority passively through normal context use. **Implementation status (runtime v1): not implemented — see §3.3.**

**Constraint:** The token must exist in the store or be defined in the same response's `new_tokens`. If neither is true, Squire rejects the response with reason `"undisplayable token §!TokenID"`. **Implementation status (runtime v1): implemented as described** — `validate_squire_response` in `squire.rs` checks this exact rule.

### 5.2 §^ — Named Span

Used to mark a region of AI output as a named referential token. This is the primary act of memory creation.

**Format:** `§^TokenID content §^` — opened by `§^TokenID`, closed by bare `§^`. Does not nest.

**Example:**
```
§^TRT_FishingSpots The best spots near Sydney are Middle Harbour for bream and Botany Bay for flathead. Both are accessible by car and have nearby bait shops. §^
```

**Effect at turn close:** Squire creates or updates a referential token with `id = TRT_FishingSpots`, stores the span content as a chunk in the vector store, and writes the chunk reference to the token record. If the token already exists, the chunk reference is updated and `accumulated_hits` increments. **Implementation status (runtime v1):** the token create/update half is implemented (`upsert_token`, with the span text captured as `full_desc` when the AI doesn't supply one explicitly) — there is no separate `chunk_ref` field in the runtime schema (the span text is stored directly as `full_desc`, not as a pointer to a distinct LanceDB chunk record), and `accumulated_hits` incrementing is not implemented (see §3.3).

**Note on §@ anchors:** The original spec used `§@NNN` segment anchors in AI output for text slicing. These are no longer required from the AI. The Squire handles all chunking internally on ingestion. The `§@` form is reserved for Squire-internal use if needed.

---

## 6. Built-in Tools

The Squire exposes exactly three built-in tools to the AI. All other tools must be discovered via `explore()`. The AI never sees raw MCP — the Squire is the sole MCP gateway.

### 6.1 `explore(resource_type, query, num_hops, max_results)`

Searches the structured partition of the vector store against `query`, then expands results via graph traversal to depth `num_hops`.

**Canonical v1 contract (Q1, hybrid-policy resolution — this is the system-prompt superset, now also the spec's contract, not the original narrower table below):**
- `resource_type`: `workflow | tool | skill | tool_skill | memory | concept | referential | all` — the four types after `skill` (`tool_skill`, `concept`, `referential`) are additive to the original five-value set; existing base types (`workflow | tool | skill | memory | all`) remain supported unchanged, so this is non-breaking.
- `query`: natural language search string
- `num_hops`: integer ≥ 0; 0 = vector search only, 1+ = include graph-connected tokens. **Implementation status (runtime v1): accepted but not implemented — see the traversal note below.**
- `max_results`: integer ≥ 1; caps the number of results returned (default 10 in runtime v1). For `tool_skill`, `max_results` applies independently per subtype.

**Returns (single type):**
```json
[
  {"token_id": "WF_InteractiveFriendlyChat", "score": 0.87, "short_desc": "Friendly conversational flow for casual user queries"},
  {"token_id": "CONCEPT_UserLocation",       "score": 0.72, "short_desc": "Concept node connecting location-relevant memory"},
  ...
]
```

**Returns (`resource_type = "tool_skill"`, runtime v1 actual shape):** a single flat array mixing both subtypes, not a `{"tool": [...], "skill": [...]}` dict — each element carries its own `type` field (`"tool"` for registry-sourced tool results, or whatever `SquireStore` reports for skill tokens) so the two subtypes remain distinguishable without a nested shape:
```json
[
  {"token_id": "TOOL_Weather",         "type": "tool",  "score": 0.91, "short_desc": "..."},
  {"token_id": "SKILL_LocationFinding","type": "skill", "score": 0.78, "short_desc": "..."}
]
```
(An earlier draft of this contract, still reflected in `context_squire_system_prompt_v2.md`, described a nested `{"tool": [...], "skill": [...]}` return for `tool_skill`. Runtime v1 does not do this — the flat, type-tagged array above is actual behavior. See `decisions.md` for why the flat shape was accepted as the documented contract rather than treated as a bug to fix.)

Results are ordered by score descending. Ties broken by effective_priority. **Implementation status (runtime v1): `effective_priority`/`accumulated_hits` do not exist at all — see the scoring note under §3.3. Ranking today is cosine-similarity-over-a-placeholder-embedding plus a substring-match boost (see `squire-storage/decisions.md`); ties are not broken by any priority field.** The AI receives token IDs and short descriptions only — full content requires a `token_to_detail()` call.

**Hit count:** Squire increments `accumulated_hits` for every token in the returned list that the AI subsequently acts on (calls `token_to_detail` or references in output). **Implementation status (runtime v1): not implemented — see §3.3.**

**Graph traversal implementation status (runtime v1):** `num_hops` is accepted by both `SquireStore` implementations (`InMemorySquireStore`, `LanceDbSquireStore`) but neither performs real triplet-store traversal — `explore_memory` does a flat type + similarity filter only, regardless of the `num_hops` value passed. The triplet store itself (relationships table) exists and is written to (`insert_relationship`), but nothing reads it back for expansion yet. This is a real gap against this spec, not an intentional simplification — flagged here rather than silently documented as if traversal works. See `decisions.md`.

### 6.2 `token_to_detail(token_id, detail_level)`

Retrieves the full or short description of a specific token.

**Parameters:**
- `token_id`: the token identifier
- `detail_level`: `short | full`

**Returns for `full`:** The type-enforced full description. For a tool token this is the complete MCP schema. For a referential token this is the stored text content. For a concept token this is the `full_desc` field.

**Returns for `short`:** The `short_desc` field only.

**Hit count:** `accumulated_hits` increments by 1 on each call. **Implementation status (runtime v1): not implemented — see §3.3.**

### 6.3 `invoke(token_id, params)`

Invokes a tool or skill through the Squire as gateway. The AI never calls external MCP servers directly.

**Parameters:**
- `token_id`: a tool token whose `full_desc` is a valid MCP schema
- `params`: parameters conforming to that schema's input definition

**Behaviour:** Squire looks up the token's full description, extracts the MCP endpoint, proxies the call, and returns the result. From the AI's perspective the interface is identical to standard tool calling — the schema it received from `token_to_detail()` is the same format it would see from any MCP server. The gateway layer is transparent.

**Note:** AskUser may be registered as a tool by a workflow or user configuration. If so, it is invoked via this path and the user's answer is returned synchronously as the tool result. If AskUser is not registered as a tool, the question mechanism is handled via the response field described in Section 8.2. **Implementation status (runtime v1):** the `invoke()`-as-tool path (Q2's "lightweight" AskUser option) works today through the ordinary tool-call loop — any tool named appropriately and registered in the tool registry is reachable this way, no Squire-specific plumbing needed. The response-field path (Section 8.2) is **not** implemented — see the note under §9.3.

---

## 7. Retrieval Architecture

### 7.1 The Three-Layer Stack

**Layer 1 — Vector search:** Entry point for all retrieval. Finds candidate tokens by semantic similarity. Low precision, broad net. This is pure RAG and its limitations are accepted. It is only the starting point. **Implementation status (runtime v1): implemented**, via a deterministic placeholder embedding (not a trained model — see `squire-storage/decisions.md`) rather than true semantic similarity; the vector-search *path* is real, the embedding quality is a documented, swappable placeholder.

**Layer 2 — Concept tokens:** Pure graph topology nodes. No content. When a vector search lands on or near a concept token, the AI traverses its edges to find what is connected. `num_hops` in `explore()` drives this traversal. A well-connected concept token is a retrieval hub — everything semantically related to that idea is reachable from it. **Implementation status (runtime v1): not implemented — see the traversal note under §6.1/§4.2.** Concept tokens can be created and stored today, but nothing traverses their edges; `explore()` never returns tokens purely by virtue of a graph connection.

**Layer 3 — Referential tokens:** The leaves of the graph. Every referential token points to an actual text chunk. `token_to_detail()` on a referential token returns the stored content. These are the only tokens that carry text payloads.

### 7.2 The Full Retrieval Path

```
User query + preserved tokens from last turn
    ↓
Vector search (structured partition) → candidate token list with scores
    ↓
Graph traversal at num_hops depth → expanded connected token set
    ↓
AI inspects short_desc of candidates, selects what to expand
    ↓
token_to_detail() → text content loaded into context
    ↓
AI reasons with loaded content
```

The AI has full agency over which candidates to expand and which to ignore. Squire surfaces candidates; AI decides what actually enters its working context.

### 7.3 Memory vs RAG

Vector search of the raw partition is pure RAG: find text similar to the query, return it. This is low-recall and low-precision because there is no structure to traverse.

Structured memory via concept and referential tokens is what makes this system different from a RAG wrapper. The AI's token and relationship graph is its *designed* retrieval architecture. A query that lands on `CONCEPT_FishingLocation` can reach `TRT_FishingSpots_T3`, `TRT_TidalConditions`, `WF_OutdoorActivityPlanning`, and `USR_T1_003` in a single traversal — none of which might score well on raw vector similarity alone.

**Consequence for AI behaviour:** When the AI creates concept tokens and writes relationships, it is designing its own future retrieval routes. This is memory architecture, not labelling.

---

## 8. Protocol

All communication between Squire and Main AI is JSON.

### 8.1 Request Format (Squire → AI)

```json
{
  "system_prompt": "Full instructions: sigil notation, built-in tools, response format, memory architecture guidelines.",

  "user_request": "The user's raw input for this turn. If an AskUser loop has occurred, the question and answer are appended here as plain text, with the full accumulated text forming a single user request.",

  "prefetched_tokens": [
    {
      "token_id": "WF_InteractiveFriendlyChat",
      "score": 0.87,
      "short_desc": "Friendly conversational flow for casual user queries"
    },
    {
      "token_id": "USR_T1_003",
      "score": 0.71,
      "short_desc": "User mentioned interest in Sydney Harbour fishing last session"
    }
  ],

  "preserved_tokens": [
    {
      "token_id": "CONCEPT_FishingLocation",
      "short_desc": "Concept node connecting location-relevant memory"
    },
    {
      "token_id": "TRT_FishingSpots_T3",
      "short_desc": "Fishing location suggestions generated turn 3"
    }
  ]
}
```

**Transport note (runtime v1, added by `protocol-doc-sync`):** this illustrative wire format is a conceptual/reference shape, not what is literally sent over the network in runtime v1. Because the runtime's LLM transport is provider-native tool-calling (a `ChatRequest` with a `messages` array and a `tools` array), the system prompt is carried as a `ChatMessage` with `role: System` — a first-class field the transport already supports — rather than duplicated as a `system_prompt` key inside a JSON blob passed as user content. The per-turn JSON body that *is* sent as the user-role message content contains exactly `user_request`, `prefetched_tokens`, and `preserved_tokens` — no `system_prompt` key. See `SQUIRE_SYSTEM_PROMPT`/`build_turn_input` in `squire.rs`, and the reconciliation note under §8.1's sibling doc, `context_squire_system_prompt_v2.md`, for the full rationale (this was an explicit, judged adaptation, not drift — see `decisions.md`).

`prefetched_tokens`/`preserved_tokens` entries also carry a `type` field in runtime v1 (visible in `context_squire_system_prompt_v2.md`'s version of this example, omitted from this spec's example above) — kept here unedited as a minor illustrative omission rather than a behavioral divergence, since `TokenSummary`'s `token_type` field has been present since `squire-adapter` landed.

**prefetched_tokens**: Squire's semantic match results against the current user request. Ordered by score. Short description only.

**preserved_tokens**: Token IDs the AI explicitly requested to carry forward from the previous turn. These bypass semantic scoring and always appear. Short description only — AI calls `token_to_detail()` if it needs full content.

### 8.2 Response Format (AI → Squire)

```json
{
  "ask_user": "Optional. If populated, generation ends here. Squire surfaces this question to the user, collects the answer, appends both to the user_request, and re-submits the turn. Cannot coexist with content. IMPLEMENTATION STATUS (runtime v1): the surface/collect/resubmit loop described here is NOT implemented - see the note under Section 9.3. Populating this field currently ends the turn as a hard error.",

  "content": "The AI's response to the user. May contain §!TokenID references and §^TokenID span§^ markers. Squire expands §! references before display and processes §^ spans at ingestion.",

  "preserve": [
    "CONCEPT_FishingLocation",
    "TRT_FishingSpots_T3"
  ],

  "new_tokens": [
    {
      "id": "CONCEPT_FishingLocation",
      "type": "concept",
      "short_desc": "Concept node connecting location-relevant memory",
      "full_desc": "Central concept linking all resources related to fishing locations: spots, tidal data, access routes, and seasonal conditions."
    },
    {
      "id": "TRT_FishingSpots_T3",
      "type": "referential",
      "short_desc": "Fishing location suggestions generated turn 3"
    }
  ],

  "relationships": [
    {"subject": "TRT_FishingSpots_T3",  "predicate": "instanceOf",  "object": "CONCEPT_FishingLocation"},
    {"subject": "TRT_FishingSpots_T3",  "predicate": "requires",    "object": "TOOL_WeatherCheck"},
    {"subject": "CONCEPT_FishingLocation", "predicate": "relatedTo", "object": "WF_InteractiveFriendlyChat"}
  ]
}
```

**Notes on fields:**

`ask_user` and `content` are mutually exclusive. A response with both populated is rejected.

`preserve` is a flat list of token IDs. These tokens will be loaded at the start of the next turn regardless of semantic score. Squire presents them in the `preserved_tokens` field of the next request.

`new_tokens` entries for referential types created via §^ do not require `full_desc` — the content is the §^ span text, already captured during parsing. Concept tokens should include `full_desc` if the AI wants `token_to_detail()` to return more than the short description.

`relationships` may reference tokens defined in the same `new_tokens` list. Squire inserts tokens first, then relationships.

### 8.3 Validity Rules and Rejections

```json
{
  "rejected": true,
  "reason": "description of the violation"
}
```

| Condition | Rejection reason |
|---|---|
| `ask_user` and `content` both populated | `"ask_user and content cannot coexist"` |
| `content` populated but no `new_tokens`, `relationships`, or `preserve` and content is empty | `"empty close response"` |
| `§!TokenID` in content where token is not in store and not in `new_tokens` | `"undisplayable token §!TokenID"` |
| `§^` opened but never closed | `"unclosed §^ span TokenID"` |
| `invoke()` called with a token_id whose full_desc is not a valid MCP schema | `"non-invocable token TokenID"` |

On rejection the Squire re-submits the same turn with the rejection payload appended to the conversation. On exhausting the configured retry limit (**runtime v1**: implemented, see rejection-ux), the Squire does **not** discard the turn silently — it persists a visible chat message containing the rejection reason and the model's full failed response (so the user can inspect what was produced and adjust their next prompt), and separately records structured diagnostic metadata (rule id, free-text reason, retry count, timestamp) for debugging. Nothing about the turn is stored as a compliant Squire response (no tokens/relationships/preserve-list updates happen), but the failure itself — and the content that caused it — is not thrown away.

**Additional rejection reasons beyond the table above (runtime v1):** two more conditions produce a rejection/retry cycle through the same mechanism, though they are not table-driven `validate_squire_response` checks:

| Condition | Rejection reason |
|---|---|
| AI response is not parseable as the Squire response JSON shape at all | `"response is not valid Squire protocol JSON: {parse error}"` |
| `ask_user` is populated (see note below — the response-field AskUser loop is not yet wired to a UI round-trip in runtime v1) | a fixed diagnostic string; **this currently ends the turn as a hard error, not a retry/compliance-failure cycle** — see the AskUser implementation-status note in §9.3 |

**Implementation-status notes below (runtime v1), not part of the original spec's scope, added by `protocol-doc-sync`:**
- §9.3's AskUser response-field loop is unimplemented — see the note under §9.3.
- `num_hops` graph traversal, `accumulated_hits`/`effective_priority` scoring, user-input auto-chunking, and raw-partition audit storage are also unimplemented — see the notes under §3.3, §4.1, §4.2, §6.1, §7.1, §9.1, and §10.1 respectively.

---

## 9. Turn Lifecycle

### 9.1 Turn Open

1. Squire receives user input.
2. Squire auto-chunks the input by natural language structure. Creates USR_TN_NNN tokens for each chunk. No relationships are written. **Implementation status (runtime v1): not implemented — see §4.3.** `build_turn_input` passes the latest user message's raw text straight through as `user_request`; no chunking or `USR_` token creation occurs.
3. Squire runs vector search against the structured partition using the user input as query. Returns top-N candidates with scores and short descriptions. **Implementation status (runtime v1): implemented** (`explore_memory("all", user_text, 1, 10)`, called unconditionally at the start of `build_turn_input`).
4. Squire loads the preserved token list from the previous turn's close. Looks up short descriptions. **Implementation status (runtime v1): implemented** (`preserved_tokens(session_id)`).
5. Squire assembles the request JSON: system prompt, user request (anchored), prefetched_tokens, preserved_tokens. **Implementation status (runtime v1): the system prompt is sent as a separate `ChatMessage::System`-role message, not a `system_prompt` field inside this JSON** — see the transport note under §8.1. The JSON body itself contains exactly `user_request`/`prefetched_tokens`/`preserved_tokens`, matching this step for those three fields.
6. Squire sends to Main AI.

### 9.2 During Turn — Tool Call Loop

The Main AI may call built-in tools at any point during generation. Each tool call is synchronous — the call fires, Squire executes it, the result is returned in the same generation turn.

The AI may call tools in any sequence and any number of times. There is no forced round-trip for tool calls. The generation continues after the result is returned.

**Typical early-turn pattern:**
```
AI sees prefetched tokens
→ explore(workflow, "friendly casual chat", 1)    # find better workflow match
→ token_to_detail("WF_InteractiveFriendlyChat", "full")  # read workflow instructions
→ explore(tools, "weather location", 1)           # discover relevant tools
→ token_to_detail("TOOL_IPLocation", "full")      # read tool schema
→ invoke("TOOL_IPLocation", {})                   # get user location
→ [generate response content]
```

### 9.3 AskUser Loop

**If AskUser is registered as a tool:**  
The AI calls `invoke("TOOL_AskUser", {"question": "..."})`. The Squire surfaces the question to the user, collects the answer, and returns it as the tool result. Generation continues. The loop is within the single turn.

**Implementation status (runtime v1): implemented**, via the ordinary tool-call loop — no Squire-specific code is needed for this path since `invoke()` already proxies to the tool registry.

**If AskUser is a response field:**  
The AI emits `ask_user` in its response with no `content`. Squire surfaces the question to the user and waits. The user's answer is collected. Squire appends the question and answer as plain text to the accumulated `user_request`. The full turn is re-submitted with the extended user request and the same prefetched/preserved context. This repeats until the AI emits a response with `content` and no `ask_user`.

The AI's question and the user's answer are indistinguishable from additional user input. No special namespace is required. The AI sees the full accumulated conversation in `user_request`.

**Implementation status (runtime v1): NOT implemented. This is a known, tracked gap (`squire-adapter/todo.json` sa-5), not a silent omission.** `SquireContextAdapter::finalize_turn` detects a populated `ask_user` field and immediately returns `Err("Squire ask_user response field is not yet wired to a UI round-trip...")` — this is a hard orchestration error (ends the turn with `stream-error`, same as any other unexpected failure), not the retry/compliance-failure cycle described in §8.3, and definitely not the surface-question/collect-answer/resubmit loop described above. There is no IPC command to surface a mid-turn question to the frontend and no UI to collect an answer and route it back. Until sa-5 is implemented, only the tool-registered AskUser path above actually works for Squire-mode sessions.

### 9.4 Turn Close

The turn closes when the AI emits a response with `content` populated and `ask_user` empty.

Squire performs the following in order (**implementation status per step, runtime v1, added by `protocol-doc-sync`**):

1. **Validate** inline token references in `content`. Reject if any `§!TokenID` is unresolvable. — *Implemented* (`validate_squire_response`), plus the malformed-JSON and unclosed-span checks documented in §8.3.
2. **Parse §^ spans** in `content`. For each span: extract token ID and text content, store chunk in LanceDB structured partition, create or update the referential token record. — *Implemented* (`upsert_token` with span text as `full_desc`), without a distinct `chunk_ref`-style pointer — see the §5.2 note.
3. **Expand sigils** for display: replace `§!TokenID` with `short_desc`, remove `§^` markers. Output clean prose to user. — *Implemented* (`expand_for_display`).
4. **Store raw response** in LanceDB raw partition. Not tokenized. Audit log only. — *Not implemented* — see §4.1.
5. **Process `new_tokens`**: insert each token if not exists (set `creation_turn` to current turn), increment `accumulated_hits` by 1 regardless. — *Partially implemented*: `creation_turn`/upsert is implemented; the `accumulated_hits` increment is not (see §3.3).
6. **Process `relationships`**: insert each triplet into the triplet store. — *Implemented* (`insert_relationship`); nothing reads them back yet (see §4.2).
7. **Scan content for §! references**: increment `accumulated_hits` by 1 for each referenced token found. — *Not implemented* — see §3.3.
8. **Store `preserve` list** for next turn bootstrap. — *Implemented* (`set_preserve_list`, wholesale replace semantics — see Q7 and `squire-storage`/`rejection-ux` decisions.md for the full next-turn-only + restart-clear lifecycle).
9. **Increment turn counter**. — *Implemented* (`increment_turn`).

---

## 10. Bootstrap Construction

The bootstrap is the initial context assembled at turn open before any AI tool calls. It is the Squire's best guess at what will be useful. After the first request is sent, the Squire is a slave to the AI's tool calls and the bootstrap is not rebuilt mid-turn.

### 10.1 Steps

1. Run user input through vector search against structured partition. Retrieve top-N scored tokens. — *Implemented* (`explore_memory("all", user_text, 1, 10)` in `build_turn_input`; top-N is hardcoded to 10 in runtime v1 rather than driven by the `bootstrap_top_n` config value in §15 — no config-file/settings plumbing for Squire tuning constants exists yet).
2. For each matched token, retrieve depth-1 connected tokens via the triplet store. Add to candidate set. — *Not implemented* — see the traversal note under §4.2/§6.1.
3. Load preserved tokens from last turn. These bypass scoring and are always included. — *Implemented*.
4. Score remaining candidates by `effective_priority` descending, edge count as tiebreaker. — *Not implemented as specified* — see §3.3; runtime ranks by cosine-similarity-plus-substring-boost instead, with no edge-count tiebreak (there being no traversal to produce edge counts from).
5. Trim candidates to the configured bootstrap token limit (character-based estimate ÷ 4). — *Not implemented*: runtime truncates by result *count* (`max_results`, default 10), not by a character-budget estimate of combined `short_desc` length.
6. Assemble `prefetched_tokens` from trimmed candidates. Include `token_id`, `score`, and `short_desc` only. — *Implemented*, plus a `type` field on each entry (present in the system-prompt doc's version of this shape, not in this spec's §8.1 example — see the §8.1 note).
7. Increment `accumulated_hits` for all tokens in the prefetched list. — *Not implemented* — see §3.3.

### 10.2 Token Limit

The bootstrap token limit is a configurable cap on the total content size of the prefetched context. Full content is not loaded at bootstrap — only short descriptions. The limit applies to the combined character count of all short descriptions in the prefetched list.

The AI calls `token_to_detail()` to load full content as needed. This is deliberate: the bootstrap surfaces candidates cheaply, and the AI decides what to actually read.

---

## 11. Squire Responsibilities

The Squire is a dumb script. Its responsibilities are precisely bounded.

**Implementation status summary (runtime v1, added by `protocol-doc-sync`):** items below marked with a trailing note are gaps against this design — see the section reference for detail. Everything else in this list is implemented as described.

**Turn open:**
- Auto-chunk user input → USR_ tokens, no relationships — **not implemented, see §4.3/§9.1**
- Vector search → prefetch candidate list
- Load preserved tokens from last turn
- Assemble and send request JSON

**During turn:**
- Execute built-in tool calls: `explore()`, `token_to_detail()`, `invoke()`
- Handle AskUser response field loop if triggered — **not implemented, see §9.3**
- Validate each response before acting on it
- Return rejection payloads on invalid responses

**Turn close:**
- Parse §^ spans and create referential tokens
- Expand §! references and §^ spans for user display
- Store raw response in LanceDB raw partition — **not implemented, see §4.1**
- Process new_tokens and relationships
- Increment §! hit counts — **not implemented, see §3.3**
- Store preserve list

**Session end:**
- Flush all in-memory state to disk — **not applicable to runtime v1's storage model: `LanceDbSquireStore` writes are immediately durable (LanceDB is a persistent, disk-backed store, not an in-memory structure requiring an explicit flush-on-exit step); there is no separate session-end flush operation, nor does one need to exist. `InMemorySquireStore` (the pre-`squire-storage` test double) never persisted across restarts by design, so a flush step would not have helped it either.**

**Never:**
- Build relationships between tokens
- Classify or categorise resources
- Make retrieval decisions
- Reason about content

---

## 12. Main AI Responsibilities

### 12.1 Exploration

The AI uses `explore()` to navigate its own memory. It inspects short descriptions to decide which candidates to expand. It calls `token_to_detail()` only on tokens it actually needs. It uses `num_hops` to widen the search when shallow results are insufficient.

### 12.2 Ingestion

When the AI receives external world content via tool results, it decides whether to remember it. If yes, it marks the relevant span in its response with `§^`, creates a referential token in `new_tokens`, and writes relationships connecting it to existing concept tokens. This is the act of ingestion. Content not marked this way is gone at turn end.

### 12.3 Memory Architecture

The AI is responsible for the quality and structure of the memory graph. Well-designed concept tokens with many edges make future retrieval precise. Poorly connected graphs force the system to fall back to pure vector similarity. When the AI creates a concept token and writes relationships, it is designing its own future retrieval routes.

Key practices:
- Create concept tokens for recurring ideas, entities, and categories
- Connect new referential tokens to relevant concept tokens immediately
- Use `preserve` to carry forward tokens that will be relevant next turn
- Mark spans with §^ proportionally to their future retrieval value — not everything needs to be structured memory

### 12.4 Tool Discovery

The AI discovers tools, skills, and workflows via `explore()` as needed. It reads their full schema via `token_to_detail()` before invocation. It invokes them via `invoke()`. The Squire is transparent as a gateway — the tool schema the AI receives is identical to standard MCP format.

---

## 13. Extended Resource Model

### 13.1 Workflow Selection

A workflow is a token whose full description tells the AI how to behave for a class of task. The AI selects a workflow by exploring, reading it, and following its instructions. Workflow selection is advisory — the AI decides whether a found workflow fits the current task.

Workflows may define AskUser as a built-in tool, specify how to structure output, require certain other tools to be discovered first, or define any other pattern of behaviour. They are instruction sets, not hardcoded modes.

### 13.2 Secondary AI as Tools

There is no hardcoded secondary AI for cleanup, context management, or relationship building. If such a capability is needed, it is registered as a tool — a workflow token describing a cleanup pattern, or a tool token whose schema invokes a secondary model. The AI discovers it via `explore(tools, "context management")` the same way it discovers any other capability. The architecture does not need to know it exists.

This means the system is extensible without redesign. New AI-powered capabilities are registered as tokens and become discoverable immediately.

---

## 14. CLI

**Implementation status (runtime v1, added by `protocol-doc-sync`):** the actual product is a Tauri desktop chat app, not a CLI read-evaluate-print loop — this section describes the original reference interface concept. The behavioral rules below (display expansion, rejection handling) still apply to the real UI's chat pane; only the literal "printed to a terminal" framing is aspirational/illustrative rather than what ships. `AskUser (response field path)` in particular describes a loop that is not implemented at all in runtime v1 — see §9.3's implementation-status note.

The CLI is the user-facing interface — a standard read-evaluate-print loop.

**Normal flow:**
```
User types input
  → Squire opens turn
  → tool call loop
  → Squire closes turn
  → display expanded clean prose
  → repeat
```

**AskUser (response field path):**  
The question is printed as a prompt. The user's answer is read as the next input line. The sub-loop is not visible to the user — they see a question and type an answer.

**Display expansion:**  
Before printing to the user, Squire: expands `§!TokenID` to the token's short description, removes `§^` markers, and prints clean prose. No protocol artefacts are ever visible.

**Rejection handling:**  
Squire rejections are handled silently. The retry is not visible to the user. On retry exhaustion (runtime v1: implemented), the Squire does not merely print a generic error and discard the turn — it surfaces a live error indicator to the user (the chat app's existing error-banner mechanism) *and* persists a durable, inspectable message containing the rejection reason and the model's full failed response, plus a structured diagnostic record. See §8.3 for the complete, current behavior.

---

## 15. Configuration

```python
bootstrap_token_limit   = 2000    # character-based estimate of prefetch budget
bootstrap_top_n         = 20      # max candidates from vector search before trim
summary_top_n           = 10      # top N tokens shown in session summary (if implemented)
max_retries             = 3       # rejection retry limit per turn
embedding_model         = "all-MiniLM-L6-v2"
main_ai_model           = "claude-sonnet-4-6"
session_dir             = "./session"
```

---

## 16. Deferred Components

The following are part of the broader design but explicitly out of scope for this version.

**Cleanup mode:** Triggered by ratios of total text segments to total tokens, average edges per token, and explore hit rate. The AI enters a structured cleanup workflow to categorise, merge, and prune the resource graph. Not implemented.

**Cross-session continuity:** Session state is written to disk at session end. Reloading a prior session is not implemented. Each session starts fresh.

**Sandboxes:** Isolated execution environments the AI can request for testing or research. Not implemented.

**Pipeline resources:** Pipelines are complex multi-step workflows with prompt injection, looping, and processing hooks. They are a workflow subtype and follow the same discovery mechanism, but their internal execution model is not specified here.

---

## 17. Glossary

**Implementation-status legend for this table (runtime v1, added by `protocol-doc-sync`):** `[not impl.]` marks terms whose defined mechanism does not exist in the runtime today — see the cross-referenced section for detail. Unmarked terms are implemented as defined.

| Term | Definition |
|---|---|
| Token | The fundamental unit of memory. A handle to information stored in the system. |
| Concept Token | A pure semantic node with no text body. Exists to connect things in the graph. |
| Referential Token | A named pointer to a text chunk. Created by AI via §^ marking. |
| System Referential Token `[not impl., §4.3/§9.1]` | Auto-generated by Squire from user input chunks. Prefixed USR_. |
| Resource Token | A workflow, tool, or skill stored as a token with a type-enforced full description. |
| §! | Inline token reference in AI output. Expands to short_desc at display. Increments hit count on context load `[hit-count part not impl., §3.3]`. |
| §^ | Named span marker. Opens and closes a region of AI output to be stored as a referential token. |
| explore() | Built-in tool. Vector search followed by graph traversal `[traversal not impl., §4.2/§6.1]`. Returns token list with scores and short descriptions. |
| token_to_detail() | Built-in tool. Returns short or full description of a token. |
| invoke() | Built-in tool. Proxies a tool call through Squire as MCP gateway. |
| Preserve list | Token IDs the AI carries forward to the next turn. Bypass semantic scoring. Next-turn-only, cleared on app restart (Q7). |
| Bootstrap | The initial prefetched token list assembled by Squire at turn open. |
| Effective priority `[not impl., §3.3]` | `accumulated_hits - (current_turn - creation_turn)`. Computed lazily at sort time. |
| Structured partition | LanceDB partition for tokenized content. Searchable and graph-connected. |
| Raw partition `[not impl., §4.1]` | LanceDB partition for unmarked AI output. Vector search only. Audit log. |
| External world | Everything not indexed in memory: files, web, APIs. Discovered via tools. |
| Ingestion | The act of bringing external content into structured memory via §^ marking and relationship writing. |
| Display boundary | The point at which Squire expands sigils and strips markers before showing output to the user. |
| Turn | The full lifecycle from user input to clean response display. |
| Squire | The Context Squire. Dumb script responsible for storage, retrieval, tool execution, and display expansion. |
