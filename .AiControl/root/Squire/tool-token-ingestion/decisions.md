# Decisions

## Trigger point: the existing per-turn `ToolRegistry` construction in `streaming_cmd.rs`, not a new event system

`squire-storage/decisions.md`'s original ss-9 pointer speculated ingestion would live
"likely in `streaming_cmd.rs` alongside MCP tool discovery" — reading the actual code confirms
this exactly, and confirms there is no separate "MCP server connect/disconnect" event system
to hook into instead. `ToolRegistry` is rebuilt from scratch at the top of every single turn's
`tokio::spawn`'d task (`send_message_impl`, both Legacy and Squire mode), which already:
registers the two local built-ins (`TerminalTool`, `WebFetchTool`), then loops over enabled
`McpServerConfig`s calling `crate::mcp::discover_tools` and registering each result as an
`McpProxyTool`. This loop's completion (`let tool_registry = Arc::new(tool_registry);`) is the
one point in the codebase where "the full, current set of available tools" is known — no
earlier and no more often. Ingestion is wired as a single call immediately after this point,
before the registry is wrapped in `Arc` (so it can still take `&ToolRegistry` by reference,
consistent with how `tool_registry.definitions()` is otherwise called).

**Considered and rejected: app-startup-only ingestion.** MCP servers can be enabled/disabled
via config at any time and `ToolRegistry` deliberately has no persistent/cached form — discovery
already re-runs every turn as an existing, unavoidable cost of building `ChatRequest.tools`.
Ingesting only at startup would leave the store permanently stale the moment a user toggles an
MCP server on/off without restarting the app, which defeats the point of a "real tool-token
ingestion" feature (ss-9's own wording: "turns MCP/local tool discovery into persisted...
rows" — discovery already happens continuously, ingestion should track it).

**Considered and rejected: a dedicated ingestion trigger fired only on config change.** This
would need new event-wiring (a config-save hook, a "tools changed" signal) that doesn't exist
today, for a benefit (avoiding redundant per-turn upserts) that's already cheap in practice —
see the "repeated ingestion cost" note below. Reusing the trigger point that already fires,
already has the data assembled, and already runs once per turn regardless, is simpler and
strictly no worse in staleness characteristics (it's always at least as fresh as the model's
own view of `ChatRequest.tools` for that exact turn, by construction — same registry, same
turn).

## Ingestion applies in both Legacy and Squire mode, unconditionally

`tool_registry` is built identically regardless of `session.session.context_mode` — the mode
branch only affects which *dispatch* registry the tool-call loop executes against
(`dispatch_registry`), not whether the full registry is assembled. Gating ingestion to
Squire-mode turns only would require threading `context_mode` earlier than it's otherwise
needed and would leave the store's tool-token view depending on which mode happened to run
most recently — a confusing, order-dependent staleness story for no real benefit, since writing
tool tokens is harmless and inert for Legacy-mode sessions (nothing reads
`explore()`/`tool_skill` results outside Squire mode; `SquireExploreTool` is never constructed
for Legacy sessions). Ingestion is unconditional: any turn, either mode, populates/refreshes
the same shared store.

## Ingestion function: one new free function in `agent::squire`, calling the existing `SquireStore::upsert_token` — no new trait method

```rust
/// Ingests the app's real tool registry into `store` as `tool_skill` tokens
/// (spec §3.1: "Tool: Standard MCP tool schema"). Backend-agnostic — calls
/// only `SquireStore::upsert_token`, so it works unmodified against both
/// InMemorySquireStore and LanceDbSquireStore.
pub async fn ingest_tool_registry(registry: &ToolRegistry, store: &dyn SquireStore) {
    for def in registry.definitions() {
        store.upsert_token(
            NewTokenSpec {
                id: tool_token_id(&def.name),
                token_type: "tool".to_string(),
                short_desc: def.description.clone(),
                full_desc: Some(tool_full_desc(&def)),
            },
            0, // see "creation_turn" decision below
        ).await;
    }
}
```

