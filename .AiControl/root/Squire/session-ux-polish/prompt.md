# Prompt

Close out the two small, explicitly non-blocking UX follow-ups `../session-creation-ux`
surfaced in its own `decisions.md`/`state.md` (Risks section) and carried in
`../handoff.md`'s residual backlog ever since:

> - (Small, optional, non-blocking, surfaced by `session-creation-ux`) persisting the
>   Squire-toggle's last choice across remounts, and/or adding a mode indicator to the
>   active conversation's own chat header

Concretely, two items:

1. **Toggle persistence.** Today, per `../session-creation-ux/decisions.md`, the Squire
   mode toggle (`ConversationSidebar`'s `nextSessionSquireMode`, component-local
   `useState`, default `false`) always resets to off/Legacy on every remount — a user who
   wants Squire mode has to re-flip it every time `ConversationSidebar` remounts (e.g. app
   restart), even within the same running app session in some cases.
2. **Active-conversation mode indicator.** Today, per the same decisions.md, the "Squire"
   badge only appears on conversation rows in the sidebar list. Once a user is actually
   inside/viewing a Squire-mode conversation (the "Chat" tab, not the "Sessions" tab), there
   is no indicator in the main chat panel itself telling them which mode that conversation
   is in.

Deliverables:
- Read `../session-creation-ux/decisions.md` and `state.md` in full first — confirms exactly
  what was built (the `Switch` in `ConversationSidebar`, the `nextSessionSquireMode`
  component-local state, the sidebar row badge, `chat-store.ts`'s `createNewConversation`)
  and exactly why these two items were deliberately left out of that node's own scope (see
  its "Default: Legacy, unconditionally" and "Visual indicator" decision sections, and its
  Risks list).
- Read the actual frontend code: `src/components/conversation-sidebar.tsx` (existing switch
  + badge), `src/stores/chat-store.ts` (state management, `conversations`/
  `activeConversationId`), `src/stores/chat-store/preferences.ts` (existing `localStorage`
  persistence pattern for model/thinking-level selection — the established precedent for any
  persistence choice here), and `src/components/chat-panel.tsx` (the main chat panel's own
  header area, where the model/thinking-level selectors already live).
- Verify baseline first: `npx tsc --noEmit -p tsconfig.app.json` (expect the same 7
  pre-existing `tools-panel.tsx` errors, zero new) and `npm test -- --run` (expect 82/84,
  same 2 known pre-existing failures: `chat-input.test.tsx`, `chat-blocks.test.tsx`) from
  repo root; `cargo build` and `cargo test --lib` from `src-tauri/` (expect clean/206
  passing — this session should not need backend changes, but verify anyway).
- Implement both items, keeping scope minimal:
  - Toggle persistence: pick a sensible mechanism and document the choice in decisions.md.
    In-memory store state that survives remounts within the same app session may be
    sufficient — check whether the complaint is really about full app-restart persistence or
    just component remounts, and don't add `localStorage` persistence speculatively if a
    simpler fix satisfies the actual complaint.
  - Chat header indicator: add a small, minimal visual element to the active conversation's
    chat header (the "Chat" tab's top bar, near the model/thinking selectors) showing its
    mode, matching the sidebar badge's existing style/conventions for consistency. Use
    judgment on whether to show only for Squire (matching the sidebar's "badge only for the
    non-default case" convention) or something else if that reads better in this specific
    location — document whichever is chosen.
- Add/update frontend tests for both changes, matching `../session-creation-ux`'s test-file
  conventions (`chat-store.test.ts`, `conversation-sidebar.test.tsx`, or a new
  `chat-panel.test.tsx` if a new component/behavior needs its own file).
- Manually verify via the WDIO+tauri-driver e2e setup if practical (a free-tier test LLM
  provider is configured in `%APPDATA%\com.squirecli.app\config.toml`;
  `e2e/specs/session-creation-ux.test.ts` is the relevant existing spec to extend or
  reference) — confirm the toggle stays on Squire after creating a session and
  navigating/remounting, and that the chat header shows the indicator for a Squire-mode
  conversation. If this feels disproportionate for such a small UI change, unit/component
  tests are an acceptable substitute — use judgment and document the choice, consistent with
  how prior small-scope sessions in this epic (`tool-token-ingestion`,
  `user-input-chunking`, `raw-partition-storage`) have handled verification proportionality
  for changes with limited/no new user-facing surface (here there *is* new user-facing
  surface, so this needs its own judgment call, not a copy of those nodes' "no frontend
  surface" reasoning).
- Update `../state.md`'s "Child Nodes" list and `../handoff.md` to reflect this session's
  work, current build/test status, and the remaining backlog (`retrieval-fidelity/todo.json`
  rf-13, the endpoint-carrying `TokenDetail` extension, the memory-alias gap
  `user-input-chunking` flagged — this item now resolved).

Reference: `../session-creation-ux/decisions.md` (full UI-placement/default-behavior/
visual-indicator design reasoning and the exact "Where else a mode indicator could have
gone" note that pre-scopes this node's second item), `../session-creation-ux/state.md`
(Risks section, the exact origin of both items), `../handoff.md` ("Remaining backlog"
section), `src/components/conversation-sidebar.tsx`, `src/stores/chat-store.ts`,
`src/stores/chat-store/preferences.ts`, `src/components/chat-panel.tsx`.

Out of scope (do NOT change here):
- Any backend adapter/protocol/storage logic (unrelated to this UI-only polish)
- A "change mode after creation" feature (contradicts immutable-by-construction design,
  per `../session-mode/decisions.md`)
- `retrieval-fidelity/todo.json` rf-13, the endpoint-carrying `TokenDetail` extension, the
  `"memory"`-alias/`system_referential` gap `../user-input-chunking` flagged (separate,
  unclaimed backlog items)
