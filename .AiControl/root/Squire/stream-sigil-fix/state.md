# State

## Timeline

- 2026-07-02: Node created as a sibling of `retrieval-fidelity`/`rejection-ux` under
  `root/Squire`, split out to close `squire-adapter/todo.json`'s sa-4 (raw stream leaks
  protocol sigils before `finalize_turn` expands them) — the one item from the six-item
  residual backlog this session was scoped to tackle. Repointed `.AiControl/.current` to
  this node.
- 2026-07-02: Read context: `squire-adapter/todo.json` sa-4's exact original wording,
  `squire-adapter/decisions.md`'s streaming/`finalize_turn`/validation-gate design notes
  and its "Known gaps deliberately left open" section (sa-4's own suggested fix:
  "not emitting `stream-chunk` at all in Squire mode and instead emitting the expanded
  content once at turn close"), `rejection-ux/decisions.md`'s "Two flagged follow-ups
  considered for this node, deliberately not absorbed" section (confirms sa-4 is a
  live-streaming-display-boundary bug independent of `TurnOutcome::Failed`/compliance-UX
  concerns). Read `context_squire_spec_v2.md` §5 (sigil notation: `§!TokenID` inline
  reference, `§^TokenID ... §^` named span) and §14 (display-boundary guarantee: "no
  protocol artefacts are ever visible to the user").
- 2026-07-02: Read the actual code in full: `src-tauri/src/agent/squire.rs`
  (`SquireContextAdapter::finalize_turn`, `expand_for_display`, `extract_inline_refs`,
  `extract_spans`, `strip_span_markers` — confirmed the expansion logic itself is
  correct and already tested; the bug is purely that raw content is *also* shown live,
  earlier, before this logic ever runs), `src-tauri/src/commands/streaming_cmd.rs` in
  full (found the exact leak site: `StreamEvent::Chunk(text) => { full_response.push_str(&text); app_clone.emit("stream-chunk", text); }`
  inside the streaming-receive loop, called unconditionally regardless of
  `session.session.context_mode`, plus two more `stream-chunk` emissions inside the
  `FinishReason::ToolCalls` branch — a `"\n\n"` separator and an
  `"[Executing {tool_name}...]\n"` progress line, both also unconditional), and
  `src/stores/chat-store/stream-listeners.ts` (confirmed the frontend's `onStreamChunk`
  handler blindly accumulates every received chunk into `streamingText`/`streamingBlocks`
  for live rendering — no context-mode awareness on the frontend side at all, confirming
  this must be a backend-only fix at the emission point, not a frontation-side filter).
- 2026-07-02: Verified baseline: `cargo build` and `cargo test --lib` both clean, 149/149
  passing — matches `handoff.md` exactly, no drift. (No `protoc`/PATH issue this session —
  build was warm from the prior `retrieval-fidelity` session.)
- 2026-07-02: Designed the fix in `decisions.md` before implementing: chose full
  live-stream suppression in Squire mode (the task's option 1, also sa-4's own literal
  prescribed fix) over incremental sigil-masking (option 2) or incremental deterministic
  expansion of partial streamed content (option 3) — both rejected as requiring new
  streaming-parser machinery disproportionate to a display-boundary bug fix. See
  decisions.md for the full reasoning, including why the two tool-loop-path emissions
  were also gated, not just the literal per-token chunk handler sa-4's todo text names.
- 2026-07-02: Implemented in `src-tauri/src/commands/streaming_cmd.rs`: added
  `should_stream_live_chunks(context_mode: ContextMode) -> bool` (pure function, `false`
  only for `ContextMode::Squire`); `send_message_impl` computes `stream_live_chunks` once
  per turn and gates three `app_clone.emit("stream-chunk", ...)` call sites on it: the
  main `StreamEvent::Chunk` handler, the tool-call `"\n\n"` separator, and the
  `"[Executing {tool_name}...]\n"` progress line after destructive-tool approval.
  `full_response.push_str(&text)` (the buffer `finalize_turn` eventually receives)
  remains unconditional in all cases — only the live UI-forwarding `emit` calls are
  skipped, nothing about what's accumulated/parsed/persisted changed.
- 2026-07-02: Added 2 unit tests in a new `#[cfg(test)] mod tests` block at the end of
  `streaming_cmd.rs` (the file had no test module before this session):
  `should_stream_live_chunks_true_for_legacy_mode`, `should_stream_live_chunks_false_for_squire_mode`.
- 2026-07-02: Full verification: `cargo build` and `cargo build --bins` both clean, zero
  warnings (~2 min each, warm build, no `protoc` issue this session). `cargo test --lib`:
  151/151 passing (149 baseline + 2 new). Ran the 2 new tests in isolation
  (`cargo test --lib should_stream_live_chunks`) to confirm both pass individually, not
  just as part of the full suite.
- 2026-07-02: Checked feasibility of manual in-app verification: `src-tauri/.squirecli/config.toml`
  has `llmProviders = []` (no configured LLM provider/API key in this environment), and
  this is a sandboxed CLI session with no way to launch and visually interact with a
  Tauri desktop window. Manual verification judged not practical here — documented per
  the task's explicit fallback instruction rather than skipped silently. Relied instead
  on: (1) the 2 new unit tests directly covering the mode-gating decision, (2) manual
  code-path tracing confirming `finalize_turn`'s inputs are byte-for-byte unchanged by
  this fix, and (3) the pre-existing `squire.rs` sigil-parsing/`expand_for_display` test
  coverage (unchanged, still passing), which together confirm both halves of the fix
  (the suppression and the unaffected correct destination) are covered.
- 2026-07-02: Updated `root/Squire/state.md`'s Child Nodes list and `root/Squire/handoff.md`
  to reflect this session's work, current 151/151 build/test status, and sa-4 removed
  from the open backlog (5 items remain: sa-5, ss-9, user-input auto-chunking,
  raw-partition audit storage, rf-13).

## Decisions
(See decisions.md for full rationale on the chosen fix, the two rejected alternatives,
why the two tool-loop-path emissions were also gated, and why manual verification wasn't
practical this session.)

## Risks

- Losing the live-typing UX during Squire-mode generation is an accepted, deliberate
  tradeoff (see decisions.md) — Squire-mode users will see `stream-status` line updates
  ("Contacting model...", tool-call progress, etc.) but no live prose until the turn
  closes and the finalized message appears via the existing `stream-done` →
  `getConversation` reload path. If a future session wants live-typing back for Squire
  mode, options 2/3 from the task (considered and rejected here as overengineering for
  *this* fix) become the relevant starting points — not tracked as a new backlog item
  since no requirement anywhere asks for it, just documented here as the natural next
  step if ever prioritized.
- `should_stream_live_chunks`'s `matches!(context_mode, ContextMode::Squire)` pattern
  defaults any hypothetical future third `ContextMode` variant to "streams live" (since
  it isn't `Squire`) rather than failing to compile / forcing an explicit decision. Not a
  real risk today (`ContextMode` has had exactly two variants since `session-mode`, no
  node has proposed a third), but flagged for whoever adds one, if ever.
- Manual, visual, in-running-app confirmation was not performed — this environment has no
  configured LLM provider and no way to interact with a Tauri GUI window. The fix is
  verified via unit tests and code-path tracing only. Recommended for whoever next runs
  this app locally with a real provider configured: send a Squire-mode message and
  confirm the chat pane shows no text at all during generation (only status-line updates),
  then the full clean-prose answer appears at once when the turn closes.

## Closure summary

sa-4 is resolved. Squire mode's raw, sigil-laden protocol JSON output is no longer
forwarded to the live `stream-chunk` UI channel during generation — `streaming_cmd.rs`'s
`should_stream_live_chunks(context_mode)` gates all three `stream-chunk` emission sites
(main per-token handler, tool-call separator, tool-execution progress line) to Legacy
mode only. Squire mode's finalized, sigil-expanded content still appears correctly and
immediately at turn close via the pre-existing `finalize_turn` → `ConversationStore` →
`stream-done` → `getConversation` reload path, completely unchanged by this fix. Legacy
mode's live-streaming behavior is byte-for-byte unchanged.

This was a targeted, one-shot fix exactly matching the scope and approach sa-4's own
todo text and `squire-adapter/decisions.md`'s original gap note already prescribed — no
new streaming framework, no changes to sigil-parsing/expansion logic (already correct,
already tested), no frontend changes (the frontend's `stream-chunk` handler is
unmodified; it now simply receives fewer events during Squire-mode turns).

All verification passed: `cargo build`/`cargo build --bins` clean with zero warnings,
`cargo test --lib` 151/151 (149 baseline + 2 new). Manual in-app verification was not
practical in this environment (no configured LLM provider, no interactive GUI access) —
documented, not silently skipped.

## Next Actions

- Node scope complete for its one stated deliverable (sa-4) — ready to be marked
  complete.
- Remaining Squire-epic backlog after this node: `squire-adapter/todo.json` sa-5
  (ask_user UI round-trip loop); `squire-storage/todo.json` ss-9 (real tool-token
  ingestion); user-input auto-chunking (`USR_TN_NNN` tokens); raw-partition audit-log
  storage; `retrieval-fidelity/todo.json` rf-13 (fuller hit-count-event fidelity). None
  of these block normal use of Squire mode.
- If live-typing UX for Squire mode is ever wanted back, see decisions.md's "Rejected"
  section for the two considered-but-deferred incremental-display approaches — not
  currently tracked as a todo item since nothing requires it.
- Whoever next runs the app locally with a real LLM provider configured should do the
  manual smoke test this session couldn't: send a Squire-mode message, confirm no raw
  sigils/JSON appear in the chat pane during generation.
