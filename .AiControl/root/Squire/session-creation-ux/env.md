# Env

- Parent node: root/Squire
- Node path: root/Squire/session-creation-ux
- Objective: close the "newly observed gap" flagged at the end of `tool-token-ingestion`'s
  session (see `../handoff.md`'s "Newly observed gap" section) — add a real frontend UI
  control that lets a human user choose Legacy or Squire mode when creating a new
  conversation, so `context_mode` (a real, working, persisted, immutable-by-construction
  field per `../session-mode`) is actually reachable through normal app interaction, not
  only via a direct `create_conversation` IPC call (as every prior e2e spec needing a
  Squire-mode session, e.g. `e2e/specs/ask-user-loop.test.ts`, has had to do).
- Scope: a minimal mode selector at conversation-creation time (the existing "+ New
  session" affordance in `ConversationSidebar`), wired through `chat-store.ts`'s
  `createNewConversation` action to the already-existing `createConversation(title,
  contextMode?)` IPC wrapper (`src/lib/ipc.ts`); a small visual indicator (badge) so a user
  can tell which mode an existing conversation is in; frontend unit tests for both; a
  backend addition only if needed to carry `context_mode` through to `SessionSummary` for
  the badge (see Decisions).
- Non-goal: any "change mode after creation" feature (mode is immutable by construction per
  `../session-mode/decisions.md` — this node must not weaken that guarantee), a full
  onboarding/explainer flow for what Squire mode is, any backend protocol/adapter work
  (`context_mode` routing, `SquireContextAdapter`, storage — all already complete), the
  remaining protocol-completeness backlog items (user-input auto-chunking, raw-partition
  audit storage, `retrieval-fidelity/todo.json` rf-13, the endpoint-carrying `TokenDetail`
  extension `tool-token-ingestion` deliberately left out of scope) — those are separate,
  unclaimed, and out of scope here.
- Depends on: `session-mode` (the `ContextMode` type, `create_conversation`'s
  `contextMode`/`context_mode` param, the immutability-by-construction guarantee),
  `squire-adapter`/`squire-storage`/`ask-user-loop`/`tool-token-ingestion` (the working
  Squire pipeline this UI finally exposes — no changes needed to any of it).
- Status: completed, 2026-07-03.

## Durable facts (read this session)

- `create_conversation` (Tauri command, `src-tauri/src/commands/mod.rs` /
  `commands/conversations.rs::create_conversation_impl`) already accepts an optional
  `context_mode: Option<String>` and validates it via `ContextMode::from_str`. The frontend
  IPC wrapper `createConversation(title: string, contextMode?: ContextMode)`
  (`src/lib/ipc.ts`) already exists and passes `contextMode: contextMode ?? null` — **no
  backend or IPC-wrapper change was needed to plumb a chosen mode through creation itself.**
  The only frontend gap was that nothing ever called `createConversation` with a second
  argument.
- The one and only conversation-creation entry point in the UI today is
  `ConversationSidebar`'s `onCreate` prop (a bare `() => void`/`() => Promise<string|null>`,
  no arguments), wired in `chat-panel.tsx` directly to `chat-store.ts`'s
  `createNewConversation` store action, which itself was hardcoded to
  `createConversation('New Chat')` — no second argument, so every session ever created
  through the UI was implicitly Legacy (matching `NewSession.context_mode`'s `Option<...>`
  default-to-Legacy-when-`None` behavior, `session-mode/decisions.md`).
- `SessionSummary` (`src-tauri/src/storage/conversation_store.rs`, returned by
  `list_sessions`/`list_conversations` and used for `ConversationSidebar`'s row list) does
  **not** carry `context_mode` — only the full `Session` struct (`get_session`/
  `create_conversation`'s return value) does. This means the existing sidebar list has no
  data to render a per-row mode badge from without a backend change. See decisions.md for
  the fix chosen.
- `context_squire_spec_v2.md` (reconciled by `protocol-doc-sync`) is silent on
  session-creation UX/UI — it is a protocol-behavior spec (token records, explore/
  token_to_detail/invoke, turn lifecycle), not a UI spec. No spec section describes how a
  user is meant to enter/exit Squire mode. This node had full latitude to design the UI.
- `src/components/ui/switch.tsx` (a Radix `Switch` wrapper) already exists in the codebase
  and is unused elsewhere in the chat UI — a natural fit for a binary Legacy/Squire choice,
  used instead of introducing a new UI primitive.
- No existing mode indicator/badge of any kind exists anywhere in the frontend prior to this
  node (confirmed via a case-insensitive grep for "squire"/"legacy" across `src/` — the only
  hits were in `types/ipc.ts`'s `ContextMode` type itself and unrelated files).
