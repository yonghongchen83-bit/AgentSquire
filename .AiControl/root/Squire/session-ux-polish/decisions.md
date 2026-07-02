# Decisions

## Toggle persistence: `localStorage`, mirroring the existing `preferences.ts` pattern

`../session-creation-ux/decisions.md` considered and rejected `localStorage` persistence for
this exact toggle at the time, on the grounds that (a) the task asked for Legacy-as-default
"regardless of prior session choices" and (b) Squire was the newer/less-tested path, so a
deliberate per-creation friction was desirable. Both of those premises still hold in
general — this session does **not** change the *default-for-a-brand-new-install* behavior,
which stays Legacy exactly as before. What has changed is the explicit ask from a human
maintainer (this task) that the *already-made* choice should survive a remount, i.e. exactly
the "small, isolated follow-up" `session-creation-ux/state.md`'s Risks section already
pre-authorized: "If real usage shows this is unwanted friction, adding `localStorage`
persistence (mirroring `chat-store/preferences.ts`'s existing pattern) is a small, isolated
follow-up."

**Mechanism chosen:** two new functions in `src/stores/chat-store/preferences.ts` —
`loadStoredSquireModeDefault(): boolean` and `saveStoredSquireModeDefault(value: boolean):
void` — following the file's existing shape exactly (a single `localStorage` key,
`chat:last-squire-mode-default`, string `'true'`/`'false'`, `typeof window === 'undefined'`
guard, try/catch around all storage access, silent fallback to `false` on any read failure
or absent key). `ConversationSidebar`'s `nextSessionSquireMode` `useState` now lazy-
initializes via `useState(() => loadStoredSquireModeDefault())` instead of a hardcoded
`false`, and a new inline `onCheckedChange` handler both updates local state and persists
the new value immediately (`(checked) => { setNextSessionSquireMode(checked);
saveStoredSquireModeDefault(checked) }`) — no debounce needed, this is a single boolean
flip, not a high-frequency write.

