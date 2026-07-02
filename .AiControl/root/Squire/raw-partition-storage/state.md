# State

## Timeline

- 2026-07-03 Node created. Read `context_squire_spec_v2.md` §4.1/§4.3/§5.2/§9.4/§11/§17,
  `protocol-doc-sync/decisions.md` item 12, `squire-storage/decisions.md`'s storage-layout
  section, and the full `squire.rs`/`squire_lancedb.rs` source. Confirmed baseline:
  `cargo build` clean, `cargo test --lib` 193/193 passing.

- 2026-07-03 Resolved the central ambiguity: the raw partition is **not** a verbatim,
  full-response audit trail. It is specifically the AI-output text that was **not** enclosed
  in a `§^...§^` span at turn close — i.e. content the model produced but did not promote
  into a structured, addressable memory token. See `decisions.md` for the full textual
  argument (§4.1's two-partition framing, §4.3's "if the AI does not mark a span, it is
  stored only in the raw partition," §9.4's numbered step sequence treating "parse §^ spans"
  (step 2) and "store raw response" (step 4) as two destinations for the same source text
  split by markup, not two independent full copies).

- 2026-07-03 Resolved the read-back question: no read-back mechanism exists anywhere in the
  spec for this partition. §4.1's "Explore() does not search this partition by default" is
  read as descriptive of the one search surface the spec defines (`explore()`) rather than as
  a promise of some other, unspecified read path — no other tool or mechanism is described
  anywhere in the protocol for querying this partition. This is a write-only, operator/
  debugging-facing audit log, matching `squire_compliance_failures`'s existing "never queried
  for runtime decisions, only inspected for diagnostics" precedent exactly.

- 2026-07-03 Implemented: `SquireStore::record_raw_output` (new trait method, both
  `InMemorySquireStore` and `LanceDbSquireStore`), a new `squire_raw_partition` LanceDB table
  (session_id, turn, content, timestamp), and `unmarked_residual(content, spans) -> String`
  (pure helper in `agent::squire`) that strips every closed `§^...§^` span from `content`,
  leaving only the text the AI chose not to mark. Wired into `finalize_turn` immediately
  after `extract_spans` runs, only on the compliant path (a rejected response never reaches
  this point — `reject_and_record` already gives rejected turns a structured audit trail via
  `record_compliance_failure`). The write is skipped when the unmarked residual is empty (a
  fully-§^-spanned response has nothing left over to archive) to avoid pointless empty rows.

- 2026-07-03 Added unit tests: `unmarked_residual` pure-function tests (no spans -> whole
  content, one span -> everything outside it, fully-spanned -> empty, unclosed span -> spec's
  own §5.2 wording means an unclosed span never reaches `finalize_turn`'s compliant path at
  all, since `validate_squire_response` already rejects it — covered by an existing rejection
  test, not a new raw-partition one) in `squire.rs`; `InMemorySquireStore`-backed
  `record_raw_output`/read-for-test coverage; `finalize_turn` integration tests confirming
  only the unmarked text is persisted and a fully-marked response stores nothing. Mirrored
  coverage in `squire_lancedb.rs` against the real `LanceDbSquireStore` (schema round-trip,
  persistence across reopen, multiple turns).

- 2026-07-03 Full verification: `cargo build`/`cargo build --bins`/`cargo build --examples`
  all clean, zero warnings. `cargo test --lib`: **206/206 passing** (193 baseline + 13 new:
  10 in `squire.rs` — 6 pure-function `unmarked_residual` tests plus 4 `finalize_turn`
  integration tests against `InMemorySquireStore` — and 3 in `squire_lancedb.rs` against the
  real `LanceDbSquireStore`). Confirmed via repo-wide frontend grep
  (`raw_partition`/`record_raw_output`/"raw partition", case-insensitive, across `src/`) that
  this feature has zero frontend surface, matching `tool-token-ingestion`/
  `user-input-chunking`'s precedent — no WDIO/GUI spec built.

  Added `src-tauri/examples/raw_partition_storage_e2e.rs`, a headless, no-LLM-needed
  integration harness running the real `SquireContextAdapter::finalize_turn` call path
  against **both** real backends it touches — a real temp-directory `LanceDbSquireStore`
  (for the new raw partition and the pre-existing `squire_tokens` table) and a real
  temp-file SQLite `Database` (for the ordinary, unrelated `ConversationStore` chat-history
  table) — a step beyond `tool_token_ingestion_e2e.rs`/`user_input_chunking_e2e.rs`, which
  only needed the LanceDB side. Ran successfully; all assertions passed:
  - Turn 1 (mixed marked/unmarked compliant response): exactly one `squire_raw_partition`
    row was created, containing only the unmarked prose ("Sure thing. Let me know if you
    need more detail.") — the §^-marked span text ("The answer to your question is 42.")
    was correctly excluded from it and instead created a real `squire_tokens` row
    (`TRT_Answer`). The ordinary SQLite chat-history table received exactly one message
    with the normal display-expanded content (spans inlined, sigils stripped) — confirming
    this node's change is additive and does not alter the pre-existing chat-history path.
  - Turn 2 (fully §^-spanned response, nothing outside the span): the raw-partition row
    count stayed at 1 — no new row was written, confirming the empty-residual skip-write
    behavior against the real backend.
  - Turn 3 (malformed JSON, rejected): the raw-partition row count stayed at 1 — a rejected
    response writes nothing to the raw partition, confirming the compliant-path-only call
    site placement against the real backend (only the pre-existing
    `squire_compliance_failures` path fires for rejections, unchanged by this node).
  Full transcript above this line and reproducible via `cargo run --example
  raw_partition_storage_e2e` from `src-tauri/`.

## Decisions

See `decisions.md` for the full writeup: the verbatim-vs-unmarked-only textual argument, the
read-back-mechanism conclusion, the trait method/table design, the call-site placement, and
the empty-residual skip-write decision.

## Risks

- None identified that block this node. The unmarked-residual extraction reuses
  `extract_spans`'s already-tested span-parsing logic rather than re-implementing sigil
  parsing a second time, so there is no new parsing-correctness risk beyond what
  `extract_spans` itself already carries.

## Next Actions

- None — node complete.

## Closure summary

Closed `protocol-doc-sync/decisions.md` item 12 (raw-partition audit-log storage), the last
of the four newly-discovered gaps from that session's inventory to be resolved (graph
traversal and hit-count scoring were closed by `retrieval-fidelity`; user-input auto-chunking
by `user-input-chunking`). Added `SquireStore::record_raw_output` (new trait method, both
`InMemorySquireStore` and `LanceDbSquireStore`), a new sixth LanceDB table
(`squire_raw_partition`: session_id, turn, content, timestamp — no embedding column, matching
`squire_compliance_failures`'s existing append-only/debugging-only precedent), and a new pure
helper `unmarked_residual(content) -> String` in `agent::squire` that extracts exactly the
portion of a compliant turn's AI output falling outside every closed `§^...§^` span. Wired
into `SquireContextAdapter::finalize_turn` immediately after `extract_spans`, on the
compliant path only, skipping the write entirely when the residual is empty. All todo.json
items done. `cargo build`/`cargo build --bins`/`cargo build --examples`: clean, zero
warnings. `cargo test --lib`: 206/206 passing. Real end-to-end verification via
`src-tauri/examples/raw_partition_storage_e2e.rs` against real LanceDB + real SQLite
backends, all assertions passed. Status: completed, 2026-07-03.
