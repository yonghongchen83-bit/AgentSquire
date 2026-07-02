# Prompt

Close one of the four still-unclaimed backlog items `protocol-doc-sync` flagged and
`handoff.md`/`state.md` have carried forward ever since: **user-input auto-chunking into
`USR_TN_NNN` tokens** (spec §3.1/§4.3/§9.1 step 2/§11, glossary).

> **User input:** Squire auto-chunks by natural language structure. Generates USR_TN_NNN
> tokens per chunk. No relationships are auto-generated. All relationship building is left
> to the AI.
>
> **Implementation status (runtime v1): not implemented.** `SquireContextAdapter::build_turn_input`
> takes the raw text of the most recent user message directly as `user_request` — no
> auto-chunking occurs, and no `USR_`-prefixed (or any) tokens are created for user input.
> This is a genuine gap, not an intentional deferral captured anywhere in the planning
> decisions; flagged here. See `decisions.md`.

Deliverables:

- Read `../context_squire_spec_v2.md` (protocol-doc-sync-reconciled) in full for every place
  this gap is mentioned: §3.1 (System Referential Token shape/example), §3.2 (token record
  fields, including `chunk_ref`), §4.1 (structured vs. raw partition), §4.3 (Ingestion
  Rules — the primary description), §9.1 step 2 (Turn Open), §11 (Squire Responsibilities),
  §17 (glossary). Resolve, using your own judgment, exactly what triggers chunking (every
  user message, or only long ones?), what the `NNN` numbering scheme is (sequential per
  session? per turn?), what "chunk" means (true sub-message splitting by some
  size/semantic boundary, vs. one token per whole message), and how these tokens are meant
  to be referenced/used afterward. Document every judgment call in `decisions.md`.
- Read `../protocol-doc-sync/decisions.md` (item 11, "User-input auto-chunking into
  `USR_TN_NNN` tokens") for exactly how this gap was first scoped/described, and whether it
  flagged any spec ambiguity.
- Read `../planning/decisions.md` (Q1-Q7) to confirm (as `protocol-doc-sync` already did)
  that no planning decision explicitly descoped this — it is unclaimed drift, not an
  intentional deferral.
- Read the actual code: `src-tauri/src/agent/squire.rs` in full — especially
  `SquireContextAdapter::build_turn_input` (the exact function the spec's own runtime-status
  note names) and `SquireStore::upsert_token`'s signature/semantics (already well understood
  from `tool-token-ingestion`'s and `retrieval-fidelity`'s work).
- Verify baseline first: `cargo build` + `cargo test --lib` from `src-tauri/` (expect clean,
  173/173 passing).
- Implement chunking: when a turn begins, take the user's latest message and create
  corresponding `USR_TN_NNN`-id token(s) in the active `SquireStore` before/as part of
  building the turn input. Keep it proportionate to what the spec actually asks for — do not
  over-engineer a semantic chunker if the resolved scheme turns out to be simpler.
- Add real unit tests for the chunking logic, matching the existing test style in
  `squire.rs`.
- Verify manually/e2e if practical (a free-tier test LLM provider is configured in
  `%APPDATA%\com.squirecli.app\config.toml`; WDIO+tauri-driver e2e setup is working per
  `ask-user-loop`/`session-creation-ux`). If a GUI/e2e spec would be a strictly weaker
  verification signal than a direct backend check (as `tool-token-ingestion` judged for its
  own similarly backend-only change), a headless integration harness or thorough unit tests
  are an acceptable substitute — use your judgment and document the choice.
- Update `../state.md`'s "Child Nodes" list and `../handoff.md` to reflect this session's
  work, current build/test status, and the remaining backlog (raw-partition storage, the
  endpoint-carrying `TokenDetail` extension, rf-13 — user-input auto-chunking now resolved).

Out of scope (do NOT change here):
- Raw-partition audit-log storage (separate, unclaimed backlog item)
- `retrieval-fidelity/todo.json` rf-13 (fuller hit-count-event fidelity)
- The endpoint-carrying `TokenDetail`/`invoke()` extension `tool-token-ingestion` deliberately
  left out of scope
- Any frontend/UI work — this is a pure backend turn-open write path, same category as
  `tool-token-ingestion`
- Auto-generating relationships for chunked tokens — the spec is explicit that "no
  relationships are auto-generated" for this feature; that remains entirely the AI's
  responsibility