The `SquireStore` trait (read in full before starting, per `squire-storage/decisions.md`'s own
precedent of confirming trait shape before assuming a change is needed) already has exactly
the right method: `upsert_token(NewTokenSpec { id, token_type, short_desc, full_desc },
creation_turn)`. No new trait method, no `record_hit`-style special case, no per-backend
branch — `ingest_tool_registry` takes `&dyn SquireStore` (a trait object reference, matching
how call sites elsewhere in `streaming_cmd.rs` already hold `Arc<dyn SquireStore>` and can
pass `&*squire_store`) and is entirely backend-agnostic, satisfying the task's explicit
"ingestion logic itself should probably be backend-agnostic... so it doesn't need duplicating
per backend" guidance. This is the same shape `retrieval-fidelity`'s shared free functions
(`effective_priority`, `sort_by_score_then_priority`, `traverse_relationships`) already
established as this codebase's idiom for backend-agnostic Squire logic living in `agent::squire`
and being exercised by both stores' test suites.

## Token ID scheme: `tool_token_id(name) = name` (the registry name itself), asserted stable by construction

`ToolDefinition.name` is already the right stable, deterministic identifier:
- Local built-ins (`TerminalTool` -> `"run_terminal"`, `WebFetchTool` -> `"web_fetch"`, etc.)
  have fixed, hardcoded names that never change turn to turn.
- MCP tools get their local name assigned once per discovery pass by
  `streaming_cmd.rs`'s existing sanitization scheme: `mcp_{server_id}_{tool_id}` (both
  components alphanumeric-only, collision-suffixed with `_2`, `_3`, ... if two servers happen
  to produce the same sanitized pair). This name is a pure function of
  `(server.id, remote_tool_name)` — as long as neither the server's configured `id` nor the
  remote tool's own advertised name changes, re-discovery on every subsequent turn produces the
  *exact same* local name every time, which is exactly the determinism ss-9 asked for ("must be
  stable/deterministic so re-ingestion updates rather than duplicates").
- `token_id = name` verbatim (no added prefix like `TOOL_` — see below) keeps
  `SquireInvokeTool`'s existing fallback (`store.token_detail(token_id)`, keyed by whatever the
  model passed as `token_id` in its `invoke()` call) trivially consistent with
  `tool_registry.get(token_id)`'s primary lookup, which is also keyed by the exact registry
  name. If the ingested token's id didn't match the registry name, a token discovered via
  `explore(resource_type="tool_skill")` would show the model an id it couldn't actually
  `invoke()` against the live registry — silently reintroducing a dead-end ss-9 exists to close.

**Considered and rejected: a `TOOL_`-prefixed synthetic id (e.g. `TOOL_run_terminal`),
matching the spec's illustrative examples (`TOOL_Weather`, `TOOL_IPLocation`).** The spec's
examples are illustrative naming convention, not a literal contract — nothing in
`validate_squire_response`, `SquireStore`, or any existing token ever enforces a `TOOL_`
prefix (the AI's own `new_tokens` ids are free-form strings the model chooses per spec §3.2's
"unique identifier, no spaces or § characters"). Prefixing would break the
`token_id == registry name` invariant the previous paragraph relies on for `invoke()` to work
without translation, for a purely cosmetic naming-convention match with no functional benefit
— and would require a translation layer (`registry name` <-> `TOOL_`-prefixed token id) that
doesn't exist anywhere today and that no other part of this node's scope asks for. Not
implemented.

## Content shape: `short_desc` = the tool's own `description`, `full_desc` = a small JSON envelope matching the "standard MCP tool schema" spec wording

```rust
fn tool_full_desc(def: &ToolDefinition) -> String {
    serde_json::json!({
        "name": def.name,
        "description": def.description,
        "input_schema": def.input_schema,
    }).to_string()
}
```

This exactly matches `SquireTokenToDetailTool::execute`'s own existing `detail_level == "full"`
JSON shape for registry-sourced tools (`{"name", "description", "input_schema"}, `already
implemented and tested before this node) — reusing the same shape means a caller who gets a
tool's full detail via the registry-primary path and one who gets it via the
store-fallback path (once ingested) see byte-for-byte the same JSON structure, not two
different "full tool description" conventions for the same conceptual resource. `short_desc`
is simply the description string, matching every other token type's `short_desc` convention
(`"one or two sentences; shown in explore() results and prefetch lists"`, spec §3.2) and
consistent with `SquireExploreTool`'s existing live-registry `tool_skill` results, which also
use `d.description.clone()` as `short_desc`.

## `creation_turn`: `0` for all tool tokens, not tied to any session's turn counter

