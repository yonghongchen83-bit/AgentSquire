# Decisions

## Mode selector: a `Switch` next to the existing "+ New session" button, not a new dialog

The task's own instructions say not to over-build this — a full onboarding flow is
explicitly out of scope, and the existing UI already has exactly one conversation-creation
affordance (`ConversationSidebar`'s "+ New session" button, wired to `chat-panel.tsx`'s
`createNewConversation`). Rather than adding a second parallel creation path (e.g. a "New
Squire Session" menu item, a creation dialog with a mode dropdown), this node adds a small
`Switch` (the pre-existing, previously-unused-in-this-context `src/components/ui/switch.tsx`
Radix wrapper) directly in the sidebar header, immediately to the left of the existing "+"
button, with a "Squire" label. The switch's boolean state (`nextSessionSquireMode`,
component-local `useState`, default `false`) is read only at the moment "+" is clicked —
`onCreate(nextSessionSquireMode ? 'squire' : 'legacy')` — and is not itself persisted
anywhere; it is a pure "what mode should my *next* click of + use" control, not a property
of any existing conversation.

**Considered and rejected: a dropdown/select (matching the existing model/thinking-level
`Select` components in `chat-panel.tsx`).** A two-value binary choice (Legacy vs. Squire)
is exactly what a toggle communicates most economically; a `Select` would need a
placeholder state, an explicit "Legacy"/"Squire" item pair, and more visual footprint for
the same amount of information. The existing `Switch` primitive was already in the
codebase, unused in this part of the UI, and is the more idiomatic match for a strictly
binary, infrequently-changed setting.

**Considered and rejected: putting the toggle inside a new confirmation dialog shown when
"+" is clicked.** This would add a full modal/dialog interaction for what the task
description calls "enough" as "a simple toggle/dropdown/radio at conversation-creation
time" — a modal is a heavier interaction than the gap warrants, and the existing app has no
precedent of gating conversation creation behind a confirm step today (clicking "+"
currently creates immediately). Keeping the toggle always-visible next to "+" also makes
the *current* choice legible at a glance before the user commits, which a modal's
one-time-per-click state would not.

## Default: Legacy, unconditionally, matching pre-existing implicit behavior

`nextSessionSquireMode` initializes to `false` on every mount (not persisted to
`localStorage` or any other store). Every session created through the UI before this node
existed was implicitly Legacy (the hardcoded `createConversation('New Chat')` call, no
second argument, which itself maps to `NewSession.context_mode: None` -> backend
`unwrap_or_default()` -> `ContextMode::Legacy`). Making Legacy the always-reset default
means a user who has never touched the new switch experiences byte-for-byte the same
creation behavior as before this node — the task's own instruction ("least disruption to
existing muscle memory") is satisfied exactly, not approximately.

**Considered and rejected: persisting the last-chosen mode (e.g. `localStorage`, mirroring
`chat-store/preferences.ts`'s existing provider/model/thinking-level persistence
pattern).** This was a closer call — the codebase does have a precedent for persisting
"last used" UI selections. Rejected because (a) the task explicitly names Legacy-as-default
as the desired behavior regardless of prior session choices ("Sensible default (Legacy,
matching current implicit behavior... with Squire as an explicit opt-in)" — read as
per-creation opt-in, not sticky), and (b) Squire mode is the less-tested, more
experimental/newer path in this epic; requiring an explicit toggle click *every time* is a
deliberate small friction that keeps a user from accidentally creating a string of
Squire-mode sessions after one intentional choice. If real usage later shows this friction
is unwanted, adding persistence is a small, isolated follow-up (one `localStorage`
read/write, matching the existing `preferences.ts` pattern) — not a re-architecture.

## Visual indicator: a small badge on Squire-mode rows only, no badge for Legacy

`SessionSummary` did not previously carry `context_mode` (only the full `Session` struct
returned by `create_conversation`/`get_conversation` did) — the sidebar's conversation list
(`ConversationSidebar`, backed by `list_conversations`/`SessionSummary`) had no data to
render a per-row indicator from. This node adds `context_mode` to `SessionSummary`
end-to-end (Rust struct field with `#[serde(default)]` for forward/backward compatibility,
`list_sessions`'s SQL `SELECT`/row-mapping in `sqlite_store.rs`, the frontend `SessionSummary`
type, `RawSessionSummary`/`mapSessionSummary` in `lib/ipc.ts`) so the sidebar can render a
badge without a second per-row round-trip (`get_conversation` for every visible row would be
needlessly expensive — `list_conversations` already runs once for the whole list).

