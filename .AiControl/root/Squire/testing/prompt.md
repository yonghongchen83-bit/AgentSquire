# Prompt

Establish a dedicated **testing/QA phase** for the Squire context-mode pipeline as a whole —
not a new feature or gap closure, but a consolidation and hardening pass over the test coverage
already built up incrementally across all 17+ completed implementation nodes
(`adapter-core`, `session-mode`, `squire-adapter`, `squire-storage`, `rejection-ux`,
`protocol-doc-sync`, `retrieval-fidelity`, `stream-sigil-fix`, `ask-user-loop`,
`tool-token-ingestion`, `session-creation-ux`, `user-input-chunking`, `raw-partition-storage`,
`session-ux-polish`, `hit-count-fidelity`, `token-detail-endpoint`, `memory-alias-fix`).

Every prior node added its own unit tests (both `SquireStore` backends per `../env.md`'s
non-negotiable parity convention), and several added headless integration harnesses
(`src-tauri/examples/*_e2e.rs`) or WDIO+tauri-driver specs (`e2e/specs/*.test.ts`). No node's
job was to step back and look at the suite as a whole. This phase's job is exactly that.

Deliverables:

- Read `../state.md`'s full "Child Nodes" numbered list and `../handoff.md` in full for the
  complete history of what each prior node built and verified — this is the map of what
  coverage already exists and where. Do not re-derive it from scratch by reading every child
  node's own files line by line; the parent's summaries are the intended entry point.
- Read `../env.md` and `../decisions.md` for the settled architecture facts and testing-
  methodology conventions this phase must respect rather than re-litigate: the two-backend
  parity rule, the "unit tests + optional headless example for backend-only changes, real
  WDIO/tauri-driver e2e only for genuine new frontend surface" tier system, the "no active
  cleanup/sweep" philosophy, and the known pre-existing failures list.
- Establish a baseline: run `cargo build`, `cargo build --bins`, `cargo build --examples`, and
  `cargo test --lib` from `src-tauri/` (expect clean, 223/223 passing per `../handoff.md`'s
  last recorded count — re-confirm the live number, since further nodes may have landed since).
  From the repo root, run `npx tsc --noEmit -p tsconfig.app.json` and `npm test -- --run`
  (expect the same 7 pre-existing `tools-panel.tsx` TS errors and 2 pre-existing frontend test
  failures documented in `../env.md` — confirm these are still the *only* failures, not new
  ones).
- Inventory existing coverage across the pipeline's four stages the task calls out — adapter
  (`ContextManagerAdapter` trait, `LegacyContextAdapter`, `SquireContextAdapter`), storage
  (`SquireStore` trait, `InMemorySquireStore`, `LanceDbSquireStore`'s six tables), retrieval
  (`explore`/`token_to_detail`/`invoke`, graph traversal, hit-count/effective-priority scoring),
  and streaming (`streaming_cmd.rs`'s per-turn orchestration, live-chunk gating, the
  ask-user pause/resume loop) — and identify concrete, real gaps: gaps in negative-path/error
  coverage, gaps in one backend having a test the other lacks (parity drift), any e2e spec that
  only covers the happy path, and any component the repo-wide frontend greps from prior nodes
  confirmed has zero test surface at all.
- Note the uncommitted working-tree changes present at this phase's start (`git diff --stat`
  from repo root) touching `context_adapter.rs`, `context_adapter_test.rs`, `squire.rs`,
  `streaming_cmd.rs`, `openai.rs`, `chat-store.ts` — read what they actually change before
  assuming they're in scope; if they represent real, uncommitted behavior changes to the
  pipeline, this phase's coverage inventory and any new/updated tests should account for
  them, not silently test against a stale mental model of the code.
- Close real, concretely-identified gaps with new or strengthened tests, following the
  existing conventions exactly (two-backend parity for any `SquireStore`-touching test,
  proportional tiering for e2e vs. unit vs. headless-integration coverage — do not invent a
  new testing tier or tooling choice without a documented reason the existing ones don't fit).
  Prefer closing a small number of concretely-identified, real gaps over a speculative
  wall-to-wall rewrite of the existing suite.
- Audit `e2e/specs/*.test.ts` for the flakiness pattern `session-ux-polish` already found and
  fixed once (`../env.md`'s "recurring e2e flakiness pattern" note: non-polling
  `expect(...).toBe(...)` immediately after a UI action) — that node explicitly left the rest
  of the suite unaudited. Fix any further instances found, using the same polling
  `browser.waitUntil(...)` idiom already established.
- Document every decision about what was and wasn't worth closing in `decisions.md` before
  writing test code — in particular, any case where "add more tests" was considered and
  deliberately declined as disproportionate, per the epic's established proportionality
  practice (`../decisions.md`'s "Proportionality is the epic's central recurring judgment
  call" section).
- Update `../state.md`'s "Child Nodes" list and `../handoff.md` to reflect this phase's work,
  the final build/test counts, and any newly-identified-but-declined follow-up gaps.

Out of scope (do NOT change here):
- Any new product feature or gap closure — this phase is testing/verification only. If while
  auditing coverage a genuine, unimplemented functional gap is discovered (not a test-coverage
  gap), flag it in `decisions.md`/`state.md` as a newly-observed gap for a future node, do not
  fix it here.
- The nested-`§!`-citation residual `hit-count-fidelity` flagged and `memory-alias-fix` closed
  out per direct user instruction — this is a permanent, intentional simplification, not
  backlog: do not add tests asserting the opposite behavior or otherwise re-open it.
- Rewriting or restructuring already-passing, adequately-covered tests purely for style —
  only touch existing tests where a concrete coverage or parity gap was identified.
- The two known pre-existing frontend issues (`tools-panel.tsx` type errors, the two
  intermittently-failing frontend tests) — these predate the Squire epic entirely; do not fix
  them under this phase's scope unless a fix is trivial and directly unblocks a new test this
  phase needs to add.
