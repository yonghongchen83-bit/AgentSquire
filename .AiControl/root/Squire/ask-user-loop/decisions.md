# Decisions

## Pause/resume mechanism: mirror `PendingApprovals` exactly, with a `String` payload

The codebase already has one working pattern for "pause an in-flight async turn, round-trip
a question to the frontend, resume with a value fed back from a separate Tauri command call":
`PendingApprovals` (`src-tauri/src/agent/mod.rs`) plus its two Tauri commands
(`approve_tool_call`/`reject_tool_call`, `src-tauri/src/commands/stream_control.rs`) and its
one IPC event (`stream-tool-pending`, emitted from `streaming_cmd.rs`). This node's ask_user
loop needs the identical shape — pause, surface, collect, resume — just with a free-text
answer instead of a boolean approval decision. Rather than inventing a new pattern, this node
adds a structurally identical second registry:

```rust
// src-tauri/src/agent/mod.rs
pub type AskUserAnswerSender = oneshot::Sender<String>;

pub struct PendingAskUserQuestions {
    pub pending: Arc<Mutex<HashMap<String, AskUserAnswerSender>>>,
}
```

keyed by a freshly generated UUID per question (not the tool `call_id` scheme — ask_user is
a response-field event, not a tool call, so there is no existing call id to key off; a new
UUID is generated in `streaming_cmd.rs` when the question is raised), managed as Tauri state
exactly like `PendingApprovals` (`app.manage(PendingAskUserQuestions::new())` in
`setup_cmd.rs`).

