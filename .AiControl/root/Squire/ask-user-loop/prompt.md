# Prompt

Wire `squire-adapter/todo.json`'s **sa-5**, deliberately deferred by `squire-adapter`,
`rejection-ux`, `stream-sigil-fix`, and `protocol-doc-sync`'s reconciliation pass as an
orchestration/UI round-trip concern outside each of those nodes' scope:

> Wire the ask_user response-field AskUser loop (spec 9.3): currently finalize_turn fails
> the turn with an error if the model populates ask_user, since resubmitting with an
> accumulated user_request requires a UI round-trip orchestration doesn't have yet.
> TOOL_AskUser via invoke() (Q2's other path) is unaffected and works today through the
> normal tool-call loop.

Concretely, per `context_squire_spec_v2.md` §8.2/§9.3 (as reconciled by `protocol-doc-sync`):
when the model's Squire-protocol JSON response populates `ask_user` (with `content` empty,
per the mutual-exclusion rule), the Squire is supposed to surface that question to the human
user, collect their answer, append both the question and answer as plain text to the
accumulated `user_request`, and resubmit the full turn with the extended user request and the
same prefetched/preserved context — repeating until the model emits a response with `content`
populated and `ask_user` empty. Today `SquireContextAdapter::finalize_turn` instead returns
`Err(...)` immediately, which orchestration surfaces as a hard `stream-error` — ending the turn
with no way for the user to actually answer.

Deliverables:
- Design and implement a real pause/resume mechanism: when `finalize_turn` detects a
  populated `ask_user` field, pause the turn (do not error out), emit an IPC event carrying
  the question to the frontend, and await a resume signal — a new Tauri command the frontend
  calls with the user's answer — that feeds the answer back into the turn so generation
  continues with the extended `user_request`. Follow the same async/IPC idioms already used
  for the existing destructive-tool-call approval flow (`PendingApprovals`,
  `stream-tool-pending`/`approve_tool_call`/`reject_tool_call` in
  `src-tauri/src/commands/stream_control.rs` and `streaming_cmd.rs`) rather than inventing a
  new pattern from scratch.
- Frontend: render the pending question inline in the chat UI (the existing
  `pendingApprovals` inline-prompt idiom in `chat-panel.tsx` is the model to follow — don't
  build a modal system) and wire submission to call the new backend command.
- Handle the turn-abandonment edge case (user navigates away / closes the app / never
  answers) pragmatically — no full timeout-engineering system required, but the pending
  question state must not leak forever. Document the chosen approach.
- Add real backend unit tests for the new dispatch/pause/resume logic. Frontend tests only
  if the existing test setup makes this easy for this kind of interactive component —
  don't block on frontend test infrastructure that doesn't already exist for it.
- Run `cargo build` + `cargo test --lib` (expect clean build, 151/151 passing baseline) and
  `npx tsc --noEmit -p tsconfig.app.json` from repo root after any frontend changes.
- Manually verify in the running app if practical; otherwise document why not (the previous
  session found no LLM provider configured in this sandbox) and rely on unit tests plus a
  code-path trace.

Reference: `../squire-adapter/todo.json` sa-5; `../squire-adapter/decisions.md` (Q5 tool
boundary: explore/token_to_detail/invoke, the `invoke` dispatch shape); `../context_squire_spec_v2.md`
§8.2 (response format, `ask_user` field), §9.3 (AskUser Loop, both the tool-registered path
already implemented and the response-field path this node implements); `src-tauri/src/agent/squire.rs`
(`SquireContextAdapter`, `TurnOutcome`, `finalize_turn`'s current hard-error branch);
`src-tauri/src/commands/streaming_cmd.rs` (turn loop orchestration, IPC event emission,
`FinishReason::Stop | Length` branch that calls `finalize_turn`); `src-tauri/src/commands/stream_control.rs`
and `src-tauri/src/agent/mod.rs` (`PendingApprovals`, `ApprovalSender`/`ApprovalReceiver` —
the existing human-in-the-loop pattern to mirror); `src/stores/chat-store/stream-listeners.ts`,
`src/stores/chat-store.ts`, `src/components/chat-panel.tsx` (the existing
`pendingApprovals`/`stream-tool-pending` event-driven inline-approval UI to mirror for the
new inline question/answer UI).

Out of scope (do NOT fix here — separately tracked, deliberately deferred):
- `squire-storage/todo.json` ss-9 (real tool-token ingestion)
- User-input auto-chunking (`USR_TN_NNN` tokens)
- Raw-partition audit-log storage
- `retrieval-fidelity/todo.json` rf-13 (fuller hit-count-event fidelity)
