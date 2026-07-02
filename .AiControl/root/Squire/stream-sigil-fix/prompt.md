# Prompt

Fix `squire-adapter/todo.json`'s **sa-4**, deliberately deferred by both `squire-adapter`
and `rejection-ux` as an orchestration/streaming-buffering concern outside either node's
scope:

> Raw model output (including §! and §^ sigils) is streamed live to the UI via
> stream-chunk before finalize_turn ever parses/expands it - Squire-mode protocol
> artefacts currently leak into the live stream. Needs orchestration-level buffering
> (don't emit stream-chunk in Squire mode; emit expanded display content once at turn
> close instead).

Concretely: in Squire mode, the model's raw output is protocol JSON containing
unexpanded `§!TokenID`/`§^TokenID ... §^` markers (and the JSON envelope itself). Today
`streaming_cmd.rs` forwards every `StreamEvent::Chunk` straight to the frontend's
`stream-chunk` IPC event as it arrives, regardless of context mode — so a Squire-mode
user watching the chat pane mid-generation sees raw, un-expanded protocol markup
instead of clean prose. Only once `finalize_turn` runs (at turn close) does the
sigil-expanded, clean-prose version get persisted and shown. Legacy mode is unaffected
(its raw content is always already display-ready text).

Deliverables:
- Fix the leak by fitting into the existing `context_mode` branch point in
  `streaming_cmd.rs` (the same place `dispatch_registry`/`adapter` already branch on
  `session.session.context_mode`) — not a new global streaming framework, not a change
  that affects Legacy mode's behavior at all.
- Pick the simplest approach that doesn't overengineer this. Read
  `squire-adapter/decisions.md` and `rejection-ux/decisions.md` (both explicitly discuss
  why sa-4 was deferred and what fix they each imagined) before choosing.
- Add real unit tests for whatever the fix's decision logic is, in the same style as
  existing test suites (`squire.rs`, `context_adapter.rs`).
- Manually verify in the running app if practical (Squire-mode message, watch the chat
  pane mid-stream for absence of raw sigils); otherwise document why it wasn't
  practical and rely on unit tests.

Reference: `../squire-adapter/todo.json` sa-4; `../squire-adapter/decisions.md` (streaming
design, `finalize_turn`/`expand_for_display`); `../rejection-ux/decisions.md` ("Two flagged
follow-ups considered for this node, deliberately not absorbed"); `../context_squire_spec_v2.md`
§5 (sigil notation), §9.4 step 3 (`expand_for_display`), §14 (display-boundary guarantee:
"no protocol artefacts are ever visible to the user"); `src-tauri/src/agent/squire.rs`
(`SquireContextAdapter::finalize_turn`/`expand_for_display`); `src-tauri/src/commands/streaming_cmd.rs`
(`StreamEvent::Chunk` handling, `context_mode` branch points); `src/stores/chat-store/stream-listeners.ts`
(frontend consumer of `stream-chunk`).

Out of scope (do NOT fix here — separately tracked, deliberately deferred):
- sa-5 (ask_user response-field UI round-trip loop)
- `squire-storage/todo.json` ss-9 (real tool-token ingestion)
- User-input auto-chunking (`USR_TN_NNN` tokens)
- Raw-partition audit-log storage
- `retrieval-fidelity/todo.json` rf-13 (fuller hit-count-event fidelity)
