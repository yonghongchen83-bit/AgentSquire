# Env

- Parent node: root/Squire
- Node path: root/Squire/stream-sigil-fix
- Objective: Close `squire-adapter/todo.json` sa-4 — stop forwarding Squire mode's raw,
  sigil-laden protocol JSON to the live `stream-chunk` UI channel before `finalize_turn`
  expands/strips it, without changing Legacy mode's live-streaming behavior at all.
- Scope: `src-tauri/src/commands/streaming_cmd.rs`'s `StreamEvent::Chunk` handling and
  the two tool-loop-path `stream-chunk` emission sites in the same function; a small
  pure helper function plus unit tests for the mode-gating decision.
- Non-goal: sa-5 (ask_user UI round-trip loop), `squire-storage/todo.json` ss-9 (real
  tool-token ingestion), user-input auto-chunking (`USR_TN_NNN` tokens), raw-partition
  audit-log storage, `retrieval-fidelity/todo.json` rf-13 (hit-count-event fidelity).
  All remain open, unclaimed follow-ups tracked in their original nodes' todo.json files.
- Depends on: squire-adapter (`SquireContextAdapter::finalize_turn`/`expand_for_display`,
  the sigil-expansion logic this fix defers to), session-mode (`ContextMode` enum, the
  branch point this fix hooks into).
- Status: implemented and landed 2026-07-02.

## Durable facts (added this session)

- `src-tauri/src/commands/streaming_cmd.rs` gained a small pure function,
  `should_stream_live_chunks(context_mode: ContextMode) -> bool`, returning `false` only
  for `ContextMode::Squire`. This is the single source of truth for whether live
  per-token model output should be forwarded to the frontend's `stream-chunk` IPC event
  during generation. `send_message_impl` computes `stream_live_chunks` once per turn
  (from `session.session.context_mode`) and gates three emission sites on it:
  1. The main `StreamEvent::Chunk(text)` handler in the streaming-receive loop (the
     actual sigil-leak site sa-4 named).
  2. The `"\n\n"` separator emitted before starting tool execution in the
     `FinishReason::ToolCalls` branch.
  3. The `"[Executing {tool_name}...]\n"` progress-text chunk emitted after a
     destructive-tool approval is granted.
  `full_response` (the buffer `finalize_turn` eventually receives) still accumulates
  unconditionally in all three cases — only the *live UI forwarding* is suppressed, not
  the underlying accumulation `finalize_turn` depends on.
- Legacy mode (`ContextMode::Legacy`) is completely unaffected — `should_stream_live_chunks`
  returns `true` for it, so all three emission sites behave exactly as before this session.
- Squire-mode sessions now show no live-typing text in the chat pane during generation
  (an accepted UX tradeoff — see decisions.md); the finalized, sigil-expanded message
  still appears the moment `finalize_turn` persists it and `stream-done` fires, exactly
  as it did before this fix. `emit_stream_status`/`stream-status` events (the small
  "Contacting model...", "Model requested tool execution", etc. status-line updates)
  are untouched and continue to fire in Squire mode, so the UI is not silent/frozen
  during a Squire turn — there is still a live status indicator, just not live prose.
- No frontend files were touched this session — the fix is entirely a backend
  emission-gating change. The frontend's `stream-chunk` handler
  (`src/stores/chat-store/stream-listeners.ts`) is unmodified; it simply receives fewer
  events during Squire-mode turns.
