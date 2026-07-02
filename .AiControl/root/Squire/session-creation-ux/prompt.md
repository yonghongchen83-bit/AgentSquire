# Prompt

Close the "newly observed gap" flagged at the end of `../tool-token-ingestion`'s session
(see `../handoff.md`'s "Newly observed gap" section, and `../state.md`'s epic-closeout
judgment call which lists this as the most consequential remaining gap):

> No frontend UI exists to create a Squire-mode session — `context_mode` is only settable
> via a direct `create_conversation` IPC call with a `contextMode` argument today... Not
> claimed by any node; a session-creation UX gap, not a protocol-fidelity gap.

Concretely: `context_mode` (legacy|squire) is a real, working, persisted,
immutable-by-construction field on conversations (`../session-mode`), and the entire Squire
pipeline behind it (`../squire-adapter`, `../squire-storage`, `../retrieval-fidelity`,
`../stream-sigil-fix`, `../ask-user-loop`, `../tool-token-ingestion`) works and is
reasonably well-verified end-to-end. But nothing in the actual chat UI lets a human user
choose Squire mode when starting a new conversation — every session ever created through
the UI has been implicitly Legacy, and every e2e spec needing a Squire-mode session
(`e2e/specs/ask-user-loop.test.ts`) has had to reach into
`window.__TAURI_INTERNALS__.invoke('create_conversation', { contextMode: 'squire' })`
directly, bypassing the UI entirely.

Deliverables:
- Read `../session-mode/decisions.md` and `../session-mode/state.md` first — confirm the
  persistence model, the immutable-by-construction guarantee (mode is fixed at creation
  time only, no "change later" path exists or should be added), and the existing
  `create_conversation`/`createConversation` IPC surface (already accepts a `contextMode`
  argument — this node should not need to change that contract, only start calling it with
  a real value from a real UI control).
- Read `../context_squire_spec_v2.md` for any UX expectations about entering/exiting Squire
  mode — expect it to be silent (protocol-focused spec), in which case use your own
  judgment for a sensible, minimal UI.
- Read the actual frontend code: find wherever a new conversation/session is currently
  created (`ConversationSidebar`'s `onCreate`, `chat-store.ts`'s `createNewConversation`),
  `src/types/ipc.ts`/`src/lib/ipc.ts` for the existing typed IPC wrapper, and
  `chat-store.ts`/related store files for how conversation state is modeled.
- Verify baseline first: `cargo build` + `cargo test --lib` from `src-tauri/` (expect clean,
  171/171 passing); `npx tsc --noEmit -p tsconfig.app.json` + `npm test -- --run` from repo
  root (expect zero new TS errors beyond the pre-existing `tools-panel.tsx` issues, 77/79
  frontend tests with the same 2 known pre-existing failures — confirm they're still the
  only two).
- Design and implement a minimal mode selector at conversation-creation time — extend the
  existing "+ New session" affordance rather than building a new parallel creation path.
  Sensible default: Legacy (matching current implicit behavior, least disruption to
  existing muscle memory), Squire as an explicit opt-in. Do not add any "change mode later"
  feature — that would contradict the immutable-by-construction architecture.
- Add whatever minimal visual indicator makes sense so a user can tell which mode an
  existing/active conversation is in (check first whether anything like this already
  exists — it does not, per env.md).
- Add frontend tests for the new UI/store logic, matching existing test file conventions
  (`chat-store.test.ts`, `conversation-sidebar.test.tsx`, or a new component test file if a
  new component is introduced).
- Manually verify with the WDIO+tauri-driver e2e setup (test LLM provider already
  configured in `%APPDATA%\com.squirecli.app\config.toml`; `tauri-driver`/
  `msedgedriver.exe` should already be on PATH from `../ask-user-loop`'s session — see its
  decisions.md if you need to re-establish this) that a real user can pick Squire mode when
  creating a session through the actual UI, and that the resulting conversation behaves as
  Squire mode. Consider simplifying/removing `e2e/specs/ask-user-loop.test.ts`'s
  IPC-reach-in workaround now that a real UI path exists, or adding a new spec — your
  judgment; note in state.md whether performed and what was found.
- Update `../state.md`'s "Child Nodes" list and `../handoff.md` to reflect this session's
  work, current build/test status, and the remaining backlog (user-input auto-chunking,
  raw-partition storage, rf-13, the endpoint-carrying `TokenDetail` extension —
  session-creation UX now resolved).

Reference: `../session-mode/decisions.md` (immutability-by-construction, persistence
model), `../session-mode/state.md` (deliverables list, explicitly named this as future
work: "a UI control to create a session in Squire mode"), `../handoff.md` ("Newly observed
gap" section), `../ask-user-loop/env.md`/`e2e/specs/ask-user-loop.test.ts` (the IPC
reach-in workaround this node makes unnecessary), `src/types/ipc.ts`/`src/lib/ipc.ts`
(existing `ContextMode`/`createConversation` contract), `src/stores/chat-store.ts`
(`createNewConversation`), `src/components/conversation-sidebar.tsx` (`onCreate` /
"+ New session"), `src/components/chat-panel.tsx` (wiring), `src/components/ui/switch.tsx`
(existing unused Radix Switch primitive).

Out of scope (do NOT change here):
- Any backend adapter/protocol/storage logic (all already complete and working)
- A "change mode after creation" feature (contradicts immutable-by-construction design)
- User-input auto-chunking, raw-partition audit storage, `retrieval-fidelity/todo.json`
  rf-13, the endpoint-carrying `TokenDetail` extension (separate, unclaimed backlog items)