The badge itself (`ConversationSidebar`'s row rendering) is shown **only** when
`conv.contextMode === 'squire'` — nothing is rendered for Legacy rows. Legacy is the
overwhelming default/expected case (matching the epic's own framing throughout: Legacy is
"current implicit behavior", Squire is the explicit opt-in), so marking every Legacy row
would add visual noise to every single existing conversation for no new information: the
*absence* of a badge already unambiguously means Legacy, once a user learns the convention
from the (rarer) Squire-badged rows. This mirrors the existing UI's own convention
elsewhere in this epic (`ask-user-loop`'s pending-question box and `pendingApprovals`'s
warning row both only render when there's something non-default/actionable to show, not as
a permanent always-visible state indicator) and keeps the sidebar's existing dense,
compact row layout unchanged for the common case.

**Considered and rejected: a "Legacy" badge on Legacy rows too, for explicit parity.**
Would double the number of badges rendered across a typical user's session list for a
distinction that (once understood) is already fully conveyed by badge-absence. The task's
own phrasing ("whatever minimal visual indicator makes sense") points toward the smaller
surface area.

**Where else a mode indicator could have gone, and why not (yet):** the active/open
conversation's header area (`chat-panel.tsx`'s message-list header, near the
model/thinking-level selectors) would also be a reasonable place for an indicator, but
`chat-panel.tsx` currently has no per-conversation state readily available there beyond
`activeConversationId`/`messages` — plumbing the active session's own `contextMode` into
that view would need either a `Session` fetch (already available via `selectConversation`'s
`getConversation` call, but not currently stored on `ChatState`) or reusing the
already-fetched `conversations: SessionSummary[]` list to look up the active row's
`contextMode` by id. The sidebar-row badge alone was judged sufficient for this node's
"minimal indicator" bar — the active-conversation badge is a small, easy, non-load-bearing
follow-up if wanted (look up `conversations.find(c => c.id === activeConversationId)
?.contextMode` in `chat-panel.tsx`, no new IPC/store surface needed since `contextMode` is
now already on every fetched `SessionSummary` row) but was not built here to avoid
over-building beyond what the task asked for ("don't over-build this").

## No "change mode later" feature was added, and none should be

Per `../session-mode/decisions.md`, `context_mode` is immutable by construction: "there is
no IPC command, `ConversationStore` method, or any other code path that mutates
`sessions.context_mode` after `create_session`." This node adds a mode *choice* only at the
one call site that already determines a session's mode forever
(`createConversation(title, contextMode)` inside `createNewConversation`) — no new command,
store method, or UI control was added anywhere that could mutate an existing conversation's
mode. `ConversationSidebar`'s per-row UI (rename, delete) deliberately has no "change mode"
action alongside them, and none should be added without first revisiting
`session-mode`'s architecture (which the task explicitly said not to do).

## `SessionSummary` backend/IPC change: additive, non-breaking

Adding `context_mode` to the Rust `SessionSummary` struct with `#[serde(default)]` (falls
back to `ContextMode::default()` = Legacy if a serialized value predates this field — not
actually reachable today since nothing persists `SessionSummary` itself to disk, but
matches the existing codebase's own defensive-default convention seen on `Session`/
`get_session`'s NULL-column fallback) and to the frontend `SessionSummary`/
`RawSessionSummary` types/mapper is a pure additive change: no existing caller of
`list_conversations`/`list_sessions` breaks, and no other `SessionSummary`-consuming code
in the frontend needed changes beyond the two test fixtures that constructed a
`SessionSummary` object literal by hand (TypeScript's structural typing caught both at
`tsc --noEmit` time, confirming no other silent construction site exists).

## Manual e2e verification

See `state.md` for whether this was performed this session and what was found — recorded
there rather than here since it's an outcome/finding, not a design decision.
