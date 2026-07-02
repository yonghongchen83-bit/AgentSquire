# Env

- Parent node: root/Squire
- Node path: root/Squire/raw-partition-storage
- Objective: close the "raw-partition audit-log storage" gap first flagged by
  `protocol-doc-sync` (item 12 in its decisions.md) and carried in `handoff.md`'s residual
  backlog ever since (the only backlog item left over from `protocol-doc-sync`'s 4
  newly-discovered gaps once `retrieval-fidelity` and `user-input-chunking` closed the other
  three). Give the AI's unmarked-at-close-time output the audit/debugging persistence the
  spec describes for it (§4.1/§4.3/§9.4 step 4/§11, glossary) — without conflating it with
  the ordinary chat-history table or with the structured memory-token partition.
- Scope: a new, append-only `SquireStore` trait method (`record_raw_output`, or equivalent)
  plus a sixth LanceDB table and an in-memory equivalent, wired into
  `SquireContextAdapter::finalize_turn` at the point where `parsed.content` and the parsed
  `§^` spans are both already available (immediately after `extract_spans`), persisting only
  the unmarked residual text (see Decisions for the precise definition) — not the full
  response verbatim, and not a duplicate of the chat-history message `finalize_turn` already
  writes via `ConversationStore`. Unit tests for the new storage path in both backends, in the
  same style as the five existing tables' test suites.
- Non-goal: any read-back/query path invoked by the model itself (spec is explicit:
  "Explore() does not search this partition by default" — this is a write-only,
  human/operator-facing audit log, not new AI-retrievable context); any UI to browse/search
  this data (no spec section calls for one); retention/rotation/size-capping policy (spec
  does not mention one; the four other append-only-style tables — relationships,
  compliance-failures — have none either, so none is added here either); `retrieval-fidelity/
  todo.json` rf-13 (fuller hit-count-event fidelity); the endpoint-carrying `TokenDetail`/
  `invoke()` extension `tool-token-ingestion` deliberately left out of scope; the
  `"memory"`-alias/`system_referential` gap `user-input-chunking` flagged; any frontend/UI
  work (pure backend turn-close write path, same category as `tool-token-ingestion`/
  `user-input-chunking`).
- Depends on: `squire-adapter` (`SquireStore` trait, `finalize_turn`, `extract_spans`),
  `squire-storage` (`LanceDbSquireStore`'s five-table storage-layout conventions — this node
  adds a sixth table following the same shape), `rejection-ux` (the closest existing precedent
  for an append-only, debugging-only table: `squire_compliance_failures`), `tool-token-
  ingestion`/`user-input-chunking` (the most recent precedent for the
  unit-tests-plus-headless-harness verification methodology this node also uses).
- Status: completed, 2026-07-03.

## Durable facts (read this session)

- The spec's raw partition (§4.1) is defined by contrast with the "structured partition":
  > **Structured partition** — referential tokens created by AI via §^ marking, and system
  > tokens from user input. These have token IDs and graph connections. This is the primary
  > memory partition.
  >
  > **Raw partition** — auto-stored AI output that was not explicitly marked with §^.
  > Reachable only by vector similarity. No token representation, no graph connections.
  > Treated as an audit log, not as memory. Explore() does not search this partition by
  > default.
  This is the load-bearing distinction this node resolves before implementing: the raw
  partition is **not** a verbatim, full-response audit trail redundant with the ordinary chat
  message history — it is specifically the portion of the AI's `content` that the AI chose
  **not** to promote into a `§^`-marked referential token. See decisions.md for the exact
  extraction method and the reasoning for rejecting the "store everything verbatim" reading.
- §4.3 restates the same distinction from the ingestion-rules angle: "AI owns chunking
  entirely via §^ sigils. If the AI does not mark a span, it is stored only in the raw
  partition. §^ marking is the act of memory creation." — i.e. marked vs. unmarked is a
  binary partition of the *same* response content, not two different bodies of text.
- §9.4 (Turn Close, the exact numbered sequence `finalize_turn` implements) step 4: "**Store
  raw response** in LanceDB raw partition. Not tokenized. Audit log only." — read together
  with step 2 ("Parse §^ spans... store chunk in LanceDB **structured** partition") and step 3
  ("Expand sigils for display... clean prose to user"), the numbered sequence treats "the raw
  response" and "the §^ span chunks" as separate destinations for what is fundamentally the
  same source text, split by whether a given portion fell inside a `§^...§^` span or not.
  Nothing in step 4's one-line wording says "store the entire response a second time
  regardless of §^ markup" — that reading would make step 2 and step 4 both archive the exact
  same marked-span text redundantly, which the spec's own two-partition framing in §4.1
  explicitly contradicts ("no token representation" vs. "these have token IDs").
- `SquireContextAdapter::finalize_turn` (`src-tauri/src/agent/squire.rs`) is the exact
  function every "Implementation status (runtime v1)" note for this gap names. It already
  computes, in this order: `parsed: SquireResponse` (parsed JSON, `parsed.content` is the raw
  sigil-laden text) -> `known` (validated `§!` refs) -> `validate_squire_response` (rejects
  malformed responses before this point) -> `turn = self.store.current_turn(session_id)` ->
  `let (spans, _) = extract_spans(&parsed.content)` -> a loop upserting `new_tokens` (merging
  span text into `full_desc` when present) -> `insert_relationship` -> `set_preserve_list` ->
  `increment_turn` -> `expand_for_display` -> `ConversationStore::append_message` (the
  ordinary, unrelated chat-history table). The raw-partition write belongs immediately after
  `extract_spans` runs (spans are needed to know what to exclude) and before/alongside the
  `new_tokens` loop — this node adds it there, only on the compliant/`TurnOutcome::Done` path
  (a rejected response never reaches this point; `reject_and_record` already gives a
  structured audit trail for non-compliant turns via `record_compliance_failure`).
- `ConversationStore::append_message` (called at the end of `finalize_turn`) persists the
  **display-expanded, sigil-stripped** `display_content` string to the ordinary chat-message
  history — this is a categorically different table (SQLite-backed `conversation_store`, not
  `SquireStore`/LanceDB) serving a categorically different purpose (what the user sees in
  their chat transcript). The raw partition is not a substitute or duplicate for this — it
  exists specifically to retain the *un-transformed, unmarked* source text that display
  expansion and span-parsing both discard, for operator/debugging audit purposes the ordinary
  chat transcript cannot serve (the chat transcript never shows raw `§!`/`§^` markup or
  content the AI explicitly declined to keep).
- `squire-storage/decisions.md`'s "Storage layout" section documents the existing five-table
  shape and the precedent most relevant here: `squire_compliance_failures` ("Append-only,
  debugging-only table... never queried for runtime decisions, only inspected for
  diagnostics"). The new raw-partition table follows the identical shape/spirit: append-only,
  never read back by `explore_memory` or any other trait method, plain string columns over
  more elaborate Arrow types (this table is not searched by the model, so no embedding column
  is needed — see decisions.md for why "reachable only by vector similarity" in the spec's
  wording does not require this node to build a search path nothing in this system ever
  calls).
- `SquireStore` trait methods (`src-tauri/src/agent/squire.rs`) are all narrow, single-purpose
  async methods mirroring one storage operation each (`upsert_token`, `insert_relationship`,
  `record_compliance_failure`, etc.) — the established convention this node's new method
  follows exactly, rather than overloading an existing method.
