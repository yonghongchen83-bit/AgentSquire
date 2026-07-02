# Env

- Parent node: root/Squire
- Node path: root/Squire/hit-count-fidelity
- Objective: close `retrieval-fidelity/todo.json` rf-13 — wire the remaining 2 of spec §3.3's
  4 hit-count-increment events ("Token appears in explore() results that AI acts on" and "§!
  reference found in a chunk loaded into context") more faithfully than the single narrow
  `token_to_detail`-call proxy `retrieval-fidelity` used for both, without over-building a
  full context-composition audit trail disproportionate to the benefit.
- Scope: `SquireStore`/`SquireContextAdapter` call-site wiring only — no new trait methods
  expected (reuses the existing `record_hit`), no changes to `explore_memory`'s
  ranking/traversal mechanics (`retrieval-fidelity`'s scope, already complete and not
  re-litigated here), unit tests for the newly-wired event(s) in both backends.
- Non-goal: the endpoint-carrying `TokenDetail`/`invoke()` extension `tool-token-ingestion`
  deliberately left out of scope; the `"memory"`-alias/`system_referential` gap
  `user-input-chunking` flagged; any frontend/UI work (pure backend scoring logic, no
  user-facing surface — same category as `retrieval-fidelity`/`raw-partition-storage`).
- Depends on: `retrieval-fidelity` (`accumulated_hits`/`effective_priority`/`record_hit`,
  and its own decisions.md's exact reasoning for why events 1/3 were proxied rather than
  wired directly — the starting point this node must engage with), `squire-adapter`
  (`SquireContextAdapter::finalize_turn`, sigil parsers).
- Status: completed, 2026-07-03.

## Durable facts (read this session)

- Spec §3.3's exact 4 hit-count-increment events (see `context_squire_spec_v2.md` §3.3):
  1. "Token appears in explore() results that AI acts on" (+1)
  2. "Token in preserve list loaded at turn open" (+1) — wired by `retrieval-fidelity` via
     `preserved_tokens()`.
  3. "§! reference found in a chunk loaded into context" (+1)
  4. "Token listed in new_tokens at turn close" (+1) — wired by `retrieval-fidelity` via
     `upsert_token()` (every call, both create and update paths).
  Events 1 and 3 are this node's subject. `retrieval-fidelity/decisions.md` treats them as
  "really the same underlying mechanism restated in two places" per spec §6.1's own gloss:
  "Squire increments accumulated_hits for every token in the returned list that the AI
  subsequently acts on (calls token_to_detail or references in output)" — note the
  disjunction: "calls token_to_detail **or** references in output." The prior session wired
  only the first disjunct (`token_to_detail` calls); "references in output" was left
  unaddressed.
- `SquireContextAdapter::finalize_turn` (`src-tauri/src/agent/squire.rs`) already computes,
  before any of this node's changes: `let known: HashSet<String> = extract_inline_refs(&parsed
  .content).filter(|id| store.token_exists(id))` — i.e. every `§!TokenID` reference in the
  AI's own final response content that resolves to an existing store token. This is the
  natural, already-existing hook for crediting "references in output" (the second disjunct of
  event 1, and simultaneously the most literal reading of event 3's "§! reference... loaded
  into context" for the AI's own response content, which is unambiguously loaded into the
  user-visible/display context at turn close).
- `expand_for_display` (same file) is the other place `§!` is parsed today — it's what
  actually turns a `§!TokenID` marker into user-visible text via `token_detail`, one sigil at
  a time, during display expansion. It does not currently touch `accumulated_hits` at all.
- Neither `finalize_turn`'s `known`-computation nor `expand_for_display` today scans **inside**
  a `full_desc` body returned by `token_to_detail` for further embedded `§!` references (a
  "chunk citing another chunk" case) — this remains the one piece of `retrieval-fidelity`'s
  flagged residual gap this node found no proportionate way to close without a genuine
  context-composition audit pass; see decisions.md for the explicit tradeoff judgment.
- `SquireStore::record_hit(token_id: &str)` already exists (`retrieval-fidelity`) and is
  exactly the primitive needed here — no new trait method required.
