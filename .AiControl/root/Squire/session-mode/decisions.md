# Decisions

## Immutability enforcement (sm-3) — structural, not a guarded setter

Q3 says context_mode is immutable after the first user turn. Rather than adding a `set_context_mode` store method with an internal "reject if messages exist" check, the mode is made immutable by construction: there is no IPC command, `ConversationStore` method, or any other code path that mutates `sessions.context_mode` after `create_session`. `context_mode` is set once at INSERT time from `NewSession.context_mode` (default `Legacy`) and every read (`get_session`) simply reflects that stored value. Adding a guarded-but-unreachable mutation path would be dead code with no caller — if a "change mode" feature is ever requested, that's the point to add the first-message check, not before.

## Squire mode routing before SquireContextAdapter exists

`send_message_impl` now matches on `session.session.context_mode`. `ContextMode::Legacy` selects `LegacyContextAdapter` as before. `ContextMode::Squire` currently emits `stream-error` ("Squire context mode is not yet implemented") and returns — fail-closed rather than silently falling back to Legacy. No UI currently offers creating a Squire-mode session (context_mode is plumbed through `create_conversation`'s optional param but nothing calls it with `"squire"` yet), so this path is unreachable in practice until `../squire-adapter` lands and a UI control is added.

## SQLite migration approach

Followed existing precedent in `state/db.rs` (`blocks_json`, `thinking_content`): `ALTER TABLE sessions ADD COLUMN context_mode TEXT NOT NULL DEFAULT 'legacy'` wrapped in `let _ = conn.execute(...)`, idempotent/silently-ignored on rerun. Row-mapping in `sqlite_store.rs::get_session` also falls back to `ContextMode::default()` (Legacy) if the column comes back NULL, covering any edge case where the SQLite default backfill doesn't apply.

## Discovered and fixed in passing

`src/lib/ipc.ts`'s `saveConfig()` was missing its closing brace (pre-existing, predates this node — confirmed via `git show HEAD`), which nested `listAvailableTools`/`setProjectPath`/`getProjectPath` inside it and broke their exports, cascading TS errors into `menu-bar.tsx` and `welcome-screen.tsx`. Fixed since it blocked verifying this node's own typecheck. A separate pre-existing bug (`AppConfig.disabledTools` missing from the type, plus an invalid lucide icon `title` prop) in `tools-panel.tsx` was left alone and flagged as a spawned background task — unrelated to context_mode.