Tool discovery/ingestion happens once per turn in `streaming_cmd.rs`, *before* the
session-specific adapter is constructed and before any session's `current_turn`/
`increment_turn` bookkeeping is touched — and it is explicitly not scoped to one session (the
same `ToolRegistry` build serves whichever session's turn triggered it, but conceptually "the
tools that exist" isn't itself a per-session fact the way `preserved_tokens`/turn count are).
Rather than arbitrarily attaching a tool token's `creation_turn` to whichever session happened
to trigger the most recent ingestion (which would make `effective_priority`'s
`current_turn - creation_turn` term depend on an unrelated session's turn count — a confusing,
cross-session-coupled side effect a purely infrastructural write path shouldn't introduce),
`creation_turn` is passed as `0` and, on every re-ingestion of an existing tool, is preserved
unchanged by `upsert_token`'s existing "keep existing creation_turn on update" semantics (both
stores already do this for every other token type — confirmed by reading
`InMemorySquireStore::upsert_token`/`LanceDbSquireStore::upsert_token`, no new logic needed).
This means a tool token's `effective_priority` decays exactly like a token created at turn 0
relative to whichever session/turn later queries it via `explore()` — a small, accepted
imprecision (tool tokens don't "age" the same way session-scoped memory tokens do) rather than
a cross-session coupling bug. Not flagged as a follow-up since it's a natural, low-stakes
consequence of tools being a session-independent resource, not an oversight.

## Repeated per-turn ingestion cost and `accumulated_hits` interaction: accepted, not worked around

Because ingestion runs on every turn (see trigger-point decision above) and
`SquireStore::upsert_token` increments `accumulated_hits` "regardless" on every call (both
stores' existing, `retrieval-fidelity`-established semantics, spec §9.4 step 5's literal
wording), an unchanged tool's `accumulated_hits` grows by 1 on every single turn across the
whole app, for every session, regardless of whether the model ever actually looked at or used
that tool. This inflates tool tokens' `effective_priority` over time independent of real usage
— arguably a mild semantic mismatch with spec §3.3's hit-count-event table (none of the four
listed events is "was re-ingested this turn"; the closest, "Token appears in explore() results
that AI acts on," is a usage signal, not a discovery-refresh signal).

**Considered and rejected: adding a "no-op if description/schema unchanged" guard before
calling `upsert_token`, to avoid the hit-count inflation.** This would need either a new
`SquireStore` method (a read-then-compare-then-maybe-write pattern, adding trait surface this
node's env.md's non-goals explicitly avoid) or reading back the existing token's `short_desc`/
`full_desc` via the already-available `token_detail`/`token_exists` before every write (doable
without a new trait method, but adds a read-modify-write round trip to every turn's tool setup
for every tool, for a real app where "MCP server config or a tool's own advertised schema
changed between turns" is normally a rare event compared to "the exact same server list
answers with the exact same tools it did last turn").

**Considered and rejected: bypassing `upsert_token`'s "regardless" hit-increment specifically
for ingestion, via a hypothetical `upsert_token_no_hit_increment` variant.** This is exactly
the kind of per-purpose trait-surface growth the "no new trait method" design goal in `env.md`
was meant to avoid, and it would special-case ingestion's writes to behave differently from
every other `upsert_token` caller (the model's own `new_tokens` writes, which are also spec-
mandated to increment "regardless") for a benefit (slightly more accurate `effective_priority`
staleness math for tools specifically) that doesn't block anything this node or ss-9 asked to
fix — ss-9 is about *discoverability*, not ranking precision.

Decision: accept the inflation as documented behavior, not a defect. This is functionally the
same category of imprecision already accepted and documented for `creation_turn` above (tool
tokens don't fit the per-session-turn-scoped ranking model as cleanly as session-created memory
tokens do), tracked here rather than silently absorbed, and left as a candidate for a future
node if fuller hit-count fidelity is ever revisited (the existing, still-open
`retrieval-fidelity/todo.json` rf-13 is the natural place such a refinement would belong,
since it already tracks "hit-count-event fidelity" as its own scoped concern — not claimed by
this node).

## Tool removal / staleness: no active cleanup — tokens persist as historical/informational rows

If an MCP server is later disabled, uninstalled, or a tool is removed from its `tools/list`
response, that tool's already-ingested token is **not** deleted or marked stale by this node.
Considered three options:

1. **Active sweep-on-ingestion**: at the top of each turn's ingestion pass, diff the newly
   discovered tool-name set against all `token_type = "tool"` rows currently in the store and
   delete/mark any no-longer-present ones. Rejected as disproportionate for this node: it
   requires a new store capability to enumerate/delete tokens by type (`SquireStore` has no
   "list all tokens of a type" or "delete token" method today — `explore_memory` is a scored,
   type/query-filtered *search*, not an unfiltered enumeration primitive, and adding a
   delete-by-id method is exactly the kind of new trait surface this node's env.md's
   non-goals steer away from unless truly necessary). It would also run on every single turn
   (same cost profile as the ingestion write itself) for a scenario (a tool disappearing
   between turns) that is comparatively rare relative to the steady-state case (the same tools
   available turn after turn).
2. **A separate, explicit "prune stale tool tokens" maintenance operation** (e.g. run once at
   app startup, mirroring `clear_all_preserve_lists`'s existing "once at startup" pattern) —
   considered more seriously, since it wouldn't add per-turn cost, but still needs the same
   missing enumerate/delete store primitives as option 1, and startup-only pruning wouldn't
   catch a server disabled mid-session anyway (matching this node's broader "ingestion tracks
   the registry live, per turn" design, a startup-only *removal* path would be an inconsistent,
   asymmetric complement to it).
3. **Leave stale tokens in place as informational history (chosen).** A tool token that's no
   longer live simply stops being re-ingested/re-upserted (its `accumulated_hits` stops
   growing, since ingestion only touches tools currently present in `tool_registry`) but
   remains discoverable via `explore(resource_type="tool_skill")` and describable via
   `token_to_detail()` — its `full_desc` still accurately describes a tool that *did* exist and
   *was* invocable at some point. If the model tries to `invoke()` it while it's actually gone
   from the live registry, `SquireInvokeTool`'s existing fallback path already handles this
   correctly today (registry lookup fails, store lookup succeeds, returns the pre-existing
   "recorded in Squire storage... but has no invocable endpoint bound yet" diagnostic — this
   node does not need to change that message or add a new one, since it already describes
   exactly this state accurately). This matches the same "an unreferenced token drifts negative
   [`effective_priority`], no active sweep is required" philosophy spec §3.3 already states for
   ordinary memory tokens — the formula's natural decay is judged sufficient signal that a tool
   token hasn't been seen in a long time, without needing an explicit deletion mechanism this
   node would otherwise have to invent from scratch.

Chosen: **option 3, no active cleanup.** Consistent with the spec's own stated philosophy for
token staleness generally, requires no new `SquireStore` trait surface, and correctly composes
with `SquireInvokeTool`'s pre-existing (and unchanged by this node) fallback diagnostic for a
token that's recorded but no longer live. If a future session judges this insufficient (e.g.
if stale tool tokens visibly clutter `explore()` results in practice), a dedicated cleanup node
can add the necessary enumerate/delete primitives then — not invented speculatively here.

## Why `SquireExploreTool`'s live-registry read path for `"tool"`/`"tool_skill"` is left unchanged

`SquireExploreTool::execute`'s existing branch for `resource_type in {"tool", "tool_skill"}`
already reads directly from `self.tool_registry.definitions()` (bypassing the store) — this
was `squire-adapter`'s deliberate original design so the model always sees currently-real,
invocable tools with zero staleness window, and it still is exactly the right behavior: a live
registry read can never be stale in a way ingested store rows could be (e.g. between this
turn's ingestion write and a `record_hit`/removal race, however unlikely). This node's
ingestion does not redirect that read path to the store — doing so would trade a
zero-staleness live read for a store read that could theoretically lag by exactly the write
this same turn just performed, for no discoverability benefit (the live registry already
contains everything the store now also contains, by construction, since ingestion always runs
from that same registry). Ingestion exists so the *store side* is populated for
`SquireInvokeTool`'s fallback and for any future work that wants to query tool tokens
independently of a live registry snapshot (e.g. history/audit use cases, or graph traversal
`num_hops` connecting a tool token to a workflow/skill token via `relationships` — tool tokens
now participating in `explore_memory`'s traversal, which they could not before this node, since
they never existed as rows at all).

## Verification methodology: unit tests (both backends) plus a real headless integration harness against real LanceDB — no WDIO/GUI spec

This node is a pure backend write path with **no new user-facing surface** — its effect is
only observable through `explore()`/`token_to_detail()`/`invoke()` results that already existed
and are already exercised by the model's own tool-call loop, unchanged in shape. Unlike
`ask-user-loop` (a new pause/resume UI interaction, where a real WDIO+tauri-driver spec was the
right, proportionate verification), there is no new frontend behavior here for a GUI spec to
exercise — a WDIO spec for this node would, at best, indirectly infer "ingestion worked" by
asking a real model to call `explore()` and hoping it mentions a tool by name in its answer, a
strictly weaker and less direct signal than asserting on the actual store rows.

Two verification layers were used instead, both completed:

1. **13 new unit tests** (8 in `agent::squire`'s `InMemorySquireStore`-backed suite, 5 in
   `storage::squire_lancedb`'s real-`LanceDbSquireStore`-backed suite) covering: a token is
   written per registry tool; the token id exactly matches the registry name (the id-scheme
   invariant `SquireInvokeTool`'s dispatch depends on); `full_desc` matches the same JSON shape
   `SquireTokenToDetailTool`'s existing registry-path already returns; repeated ingestion
   updates rather than duplicates; a changed tool's description is reflected on re-ingestion;
   an empty registry ingests nothing; and (in `squire.rs` only) a full trace confirming
   `SquireInvokeTool` can dispatch to a tool using exactly the id `ingest_tool_registry` chose
   for it. `cargo test --lib`: **171/171 passing** (158 baseline + 13 new).
2. **`src-tauri/examples/tool_token_ingestion_e2e.rs`** (new, headless, no network/no LLM
   needed — unlike `ask_user_e2e.rs`, ingestion is deterministic Rust, not model-dependent
   behavior) — runs the exact real production call chain (`ToolRegistry` construction ->
   `ingest_tool_registry` -> `SquireExploreTool`/`SquireTokenToDetailTool`/`SquireInvokeTool`)
   against a real temp-directory `LanceDbSquireStore`, with a hand-registered tool shaped
   exactly like a real MCP-discovered tool (`mcp_weatherserver_get_forecast`, matching
   `streaming_cmd.rs`'s real `mcp_{server_id}_{tool_id}` naming convention) standing in for a
   live MCP server subprocess (this machine does have a real configured MCP server binary —
   see state.md — but spinning up a live stdio JSON-RPC handshake was judged unnecessary
   ceremony for what is, from `ingest_tool_registry`'s point of view, just another
   `ToolDefinition` regardless of origin). Confirmed, against the real LanceDB backend: (a) a
   real row is written and immediately discoverable via `explore(resource_type="tool_skill")`
   (the live-registry read path, unchanged, still works correctly alongside ingestion); (b)
   `token_to_detail` resolves the *ingested* row correctly even with a deliberately **empty**
   live registry passed to `SquireTokenToDetailTool` — this is the exact dead-end scenario ss-9
   was filed to close, now proven closed against a real backend, not just asserted in a unit
   test; (c) three consecutive ingestion passes (simulating three turns) leave exactly one row
   per tool, not three; (d) `SquireInvokeTool` dispatches correctly to the real tool using
   the exact id `ingest_tool_registry` wrote, proving the token-id-scheme decision holds
   end-to-end across all three built-in tools' real code, not just within one function's
   isolated unit test.

**Incidental finding, not part of this node's scope, documented for future sessions:** running
this LanceDB-backed example under `tokio::main`'s default current-thread runtime + default
Windows main-thread stack size caused a real stack overflow (`STATUS_STACK_OVERFLOW`) purely
from LanceDB's own internal async call depth — reproducible, unrelated to any code this node
wrote (`ingest_tool_registry`/`SquireStore` trait calls are shallow). Existing `#[tokio::test]`-
based LanceDB tests in `squire_lancedb.rs`'s own test module are unaffected (tokio's test-thread
stack sizing differs from a bare example binary's `fn main`), and the real Tauri app is also
unaffected (`setup_cmd.rs` already runs its one LanceDB-touching startup call via
`tauri::async_runtime::block_on`, a different runtime/thread setup than a bare `#[tokio::main]`
example). Worked around locally in this one example by spawning a dedicated OS thread with a
64MB stack and a manually-built multi-thread runtime — not a change to any production code path,
just this verification harness. Flagged here in case a future example/harness touching
`LanceDbSquireStore` hits the same thing.

