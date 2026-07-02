# Env

- Parent node: root/Squire
- Node path: root/Squire/user-input-chunking
- Objective: close the "user-input auto-chunking into `USR_TN_NNN` tokens" gap first flagged
  by `protocol-doc-sync` (item 11 in its decisions.md) and carried in `handoff.md`'s residual
  backlog ever since `retrieval-fidelity`. Give user-supplied turn input the same
  addressable/referenceable token identity that AI-created memory tokens already get
  (spec §3.1 "System Referential Token"), so it can participate in the same
  explore/reference/graph machinery — without inventing scope the spec doesn't ask for.
- Scope: a new backend-agnostic chunking function in `agent::squire` that splits/wraps the
  latest user message into one or more `system_referential`-typed `SquireStore` tokens with
  `USR_TN_NNN` ids, called from `SquireContextAdapter::build_turn_input` before the
  bootstrap `explore_memory`/prefetch step; unit tests for the chunking function's
  id-numbering/type/content-shape behavior in the same style as `squire.rs`'s existing test
  module.
- Non-goal: raw-partition audit-log storage (separate, unclaimed backlog item — §4.1);
  `retrieval-fidelity/todo.json` rf-13 (fuller hit-count-event fidelity); the
  endpoint-carrying `TokenDetail`/`invoke()` extension `tool-token-ingestion` deliberately
  left out of scope; any frontend/UI work (pure backend turn-open write path, same category
  as `tool-token-ingestion`); auto-generating relationships for chunked tokens (spec is
  explicit: "No relationships are auto-generated" for this feature — left entirely to the
  AI, same as any other token); true NLP-grade semantic segmentation (see Decisions for why
  a much simpler paragraph/sentence-boundary split satisfies "natural language structure"
  without over-building).
- Depends on: `squire-adapter` (`SquireStore` trait, `NewTokenSpec`, `build_turn_input`),
  `squire-storage` (`LanceDbSquireStore`'s `upsert_token`), `retrieval-fidelity`
  (`accumulated_hits`/`effective_priority` — chunk tokens participate in the same ranking
  once created), `tool-token-ingestion` (the most recent precedent for a pure backend
  free-function-plus-call-site addition to `agent::squire`, and for the
  unit-tests-plus-headless-harness verification methodology this node also uses).
- Status: completed, 2026-07-03.

## Durable facts (read this session)

- `SquireContextAdapter::build_turn_input` (`src-tauri/src/agent/squire.rs`) is the exact
  function the spec's own "Implementation status (runtime v1)" notes name at every mention
  of this gap (§4.3, §9.1 step 2). It currently does: find the latest `MessageRole::User`
  message's raw `content` string -> use it verbatim as `user_request` in the JSON body sent
  to the model, with no intermediate processing. This is the one and only call site that
  needs a new chunking step inserted before it builds `prefetched_tokens`/the request body.
- Spec §3.1 gives one worked example of a System Referential Token:
  ```
  id:          USR_T2_001
  type:        system_referential
  short_desc:  (first sentence of the chunk)
  chunk_ref:   (LanceDB chunk identifier)
  ```
  `T2` here reads as "turn 2" (matching `creation_turn`'s existing "turn number at first
  insertion" semantics, spec §3.2, and the adapter's pre-existing `current_turn`/
  `increment_turn` per-session turn counter) and `001` as a zero-padded sequence number
  *within that turn's set of chunks* — not a global per-session counter, since a per-turn
  reset is the only reading consistent with the example's own `T2` segment existing
  separately from the trailing `001` (if `NNN` were a global monotonic counter, the `T2`
  segment would be redundant with information already recoverable from `creation_turn`).
  See decisions.md for the numbering scheme finally chosen and why the task's own
  `USR_TN_NNN` shorthand (not `USR_T{turn}_{NNN}`, which is what the worked example actually
  shows) is treated as shorthand for the same thing, not a second literal format to support.
- `NewTokenSpec` (the only shape `upsert_token` accepts) has exactly four fields: `id`,
  `token_type`, `short_desc`, `full_desc: Option<String>` — there is **no** `chunk_ref`
  field anywhere in the runtime schema (confirmed already by `protocol-doc-sync`'s §5.2 note
  and unchanged since). This node does not add one — see decisions.md for why the chunk's
  own text is stored directly as `full_desc`, mirroring exactly how `finalize_turn` already
  handles AI-created referential tokens from `§^` spans (span text captured as `full_desc`,
  no separate `chunk_ref` pointer either — `tool-token-ingestion`'s and `retrieval-fidelity`'s
  sessions both already worked within this same "no `chunk_ref` field exists" constraint
  without adding one, and neither flagged it as something this node should fix).
- `token_type` in the runtime `SquireStore`/`TokenSummary`/`explore_memory` universe today is
  a free-form string, not a Rust enum — `"concept"`, `"referential"`, `"tool"`, `"skill"` are
  all just strings compared for equality/filtering. Spec §3.2 lists
  `system_referential` as one of six defined token type enum values; the runtime's
  `explore_memory`'s `type_matches` closure (both `InMemorySquireStore` and
  `LanceDbSquireStore`) already treats any `resource_type` value not specially handled
  (`"all"`, `"memory"`, `"tool_skill"`) as an exact-string type filter — so a new
  `"system_referential"` token type value needs zero trait/schema changes to become
  filterable via `explore(resource_type="system_referential", ...)`, exactly like
  `tool-token-ingestion`'s new `"tool"` type value needed none.
- `build_turn_input` runs once per turn *before* any tool-call loop, and — critically — is
  the function that also does the bootstrap `explore_memory("all", &user_text, 1, 10,
  current_turn)` call. Chunk tokens must be created and upserted into the store *before*
  that `explore_memory` call if they are to be discoverable via the same turn's bootstrap
  prefetch (spec §9.1 step 2 precedes step 3's vector search in the numbered turn-open
  sequence) — this ordering constraint is a hard requirement, not a style choice.
