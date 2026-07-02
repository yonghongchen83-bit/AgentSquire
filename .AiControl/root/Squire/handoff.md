# Handoff — 2026-07-02, end of work-session

Short, operator-oriented status for picking this back up from another machine after `git pull`.

## Where things stand

Squire epic (`root/Squire`) — building a swappable `ContextManagerAdapter` so sessions can use Legacy (full history replay) or Squire (curated protocol context):

| Node | Status |
|------|--------|
| `planning` | ✅ Completed — architecture locked, see `planning/implementation-readiness.md` |
| `adapter-core` | ✅ Completed — `ContextManagerAdapter` trait + `LegacyContextAdapter` landed in `src-tauri/src/agent/context_adapter.rs`, wired into `send_message_impl` |
| `session-mode` | ✅ Completed — `context_mode` (legacy\|squire) persisted end-to-end, immutable by construction (no mutation path exists), `send_message_impl` routes by mode |
| `squire-adapter` | **Next up, unblocked** — implement real `SquireContextAdapter` (currently `send_message_impl` fails closed with "Squire context mode is not yet implemented" for squire-mode sessions) |
| `squire-storage` | Blocked on `squire-adapter` |
| `rejection-ux` | Blocked on `squire-adapter` |
| `protocol-doc-sync` | Not blocking, can be done any time — sync `context_squire_spec_v2.md` |

**Note on `.AiControl/.current`:** it currently points to `root/Squire/adapter-core`, even though that node is done. It got reset there mid-session by something outside this session (not reverted deliberately). Recommended action from home: point it at `root/Squire/squire-adapter` before starting, or just treat this handoff doc as the source of truth for "what's next" if you'd rather leave `.current` alone for now.

## Verification status as of this commit

- `cargo build` (lib + bin, from `src-tauri/`): clean, zero warnings.
- `cargo test --lib`: 96/96 passing.
- `npx tsc --noEmit -p tsconfig.app.json` (from repo root): clean for everything touched this session. Remaining errors are pre-existing and unrelated — see "Known pre-existing issues" below.
- `npm test -- --run` (vitest): 1 pre-existing flaky/failing test unrelated to this session's work (`chat-input.test.tsx`, confirmed failing at HEAD via `git stash` before any of today's changes too).

## Known pre-existing issues (not from this session, not yet fixed)

1. `src/components/tools-panel.tsx` references `AppConfig.disabledTools`, which doesn't exist on the `AppConfig` type in `src/types/ipc.ts`, plus an invalid `title` prop passed to a lucide-react icon (2 spots). Both predate this session (confirmed via `git show HEAD`). A background task was spawned for this during the session (chip title "Fix AppConfig.disabledTools type + lucide title prop") — that chip is local to the session it was created in and won't follow you home, so re-flag or just fix directly if you land here again.
2. `chat-input.test.tsx` — "calls onSend on Enter without Shift" fails intermittently; confirmed pre-existing (fails at HEAD too), not caused by anything in this session.

## To resume from home

1. `git pull`.
2. `cd src-tauri && cargo build && cargo test --lib` — should be clean/96 passing, confirming the pull landed correctly.
3. Read `root/Squire/squire-adapter/{env.md,prompt.md,state.md,todo.json}` for the next node's scope (SquireContextAdapter skeleton, strict tool-surface enforcement per Q5, protocol validation gates per Q6).
4. Reference `root/Squire/planning/decisions.md` for the resolved Q1–Q7 architecture decisions this all builds on, and `root/Squire/adapter-core/decisions.md` for the exact trait shape/seams already in place.
