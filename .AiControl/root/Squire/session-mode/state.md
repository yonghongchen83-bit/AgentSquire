# State

## Timeline
- 2026-07-02: Node created, split out of root/Squire/planning as implementation step 2 of the incremental delivery plan.

## Next Actions
- Unblocked: adapter-core is complete (`ContextManagerAdapter` trait + `LegacyContextAdapter` landed in `src-tauri/src/agent/context_adapter.rs`, wired into `send_message_impl`).
- Adapter is currently hardcoded to `LegacyContextAdapter` in `send_message_impl` — this node should replace that with mode-based selection.

- 2026-07-02: All 4 todos done. `context_mode` (legacy|squire, default legacy) added end-to-end: SQLite migration, `Session`/`NewSession` structs, `create_conversation` command + impl, frontend `Session`/`RawSession` IPC types, and `send_message_impl` now routes to `LegacyContextAdapter` vs a not-yet-implemented Squire path based on the session's stored mode. `cargo build` (lib + bin) clean, `cargo test --lib` 96/96 passing. Frontend `tsc --noEmit` clean for all files this node touched (unrelated pre-existing `tools-panel.tsx` errors remain, flagged separately). Frontend `vitest` has 1 pre-existing flaky/unrelated failure in `chat-input.test.tsx` (confirmed via git stash to also fail at HEAD before this node's changes).

## Node Closed — 2026-07-02

Deliverables:
- `src-tauri/src/storage/conversation_store.rs` — new `ContextMode` enum (`Legacy`/`Squire`, serde lowercase, `Default = Legacy`), added to `Session` and `NewSession`.
- `src-tauri/src/state/db.rs` — migration v3: `ALTER TABLE sessions ADD COLUMN context_mode TEXT NOT NULL DEFAULT 'legacy'`.
- `src-tauri/src/storage/sqlite_store.rs` — `create_session`/`get_session` read/write `context_mode`.
- `src-tauri/src/commands/{mod.rs,conversations.rs}` — `create_conversation` accepts optional `context_mode: Option<String>`.
- `src-tauri/src/commands/streaming_cmd.rs` — adapter selection now matches on `session.session.context_mode` (see decisions.md for the fail-closed Squire-not-implemented-yet behavior).
- `src/types/ipc.ts`, `src/lib/ipc.ts` — `ContextMode` type, `Session.contextMode`, `createConversation(title, contextMode?)`.
- Immutability (Q3) enforced structurally — no mutation path exists (see decisions.md).

Not in scope here (belongs to `../squire-adapter` and a future UI task): an actual `SquireContextAdapter`, and a UI control to create a session in Squire mode.
