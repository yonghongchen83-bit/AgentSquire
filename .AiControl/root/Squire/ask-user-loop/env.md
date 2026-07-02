# Env

- Parent node: root/Squire
- Node path: root/Squire/ask-user-loop
- Objective: Close `squire-adapter/todo.json`'s sa-5 — wire the response-field `ask_user`
  loop (spec §8.2/§9.3) into a real surface-question/collect-answer/resubmit round trip,
  instead of `finalize_turn` hard-erroring the turn when the model populates `ask_user`.
- Scope: `src-tauri/src/agent/squire.rs` (`SquireContextAdapter::finalize_turn`'s ask_user
  branch), `src-tauri/src/commands/streaming_cmd.rs` (turn loop orchestration: pause point,
  IPC event emission, resume/continue path), a new pending-question registry (mirroring
  `PendingApprovals`) plus a new Tauri command for the frontend to submit the answer,
  and the corresponding frontend wiring (`src/lib/ipc.ts`, `src/stores/chat-store.ts`,
  `src/stores/chat-store/stream-listeners.ts`, `src/components/chat-panel.tsx`).
- Non-goal: `squire-storage/todo.json` ss-9 (real tool-token ingestion), user-input
  auto-chunking (`USR_TN_NNN` tokens), raw-partition audit-log storage,
  `retrieval-fidelity/todo.json` rf-13 (hit-count-event fidelity). All remain open,
  unclaimed follow-ups tracked in their original nodes' todo.json files.
- Depends on: squire-adapter (`SquireContextAdapter`, `TurnOutcome`, the strict Q5 tool
  boundary and validation-gate design), session-mode (`ContextMode` enum), the existing
  `PendingApprovals`/`stream-tool-pending`/`approve_tool_call`/`reject_tool_call`
  human-in-the-loop pattern (`src-tauri/src/agent/mod.rs`, `src-tauri/src/commands/stream_control.rs`)
  as the direct design template for this node's pause/resume mechanism.
- Status: completed, 2026-07-03. sa-5 resolved: real pause/surface/collect/resume loop
  implemented, unit-tested, and verified end-to-end against a real model via both a headless
  Rust harness and a real WDIO+tauri-driver e2e spec (2/2 runs passing).

## Durable facts (added this session)

- The codebase already has exactly one working pattern for "pause an in-flight turn,
  round-trip to the frontend, resume with a value": `PendingApprovals` — a
  `HashMap<String, oneshot::Sender<bool>>` behind a `tokio::sync::Mutex`, managed as Tauri
  state (`app.manage(PendingApprovals::new())` in `setup_cmd.rs`). The backend inserts a
  `oneshot` sender keyed by a call id, emits an IPC event (`stream-tool-pending`) carrying
  that id, and `.await`s the paired receiver directly inline in the async turn-loop task
  (`tokio::spawn`'d in `send_message_impl`). The frontend calls a Tauri command
  (`approve_tool_call`/`reject_tool_call`) that looks up the sender by id and sends the
  decision, unblocking the `.await`. This node's ask_user pause/resume mirrors this pattern
  exactly, with a `String` (the answer) instead of a `bool` (the approval decision) as the
  channel payload.
- This session's environment turned out to support real end-to-end manual verification,
  unlike every prior Squire-epic session: a free-tier test LLM provider (OpenCode Zen,
  `deepseek-v4-flash-free`) was configured in the app's real config (`%APPDATA%\com.squirecli.app\config.toml`
  on Windows — not `src-tauri/.squirecli/config.toml`, which is only a `dirs_fallback()` path
  used when `set_config_dir` is never called), and a genuine interactive Windows desktop
  session was available. The project's pre-existing `e2e/` WDIO+tauri-driver harness
  (`root/UiAutoTestFramework`) could drive the real built app once `msedgedriver.exe` (needed
  by `tauri-driver` to automate WebView2, not itself vendored in this repo) was located
  already cached in the OS temp directory from an earlier session and added to `PATH`. Future
  sessions should check for these three things before concluding manual GUI verification is
  impossible in this environment.
- `src-tauri/examples/ask_user_e2e.rs` (new) is a reusable, headless, real-model verification
  harness for ask_user-loop behavior, run via `cargo run --example ask_user_e2e` with
  `SQUIRE_E2E_API_KEY`/`SQUIRE_E2E_BASE_URL`/`SQUIRE_E2E_MODEL` env vars.
- `e2e/specs/ask-user-loop.test.ts` (new) is a real WDIO spec exercising the full pause/resume
  loop against the actual built app; run via `npx wdio run ./e2e/wdio.conf.ts --spec ./e2e/specs/ask-user-loop.test.ts`
  with `tauri-driver` running separately (or `npm run test:e2e:dev` to start both together).
