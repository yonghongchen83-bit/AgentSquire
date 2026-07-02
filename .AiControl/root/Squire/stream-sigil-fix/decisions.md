# Decisions

## Chosen approach: suppress live `stream-chunk` entirely in Squire mode (task's option 1)

Of the three design options the task laid out, this node picked the simplest one — do not
emit `stream-chunk` at all for Squire-mode turns; let the frontend's existing `stream-done`
handling (which already calls `getConversation` and reloads `messages` from the store) pick
up the finalized, sigil-expanded content once `finalize_turn` persists it.

This is also, word-for-word, what sa-4's own todo entry already prescribed ("don't emit
stream-chunk in Squire mode; emit expanded display content once at turn close instead") and
what `squire-adapter/decisions.md`'s original gap note described as the fix
("orchestration streaming change... not emitting `stream-chunk` at all in Squire mode and
instead emitting the expanded content once at turn close"). No new design was needed here —
the fix earlier nodes deferred was already fully specified; this node's job was to implement
exactly that, verify it doesn't regress Legacy mode, and give it real test coverage.

**Rejected: incremental sigil-masking during streaming (task's option 2).** Detecting and
masking/stripping `§!`/`§^` spans as tokens arrive, showing a placeholder for in-progress
spans, would require a streaming-aware sigil parser (tracking partial-match state across
chunk boundaries — e.g. a chunk boundary landing mid-`§!TokenID` or mid-span) plus a way to
resolve `§!TokenID` to a `short_desc` *before* the token is necessarily known to exist yet
(inline refs can point at tokens defined in the same turn's `new_tokens`, which isn't known
until the full response is parsed at close). This is a materially larger, genuinely new
piece of streaming-parser machinery — exactly what the task's instructions warned against
("not a new streaming framework"). It would also only partially solve the problem: the
*outer* JSON envelope (`{"ask_user": "", "content": "...`) is itself not valid to show
verbatim either, and incremental JSON-body extraction while streaming is its own can of
worms this node has no reason to open for a "display boundary" bug.

**Rejected: incremental deterministic expansion of partial streamed content (task's option
3).** `finalize_turn`'s expansion (`expand_for_display`) is deterministic given the full
response text, but it operates on the *parsed* `SquireResponse.content` field — the raw
stream text isn't `content` directly, it's the surrounding JSON envelope containing
`content` as one field among several (`ask_user`, `preserve`, `new_tokens`, `relationships`).
There's no way to apply `expand_for_display` meaningfully to a half-received JSON blob
without first solving incremental JSON parsing of a streaming, potentially-malformed-until-
complete document — again, materially more machinery than a "display boundary" fix
warrants, and not something any part of this codebase does today for any other purpose.

**Conclusion:** losing the live-typing effect during Squire-mode generation is an accepted,
explicit UX tradeoff — Squire turns already involve multiple tool calls
(`explore`/`token_to_detail`/`invoke`) before content generation even starts, so "live
typing" was already a partial illusion for Squire mode specifically (tool-call phases show
no live text today either, only `stream-status` line updates). The user still sees
`stream-status` updates throughout ("Contacting model...", "Model requested tool
execution...", etc.) so the UI is not frozen or silent — just without live prose — and the
final answer appears immediately at turn close via the pre-existing `stream-done` →
`getConversation` reload path, unchanged by this fix.

## Extracted a pure `should_stream_live_chunks` function rather than an inline boolean

`streaming_cmd.rs` has zero existing tests (`send_message_impl` is a single large
`AppHandle`/`State`-coupled async function with no `#[cfg(test)] mod tests` anywhere in the
file before this session) — there was no established pattern for testing orchestration
logic in this file directly. Rather than leave the mode-gating decision as an untested
inline `let is_squire_mode = matches!(...)` boolean, it was extracted into a standalone
function, `should_stream_live_chunks(context_mode: ContextMode) -> bool`, purely so the
actual policy decision (which context modes get live streaming) has direct unit-test
coverage independent of the untestable Tauri plumbing around it. This is the same "extract
the pure decision, leave the untestable orchestration wrapper thin" pattern already used
throughout `squire.rs` (e.g. `effective_priority`, `classify_rejection_rule`,
`sort_by_score_then_priority` are all free functions tested directly, called from inside
less-testable trait-impl methods).

Two unit tests cover both branches (`Legacy` → `true`, `Squire` → `false`). No third branch exists since
`ContextMode` is a two-variant enum (`session-mode`'s `Legacy | Squire`) — exhaustive
coverage by construction (`matches!` would fail to compile against a non-exhaustive match
if the enum ever grows a variant without this function being revisited, though `matches!`
against `ContextMode::Squire` specifically doesn't require the enum to be exhaustively
listed, so a new third variant would silently default to "streams live" here — flagged as a
minor forward-compatibility note, not a real risk today since `ContextMode` has had exactly
two variants since `session-mode` and no node has proposed a third).

## Also gated: the two tool-loop-path `stream-chunk` emissions, not just the main chunk handler

sa-4's todo text names the general problem ("raw model output... streamed live... before
finalize_turn ever parses/expands it") and its worked example is the main per-token
`StreamEvent::Chunk` handler, but `streaming_cmd.rs` has two more `stream-chunk` emission
sites inside the `FinishReason::ToolCalls` branch: a `"\n\n"` separator emitted once before
tool execution starts (if `full_response` is non-empty), and a
`"[Executing {tool_name}...]\n"` progress line emitted after a destructive-tool approval is
granted. Neither of these carries sigil markup itself, but both are part of the exact same
live-display channel sa-4 is about, and leaving them live in Squire mode would produce a
visibly broken half-suppressed stream (silence during content generation, but stray
formatting artifacts appearing whenever the model calls `explore`/`token_to_detail`/`invoke`
mid-turn) — worse than either fully-live or fully-suppressed. Gated both behind the same
`stream_live_chunks` flag for a consistent, complete suppression in Squire mode, rather than
leaving a partial fix that only addressed the literal chunk-handler line sa-4's todo text
happened to describe.

Considered and rejected: leaving these two as always-live "meta" progress indicators distinct
from "raw model output," reasoning that they're orchestration-authored strings, not
model-authored sigil-laden content, so they don't violate the letter of sa-4's concern.
Rejected because the spec's actual guarantee (§14: "no protocol artefacts are ever visible to
the user... display expansion... before printing to the user") is about the *user-facing
display boundary* being clean during a Squire turn, not narrowly about sigil characters —
showing `[Executing explore...]` mid-turn in the chat pane, when the built-in tool calls are
supposed to be an internal, Squire-mediated implementation detail the user never explicitly
asked to see (spec §6: "the AI never sees raw MCP — the Squire is the sole MCP gateway," a
transparency guarantee that implicitly extends to the user's view too, per §7.2's framing of
tool calls as happening "during turn," not part of the displayed conversation), is exactly
the kind of protocol/orchestration leakage sa-4 is about in spirit, not just in its literal
example.

## Did not touch `finalize_turn`, `expand_for_display`, or any sigil-parsing code

This fix is purely about *which events get forwarded to the live UI channel*, not about how
sigils are parsed or expanded — that logic (`expand_for_display`, `extract_inline_refs`,
`extract_spans`, `strip_span_markers` in `squire.rs`) was already correct and already has its
own dedicated unit test coverage from `squire-adapter`'s original session (verified by
reading through it this session — see the "sigil parsing" and "validation gates" test groups
in `squire.rs`). sa-4 was never a bug in the expansion logic itself; the finalized, persisted
message was always correct (confirmed by `squire-adapter/decisions.md`'s and
`rejection-ux/decisions.md`'s original framing: "final persisted history is already
correct"). The bug was purely that a *second, earlier* copy of the raw text was also being
shown, live, before expansion happened. Fixing the leak by suppressing that second copy is a
strictly display-boundary/orchestration change, matching the scope both `squire-adapter` and
`rejection-ux` judged this to be when they each declined to absorb it.

## Not manually verified in a running app instance this session

See `state.md` for the full reasoning — no interactive GUI/desktop-window verification is
practical in this sandboxed CLI environment (no way to launch and visually interact with a
Tauri window, and Squire mode additionally requires a configured LLM provider/API key to
actually generate a turn to observe). Relied on: (1) the two new unit tests directly
exercising the mode-gating decision this fix hinges on, (2) manual code-path tracing
confirming `full_response`/`finalize_turn`'s inputs are completely unchanged by this fix (only
the `app_clone.emit("stream-chunk", ...)` call sites are conditionally skipped — nothing about
what gets accumulated, parsed, validated, or persisted changed), and (3) the pre-existing
`squire.rs` test coverage for `expand_for_display` and the sigil-parsing helpers, which
together confirm the *destination* of the suppressed-then-restored content (the finalized
message) is unaffected and correct.