**Why in-memory-only (e.g. lifting state to `chat-store.ts`, which stays alive for the
app's lifetime) was considered and rejected:** `ConversationSidebar` is already effectively
mounted once for the lifetime of a running app session — the pre-existing code comment in
`session-creation-ux/state.md` itself already established that switching between the
"Chat"/"Sessions"/"MCP" tabs does *not* remount it, since all three `TabsContent` panels
share one always-mounted `Tabs` root. The only real remount events are a full page
reload/app restart. An in-memory fix (moving the boolean into the Zustand store instead of
component state) would only help for a component-level remount that essentially never
happens today, and would do nothing for the actual named complaint in the task ("even
within the same app session, every new session start requires re-flipping it" — read
together with the task's own suggestion to check whether "in-memory... within the same app
session is probably sufficient... or if that's overkill" as literally two options to weigh).
Testing this directly: a plain in-memory Zustand-store-level fix would not survive a full
app restart, which is the scenario `session-creation-ux/state.md`'s own Risk note names
explicitly ("e.g. app restart"). `localStorage`, already proven safe and idiomatic in this
exact codebase for this exact category of "sticky UI preference," is the smallest change
that actually closes the named gap for both remount granularities (in-session and
cross-restart) at once, at no extra implementation cost over an in-memory version (same
number of touched files, one new small preferences-module function pair instead of a new
store field).

**The first-run/never-touched default is unchanged.** A user who has never flipped the
toggle still gets Legacy (no stored key -> `loadStoredSquireModeDefault()` returns `false`),
so `session-creation-ux`'s core "least disruption to existing muscle memory" guarantee for a
new user or a fresh profile is fully preserved — this node only makes an *already-expressed*
preference sticky, it does not change what happens before any preference has ever been
expressed.

## Chat header indicator: a badge matching the sidebar's exactly, shown only for Squire, placed above the model/thinking selector bar

**Where:** `chat-panel.tsx`'s "Chat" tab currently has exactly one header-like bar (the
Model/Thinking `Select` row), gated behind `providers.length > 0`. Rather than folding the
mode indicator into that same conditionally-rendered bar (which would make the indicator
disappear before providers finish loading, or on an empty-providers edge case unrelated to
which conversation is open), this node adds a small **separate** bar directly above it,
rendered whenever a conversation is selected and in Squire mode, independent of the
providers-loaded gate. This keeps the two concerns (mode indicator vs. model/thinking
selection) decoupled and means the indicator is visible immediately upon opening a
Squire-mode conversation, even in the split second before `loadProviders()` resolves.

**What data it needs, and why no new IPC/store surface was required:** `SessionSummary`
(already returned by `listConversations()`, held in `ChatState.conversations`) has carried
`contextMode` end-to-end since `../session-creation-ux`. The active conversation's own mode
is derived purely in the component via `useMemo(() =>
conversations.find((c) => c.id === activeConversationId)?.contextMode, [conversations,
activeConversationId])` — exactly the lookup `session-creation-ux/decisions.md`'s own
"Where else a mode indicator could have gone" note pre-scoped ("reusing the already-fetched
`conversations: SessionSummary[]` list to look up the active row's `contextMode` by id...
no new IPC/store surface needed"). No new command, store field, or session-detail fetch was
added.

**Visual style: an exact match of the sidebar's existing badge, not a new design.** Same
Tailwind classes (`text-[9px] font-semibold uppercase tracking-wide text-[#4A90D9]
bg-[#4A90D9]/10 rounded px-1 py-[1px]`), same label text ("Squire"), same `title` copy
("This session uses Squire's curated protocol context"). The task explicitly asked for this
("matching the existing 'Squire' badge's style/conventions from the sidebar, for
consistency") and it also keeps the two indicators trivially recognizable as "the same
concept, shown in two places" rather than introducing a second visual vocabulary for
identical information.

**Show-condition: Squire only, matching the sidebar's convention — considered and rejected
showing "Legacy" explicitly too.** The task offered discretion here ("or use your judgment
if showing both explicitly reads better in that specific location"). Considered showing an
explicit "Legacy" label in the header for the common case, on the theory that the chat
header is a more sustained, always-in-view surface than a sidebar row glanced at briefly —
arguably worth the extra permanent visual noise for at-a-glance certainty during a long
session. **Rejected**, for consistency with the sidebar's own precedent and this epic's
established pattern of "absence signals the default" (this same reasoning is documented at
length in `session-creation-ux/decisions.md`'s "Visual indicator" section, and this node's
task explicitly asked for style/convention consistency with that existing badge). Introducing
a second convention (badge-only in the sidebar, but explicit-both-states in the header) for
the exact same underlying fact would be a needless inconsistency within one feature — a user
who has learned "no badge = Legacy" in the sidebar should not need to learn a different rule
for the chat header. If real usage later shows the chat header specifically needs a more
persistent "you are in Legacy mode" reminder (e.g. because sessions there run much longer
than a sidebar glance), that is a small, easy, isolated follow-up — not built here to avoid
over-building past what a "small, minimal visual element" calls for.

**No indicator renders when no conversation is selected** (`activeContextMode` is
`undefined` in that case) — there is no "active conversation" to describe a mode for, so
nothing is shown, matching the existing empty-state message shown in the same area.

## Manual/e2e verification: extended `session-creation-ux.test.ts` for persistence, unit/component tests for the header indicator

**Toggle persistence:** extended the existing `e2e/specs/session-creation-ux.test.ts` with
one additional case that toggles Squire on, remounts the Sessions view (navigating away to
the Chat tab and back, which — per this node's own env.md finding — does *not* actually
unmount `ConversationSidebar` today, so this specifically exercises the `localStorage`
round-trip path by also reloading the page via `browser.refresh()` to force a true remount)
and confirms the toggle is still checked afterward. This was judged proportional to extend
(not skip) because the existing spec already drives exactly this UI surface end-to-end
against a real running app, and the marginal cost of one more assertion in an
already-working spec is small — unlike building a whole new spec file for a change this
size.

**Chat header indicator:** judged a new full WDIO e2e case disproportionate for a
purely-presentational, purely-derived (`useMemo` over already-tested data) UI element with
no state transitions, side effects, or IPC calls of its own — the badge is a deterministic
function of `conversations`/`activeConversationId`, both already covered end-to-end
elsewhere (`session-creation-ux.test.ts` already proves a real Squire-mode session's
`contextMode` reaches the frontend correctly; this node's own component test proves the
badge renders correctly from that same shape of data). A new `chat-panel.test.tsx` component
test suite (3 cases: no badge for legacy, badge for squire with correct title text, no badge
when no conversation is active) gives equivalent confidence at a fraction of the runtime/
flakiness cost of a fresh WDIO run, consistent with how `tool-token-ingestion`/
`user-input-chunking`/`raw-partition-storage` scaled verification effort to change size and
risk in this same epic (those nodes skipped e2e entirely for no-frontend-surface changes;
this node has frontend surface but it is presentational-only, so unit/component coverage —
not zero coverage — is the proportional middle ground, not a full e2e spec).

## A real, pre-existing e2e flakiness was found and fixed while verifying the toggle-persistence case

While running `session-creation-ux.test.ts` repeatedly to confirm the new persistence case
wasn't flaky, an *existing* test (`creates a real squire-mode session via the toggle...`,
unmodified by this node beyond context) intermittently failed with the created session
coming back `legacy` even though the toggle's `data-state` had just been asserted
`checked`. Root cause: that assertion used a single, non-polling `expect(await
toggle.getAttribute('data-state')).toBe('checked')` immediately after `toggle.click()` —
correct most of the time because the click's resulting React re-render is normally fast
enough to win the race against WebDriver's round-trip to read the attribute back, but not
guaranteed. This node's persistence write (`saveStoredSquireModeDefault` inside the same
`onCheckedChange` handler that also calls `setNextSessionSquireMode`) does a small amount of
extra synchronous work in that handler, which made the pre-existing race meaningfully more
likely to actually manifest in this environment (4 consecutive full-suite runs before the
fix: 1 failure; 4 consecutive runs after: 0 failures) — not a new hazard, but a latent
timing assumption in the original spec's own assertion style that this session's change
happened to surface.

**Fix:** replaced the one-shot `expect(...).toBe('checked')` with a polling
`browser.waitUntil(...)` (the same idiom already used everywhere else in this exact spec
file for state that updates asynchronously, e.g. waiting for `activeConversationId` to
become truthy) before proceeding to click "+ New session". This is a test-robustness fix
only — no production code changed as a result — and was applied narrowly to the one
assertion that raced, not as a speculative rewrite of the rest of the file.
