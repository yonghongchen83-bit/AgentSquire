# Decisions

Backfilled 2026-07-03 from 17 completed child nodes' own `decisions.md` files, per the updated
`aicontrol-node` agent's write-back obligation. This file holds settled, epic-wide judgment
calls — durable "we decided X because Y" facts a future sibling node should inherit without
having to read every completed node's own `decisions.md` to rediscover. See each named node's
own `decisions.md` for the full reasoning; this is a summary, not a replacement.

## Architecture (Q1-Q7, `planning`)

The original architecture questions Q1-Q7 were resolved in `planning/decisions.md` and
`planning/implementation-readiness.md` before any implementation began — read those directly
for the full text if working on anything touching the adapter trait design, the LanceDB
storage decision (Q4), the strict tool boundary (Q5), validation/retry semantics (Q6), or
preserve-list lifecycle (Q7). Every implementation node since has built on these as settled,
not re-litigated them.

## Proportionality is the epic's central recurring judgment call

Nearly every follow-up node in this epic faced the same question: build full, complete
fidelity to some spec detail, or accept a lighter-weight, explicitly-documented approximation?
The consistent practice has been: **document the tradeoff explicitly, never silently drop
scope**, and lean toward the lighter-weight option *unless a careful reading of the actual code
shows the "heavier" option is cheaper than it first appears*. Two contrasting examples worth
knowing before assuming either default:

- **Declined as disproportionate**: a full context-composition audit trail (scanning every
  piece of content that ever enters context for embedded `§!` references) to close the last
  sliver of hit-count-event fidelity (`hit-count-fidelity`); active tool-token
  cleanup/staleness sweeps (`tool-token-ingestion`); a new timeout system for abandoned
  `ask_user` questions (`ask-user-loop` reused three pre-existing mechanisms instead).
- **Turned out tractable on inspection, built in full**: real MCP tool dispatch from
  store-only metadata (`token-detail-endpoint`) — initially assumed to need new "session/
  connection lifecycle infrastructure," but reading `src-tauri/src/mcp/mod.rs` showed MCP
  calls are already stateless and one-off, so real dispatch was just calling the same existing
  function, not a new category of feature.

**Lesson for future nodes**: read the actual code before assuming a "full" implementation is
disproportionate — the assumption can be wrong in either direction, and getting the read wrong
either over-builds unnecessary infrastructure or under-builds a scoped-down diagnostic when
the real thing was cheap all along.

## Raw partition = unmarked-only, not verbatim-everything (`raw-partition-storage`)

The spec's "raw partition" is specifically the portion of a compliant turn's AI output that
fell outside every closed `§^...§^` span — content the model produced but chose not to promote
into a structured memory token. It is not a full verbatim audit trail (that would be redundant
with the ordinary chat-history table). It is write-only, operator/debugging-facing — nothing
in the runtime ever reads it back into a turn.

## `"memory"` is a convenience alias, not a literal token type

`explore(resource_type="memory", ...)` expands to a fixed list of real token types (`concept`,
`referential`, and — since `memory-alias-fix` — `system_referential`). It is not itself a
stored `token_type` value. When adding a new token type to this codebase, check whether it
should also be added to this alias's expansion list in `type_matches` (duplicated in both
`InMemorySquireStore::explore_memory` and `LanceDbSquireStore::explore_memory`) — this was
missed once already when `system_referential` was introduced by `user-input-chunking` and sat
unfixed for two nodes' worth of work before being caught.

## Token ID schemes, settled

- **Tool tokens** (`tool-token-ingestion`): the registry's own tool name, verbatim,
  unprefixed — matches exactly what `invoke()`/`SquireInvokeTool` already key on, so a token
  discovered via `explore()` is immediately `invoke()`-able with no translation layer.
- **User-input chunk tokens** (`user-input-chunking`): `USR_T{turn}_{NNN}`, where `NNN` resets
  to `001` at the start of every turn (not a session-lifetime monotonic counter) — the only
  reading consistent with the spec's own worked example.

Both schemes rely entirely on `SquireStore::upsert_token`'s pre-existing "replace by id"
semantics for idempotent re-ingestion (update, not duplicate) — neither needed a new trait
method.

## Staleness/cleanup philosophy: no active sweeps

Consistent with spec §3.3's own "no active sweep required" framing: stale tool tokens (for
tools no longer live), preserve-list carryover (cleared only once, at app startup, per Q7 —
`rejection-ux`), and stale stored MCP endpoints (`token-detail-endpoint`) are all left as
informational history or surfaced as an ordinary, honest error at use-time, rather than
actively swept/validated/cleaned up on a schedule or via a new background process. This is a
deliberate, repeated pattern — don't add active cleanup machinery to any of these without a
concrete reason the passive approach is actually causing a problem.

## UX conventions, settled

- **Badge-only-for-the-non-default-case**: every visual mode indicator this epic added
  (sidebar row badge, chat-header badge — `session-creation-ux`, `session-ux-polish`) shows
  only for Squire-mode conversations, never for Legacy — Legacy is the default/expected case
  and gets no badge, minimizing UI noise. The chat-header badge deliberately reused the
  sidebar badge's exact classes/copy for visual consistency rather than inventing new styling.
- **Default Legacy, Squire as explicit opt-in**, at conversation-creation time only — matches
  pre-existing implicit behavior for anyone who never touches the toggle, and preserves
  `context_mode`'s immutable-by-construction guarantee (no "change mode later" capability
  exists or should be added).
- **Pause/resume human-in-the-loop pattern**: `ask-user-loop`'s `PendingAskUserQuestions`
  registry (`HashMap<String, oneshot::Sender<...>>` keyed by a fresh id, resolved by a
  dedicated Tauri command) mirrors the pre-existing `PendingApprovals` tool-approval pattern
  exactly. Any future feature needing to pause a turn, surface something to the human, and
  resume with their answer should reuse this same shape rather than inventing a new one.

## Epic-closeout status

As of `memory-alias-fix` (2026-07-03), every node in the originally-planned sequence plus
eleven follow-up nodes are complete. The one remaining documented residual — a `full_desc`
body citing another token via `§!`, not itself scanned for embedded references
(`hit-count-fidelity`) — was explicitly reviewed with the user and kept as a **permanent,
intentional simplification by direct decision**, not a deferral. It should not be re-opened as
backlog absent a new, separate decision to do so. See `handoff.md` for the full operator-facing
status and the recommendation that the epic be marked complete at this level.
