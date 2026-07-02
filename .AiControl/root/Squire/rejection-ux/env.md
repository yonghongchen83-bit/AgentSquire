# Env

- Parent node: root/Squire
- Node path: root/Squire/rejection-ux
- Objective: Implement user-facing compliance-failure visibility (Q6) and preserve-list lifecycle handling (Q7).
- Scope now: surfacing failure reason + final failed AI response after retry exhaustion, structured failure-metadata persistence, preserve-list next-turn-only lifecycle.
- Non-goal now: protocol validation logic itself (see squire-adapter), the ask_user response-field UI round-trip (sa-5, deferred), live-stream sigil-leak buffering (sa-4, deferred), real tool-token ingestion (ss-9, unrelated).
- Depends on: squire-adapter (needs the validation/retry path to hook into) — also squire-storage in practice, since the preserve-list-clear-on-restart half of Q7 only matters once preserve-lists are durable (LanceDB), which squire-storage delivered.
- Status: implemented and landed 2026-07-02.

## Durable facts (added this session)

- `SquireStore` trait (`src-tauri/src/agent/squire.rs`) gained two methods: `record_compliance_failure(ComplianceFailureRecord)` (append-only diagnostic write) and `clear_all_preserve_lists()` (Q7 restart-clear). Both `InMemorySquireStore` and `LanceDbSquireStore` implement them.
- `LanceDbSquireStore` (`src-tauri/src/storage/squire_lancedb.rs`) now manages a fifth LanceDB table, `squire_compliance_failures` (`session_id`/`rule`/`reason`/`retry_count`/`failed_content`/`timestamp` — timestamp as RFC3339 text), in the same connection/directory as the other four tables.
- `setup_cmd.rs` calls `squire_store.clear_all_preserve_lists()` once at startup, immediately after opening `LanceDbSquireStore` and before `app.manage(AppState {...})` — this is the concrete Q7 "restart clears carryover" enforcement point.
- `SquireContextAdapter::finalize_turn`'s final-failure path (`TurnOutcome::Failed`) now persists a real assistant-role chat message (reason + failed content) via `ConversationStore::append_message` before returning, through a new `reject_and_record` async wrapper around the existing sync `reject` helper. No frontend changes were needed — `onStreamError` in `src/stores/chat-store/stream-listeners.ts` already reloads the conversation from the store on any `stream-error`, which is still emitted for the live-session error banner.
- `classify_rejection_rule(reason: &str) -> String` in `squire.rs` maps free-text rejection reasons to short stable rule ids for the structured failure record; falls back to `"other"` for unrecognized wording.