New IPC event: `stream-ask-user-pending`, payload `{ "question_id": "...", "session_id": "...", "question": "..." }`.
New Tauri command: `answer_ask_user_question(question_id: String, answer: String) -> Result<(), String>`,
looking up and resolving the pending oneshot sender exactly like
`resolve_tool_call_decision_impl` does for tool approvals — extracted the same way into a
reusable `resolve_ask_user_answer_impl(pending, question_id, answer)` free function for direct
unit testability, independent of Tauri's `State<'_, T>` wrapper.

**Rejected: reusing `PendingApprovals`'s existing `HashMap<String, oneshot::Sender<bool>>`
by encoding the answer as a JSON string inside the bool channel's approval path (e.g.
"approve" always, smuggle the answer through a side-channel).** This would conflate two
semantically different concepts (a yes/no tool-execution gate vs. a free-text answer) behind
one registry and one wire event, forcing the frontend to disambiguate tool-approval events
from ask-user events by shape-sniffing the event payload rather than by event name. A second,
purpose-built registry/event/command trio is barely more code than shoehorning a string into
a bool channel, and keeps the two human-in-the-loop flows independently readable, testable,
and evolvable (e.g. per-flow abandonment policy, below).

## `finalize_turn`'s `ask_user` branch: new `TurnOutcome::AskUser` variant, not an `Err`

`TurnOutcome` (`src-tauri/src/agent/context_adapter.rs`) gains a fourth variant:

```rust
pub enum TurnOutcome {
    Done,
    Retry,
    Failed { reason: String, failed_content: String },
    AskUser { question: String },
}
```

`SquireContextAdapter::finalize_turn` now returns `Ok(TurnOutcome::AskUser { question: parsed.ask_user.clone() })`
instead of `Err(...)` when `ask_user` is populated (this branch runs before validation, same
position as before — `ask_user`+`content` mutual exclusion is still enforced by
`validate_squire_response` for the ordinary compliant-close path, but a response with
`ask_user` alone and `content` empty is a valid, expected turn state per spec §8.2, not a
protocol violation, so it was never right for this to be modeled as an error in the first
place — `TurnOutcome` already existed specifically to give `finalize_turn` a way to describe
several different non-`Done` terminal-ish states to orchestration without conflating them
with a hard failure).

`LegacyContextAdapter::finalize_turn` is unaffected — it has no protocol response format and
never emits this variant; its match arm in the new orchestration branch is unreachable in
practice but must still be handled exhaustively (see below).

**Considered and rejected: making `AskUser` carry the full accumulated context (a resumable
closure or the messages vec itself) rather than just the question string.** `messages: &mut
Vec<ChatMessage>` is already mutably borrowed by `finalize_turn`'s caller for the duration of
the call and threaded through the whole turn loop in `streaming_cmd.rs` — orchestration
already owns it and is best positioned to append the resumption messages once the answer
comes back, exactly the same way it already relies on `SquireContextAdapter::reject`
appending messages for the `Retry` path. Keeping `TurnOutcome::AskUser` a plain data carrier
(just the question) keeps `context_adapter.rs`'s trait surface adapter-agnostic and matches
the existing `Retry`/`Failed` variants' shape (small, serializable-ish data, no closures or
borrowed state).

## Orchestration: pause inline in the `tokio::spawn`'d turn-loop task, mirroring tool approval

`streaming_cmd.rs`'s `FinishReason::Stop | Length` match arm gains a new
`Ok(TurnOutcome::AskUser { question })` case, structurally parallel to how
`FinishReason::ToolCalls`'s destructive-tool-call branch already pauses:

1. Generate a fresh `question_id` (UUID).
2. Insert a `oneshot::channel()`'s sender into `PendingAskUserQuestions`, keyed by
   `question_id`.
3. Emit `stream-ask-user-pending` with `{question_id, session_id, question}`.
4. `emit_stream_status(&app_clone, "Waiting for your answer...")`.
5. `.await` the paired receiver **directly inline**, in the same `tokio::spawn`'d async task
   that owns the whole turn loop — exactly like `await_approval_with_watchdog` already does
   for tool-call approval. This is possible for the same reason tool approval already works
   this way: the entire turn is one long-lived async task per session (tracked in
   `state.stream_tasks`), so blocking that one task on a channel receive does not block the
   Tauri runtime, other sessions, or the UI thread — it just holds this session's turn open
   until the user answers (or the task is aborted, see abandonment handling below).
6. On receiving the answer: append two messages to `messages` (the same vec the adapter's
   `Retry` path already mutates) — an `Assistant` message with the raw ask_user-populated
   JSON response (so the model's own question is preserved in the message history sent back
   to it, exactly mirroring how `reject_and_record`/`reject` preserve the model's prior
   rejected response before appending the rejection payload), followed by a `User` message
   containing a small JSON envelope `{ "user_answer": "<answer>" }` — **not** a raw string,
   for the same reason `reject` encodes its rejection payload as JSON rather than plain text:
   the model's system prompt describes structured JSON exchanges throughout the turn, and an
   unstructured bare-string user turn would be a novel, undocumented shape the model has no
   instruction for. `continue` back to the top of the `loop`, which re-sends `messages` via
   `provider.chat()` — generation resumes with the extended context, exactly as the spec's
   "full turn is re-submitted with the extended user request" describes, adapted to this
   runtime's provider-native tool-calling transport (which threads a `messages` array,
   not a rebuilt `user_request` string per `build_turn_input`, once a turn is already open —
   `build_turn_input` runs once per turn, at the very start, same as it always has; the
   ask_user loop extends the *already-open* turn's `messages`, it does not call
   `build_turn_input` again).
7. `stream_live_chunks` gating (sa-4) applies here too: the question itself is emitted only
   via the dedicated `stream-ask-user-pending` event, not the raw `stream-chunk` channel, so
   Squire mode's live-stream suppression is unaffected and the display-boundary guarantee
   (spec §14) holds — the question the user sees is `parsed.ask_user`, the clean field value,
   never the surrounding raw protocol JSON envelope.

**Considered and rejected: implementing this as a new adapter-level concern (e.g. an
`await_ask_user` async trait method with a callback/channel parameter passed into
`ContextManagerAdapter`), rather than orchestration-level.** `context_adapter.rs`'s existing
seam design (see `adapter-core/decisions.md`) deliberately keeps orchestration concerns
(provider calls, streaming, tool approval, watchdogs) out of adapters — adapters own only
history assembly, per-tool-call bookkeeping, and turn-close persistence. IPC event emission
and awaiting a frontend round-trip is squarely an orchestration concern (exactly like tool
approval already is), not something `SquireContextAdapter` should reach into `AppHandle`/
`State` to do itself. `TurnOutcome::AskUser` is the correct-sized seam: the adapter says "I
need a question answered," orchestration handles the how.

## Abandonment handling: reuse the existing stream-task abort/replace mechanism, no new timeout system

If the user navigates away, closes the app, or simply never answers, the paused turn's
`tokio::spawn`'d task sits blocked on the `oneshot::Receiver<String>::await` forever unless
something external unblocks or cancels it. Three pre-existing mechanisms already bound this,
so no new timeout/expiry system was added:

1. **`abort_stream` (existing command, `stream_control::abort_stream_impl`)**: already looks
   up and `.abort()`s the session's task in `state.stream_tasks`. Aborting a
   `tokio::spawn`'d task while it's suspended inside a `.await` (including a blocked
   `oneshot::Receiver::await`) is a normal, safe Tokio cancellation — the task future is
   simply dropped at its current await point, which also drops the `oneshot::Receiver`,
   which in turn causes any future `.send()` from `answer_ask_user_question` to return
   `Err` (received but discarded) rather than panicking or hanging the sender side. No new
   code was needed here: `abort_stream` already exists, is already wired to the frontend's
   "Stop" button (`chat-panel.tsx`'s `cancelStreaming`), and already fires whenever a new
   message is sent on the same session (`send_message_impl`'s existing
   `if let Some(existing) = stream_tasks.lock().await.remove(&session_key) { existing.abort(); }`
   at the top of every new turn) — so starting a new message on the same session while an
   ask_user question is still pending cleanly cancels the stuck turn rather than leaving two
   turns racing.
2. **App close**: Tauri tears down the whole process; the blocked task and its in-memory
   `PendingAskUserQuestions` entry are dropped with it. Nothing persists across restarts
   today for in-flight turns of any kind (tool approvals have the exact same property — a
   pending approval is also pure in-memory state, lost on app close), so this is consistent
   existing behavior, not a new gap introduced by this node.
3. **Orphaned `PendingAskUserQuestions` entry if the task is aborted without going through
   `answer_ask_user_question`**: the map entry (the `oneshot::Sender`) is simply dropped along
   with the aborted task's stack, which drops the sender — a subsequent (mistaken/late)
   `answer_ask_user_question` call for that same `question_id` gets a clean
   `Err("No pending question with id '...'")`, exactly parallel to
   `resolve_tool_call_decision_impl`'s existing "No pending tool call with id" error for a
   stale/already-resolved approval id. No leaked/stale map entries accumulate from abort,
   since removal happens either on resolve (frontend answers) or is naturally consistent with
   the sender being dropped (task aborted) — the *sender* side drop doesn't remove the map
   entry automatically, so a future defensive cleanup (e.g. sweeping dead entries whose
   receiver has already fired `Err` on send) could be added if this ever becomes a real
   memory-growth concern; not implemented here since `PendingApprovals` has had the exact
   same latent property since it shipped and has never needed it in practice (session count
   and abandonment rate are both low relative to process lifetime).

**Considered and rejected: an explicit timeout (e.g. auto-cancel the turn after N minutes
unanswered).** The task's own instructions explicitly say "don't need a fully engineered
timeout system." Tool-call approval (`await_approval_with_watchdog`) already has a similar
unbounded-wait shape and solves the "user might never respond" problem with periodic
`stream-status` nudges (every 10s) plus an `output:append` info line past 30s, never an
auto-decision — ask_user's pause reuses that same watchdog helper for the identical
UX-consistency reason (a stuck ask_user question should look and feel like a stuck approval
prompt to the user, not a silently different interaction pattern), rather than inventing a
new timeout policy this node has no product requirement to design.

## Frontend: inline question/answer UI mirroring the `pendingApprovals` idiom, not a modal

`chat-panel.tsx` already has an established idiom for "pause the turn, show something
actionable inline in the chat input area, resolve via a command call": the
`pendingApprovals` array + inline Approve/Reject buttons rendered just above `ChatInput`.
This node adds a single `pendingAskUserQuestion: { questionId: string; question: string } | null`
piece of state (singular, not an array — the spec's loop is strictly sequential, one
outstanding question at a time per session, never concurrent) plus an inline text input +
Submit button rendered in the same location, gated on `pendingAskUserQuestion !== null`.
Submitting calls the new `answerAskUserQuestion(questionId, answer)` store action, which
calls the new `answer_ask_user_question` IPC command and clears `pendingAskUserQuestion`
optimistically (mirroring `approveToolCall`'s existing optimistic-clear pattern).

**Rejected: a modal/dialog.** The task's own instructions say not to over-build this if the
chat UI has a simpler existing idiom for inline interactive elements — it does
(`pendingApprovals`), so this node uses it rather than introducing a new UI primitive
(a modal component, focus-trap, backdrop, etc.) for what is functionally the same
"turn is paused, waiting on you" interaction shape the app already has a working answer for.

## Manual verification: real end-to-end, via two independent methods (superseding stream-sigil-fix's precedent)

Unlike every prior Squire-epic session, this session found a real, working LLM provider
configured and a real interactive desktop session available — the "no provider configured /
no GUI access" blocker documented in `stream-sigil-fix/state.md` did not hold this time (a
free-tier test provider, OpenCode Zen serving `deepseek-v4-flash-free`, was made available
mid-session; see `state.md`'s timeline for exactly how). Two independent real-model
verification passes were performed, in order of increasing fidelity:

**1. `src-tauri/examples/ask_user_e2e.rs` — a standalone, headless, real-model harness.**
Drives the exact same adapter/orchestration sequence `streaming_cmd.rs` uses
(`SquireContextAdapter::build_turn_input` -> `provider.chat()` -> drain the stream ->
`finalize_turn` -> branch on `TurnOutcome`), outside Tauri/IPC entirely, so it can run in a
sandboxed CLI environment with no GUI. Standing in for the frontend, it supplies a canned
answer whenever `finalize_turn` returns `TurnOutcome::AskUser`, mirroring exactly what
`streaming_cmd.rs`'s real handling does with a real answer from `answer_ask_user_question`.
Run via `cargo run --example ask_user_e2e` with `SQUIRE_E2E_API_KEY`/`SQUIRE_E2E_BASE_URL`/
`SQUIRE_E2E_MODEL` env vars (not committed with the key baked in). This confirmed, against a
real model: `finalize_turn` correctly returns `TurnOutcome::AskUser` (not an `Err`) the first
time the model populates `ask_user`; the harness's simulated answer is correctly appended and
generation resumes; a subsequent malformed-JSON model response still correctly drives
`TurnOutcome::Retry` then `TurnOutcome::Failed` through the *same* adapter instance without
retry-count corruption from the earlier `AskUser` outcome (matching the
`finalize_turn_ask_user_does_not_reset_retry_count` unit test's claim, now also seen live).
The turn didn't fully close in this run because `deepseek-v4-flash-free` (a small free-tier
model) kept re-asking variations of its clarifying question across several rounds rather than
proceeding to `content` — a real model-behavior/prompt-engineering characteristic, not a
defect in the pause/resume mechanism (every round correctly went through
pause -> surface -> collect -> resume).

**2. `e2e/specs/ask-user-loop.test.ts` — a real WDIO + tauri-driver spec against the actual
built app.** This is the strongest evidence available: launches the real `squirecli.exe`
binary in a real WebView2 window, creates a Squire-mode session via the real
`create_conversation` IPC command (there is no frontend UI toggle for `context_mode` yet —
orthogonal to sa-5's scope, see `env.md`), types a real prompt into the real `ChatInput`
component, sends it with the real Ctrl+Enter shortcut, and asserts:
- the new inline question UI (`chat-panel.tsx`'s blue `pendingAskUserQuestion` box) actually
  renders, populated with the real model's real question text, arriving via the real
  `stream-ask-user-pending` IPC event;
- the store's `pendingAskUserQuestion` state matches what's rendered;
- typing into the real answer `<input>` and clicking the real "Answer" button correctly calls
  `answer_ask_user_question`, which resolves the backend's `oneshot::Receiver` and clears the
  pending-question UI;
- the turn genuinely resumes afterward (not silently dead) — observed as either a further
  `ask_user` round or `isStreaming` continuing to progress, with no regression to the old
  hard-error message this node replaced.

Two consecutive runs both passed (~17-25s each) launching/tearing down the real app fresh
each time. See `state.md` for the full console transcripts of both runs. This is the same
`e2e/` WDIO+tauri-driver framework the project's `root/UiAutoTestFramework` node already
established for exactly this kind of verification — no new test infrastructure was built,
only a new spec file following the existing `task-009`/`task-007`/`test-two-providers`
patterns (IPC calls via `window.__TAURI_INTERNALS__.invoke`, store introspection via
`window.__chatStore`, `localStorage`-seeded model selection, real chat-input interaction).

**Why this session could do what `stream-sigil-fix` couldn't:** that session's environment
genuinely had no provider configured and (as far as that session could tell) no way to drive
a GUI. This session's environment turned out to have both a real interactive Windows desktop
session (confirmed via a running `explorer.exe` process) and, once a test provider was
configured, the pre-existing `e2e/` WDIO+tauri-driver harness worked without needing any new
tooling — `tauri-driver` was already installed, the app binary was already built, and
`msedgedriver.exe` (required by `tauri-driver` for WebView2 automation, not itself vendored in
this repo) was found already cached from a prior session in the OS temp directory and put on
PATH for this session's `tauri-driver` process. Future sessions hitting a "no GUI access"
wall should check for exactly these three things (interactive desktop session, `tauri-driver`
binary, `msedgedriver.exe` availability/PATH) before concluding manual verification is
impossible — it may only be that `msedgedriver.exe` isn't on PATH yet.
