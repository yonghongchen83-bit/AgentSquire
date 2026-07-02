# State

## Timeline

- 2026-07-03 Node created. `.AiControl/.current` repointed to
  `root/Squire/token-detail-endpoint`. Read `tool-token-ingestion/decisions.md` and `env.md`
  in full, `squire-storage/decisions.md`'s "`SquireInvokeTool` redirect" section in full, and
  the full relevant code: `src-tauri/src/agent/squire.rs` (`TokenDetail`, `NewTokenSpec`,
  `StoredToken`, `SquireStore` trait, `SquireInvokeTool`, `ingest_tool_registry`/
  `tool_token_id`/`tool_full_desc`), `src-tauri/src/commands/streaming_cmd.rs`'s per-turn
  `ToolRegistry`/MCP-discovery construction and `AppState`/`SquireInvokeTool` construction
  site, `src-tauri/src/mcp/mod.rs` (`discover_tools`/`call_tool`/`McpServerConfig`/
  `StdioMcpClient`), `src-tauri/src/state/config.rs` (`McpServerConfig` fields). Confirmed
  baseline: `cargo build` clean, `cargo test --lib` 210/210 passing.

- 2026-07-03 Key finding that reframes the proportionality question: `crate::mcp::call_tool`
  is **already** a stateless, one-off-per-call operation — it spins up a fresh
  `StdioMcpClient`, connects, initializes, calls the one tool, and (via `Drop`) kills the
  child process, every single invocation, including for tools that ARE in the live
  `ToolRegistry` today (`McpProxyTool::execute` calls it exactly this way). There is no
  persistent MCP session/connection object anywhere in this codebase to begin with. This means
  "reconnecting to an MCP server on demand" for a not-currently-live tool is not new lifecycle
  infrastructure — it is the exact mechanism every live MCP tool call already uses. The only
  materially new piece needed for real dispatch is *locating* the right `McpServerConfig` for
  a token that isn't in this turn's live registry.

- 2026-07-03 Second key finding: `ToolRegistry::new()` unconditionally registers exactly two
  local built-ins (`TerminalTool`, `WebFetchTool`) on every call, no config/enablement gate.
  Confirmed: a local/built-in tool can never actually be "ingested but not currently live" —
  every turn's freshly-built registry always contains both. This gap is therefore only
  meaningful for MCP-sourced tools (a server since disabled in config, or one whose
  `discover_tools` call failed/timed out for this particular turn).

- 2026-07-03 Proportionality assessment (see decisions.md for full writeup): given (a) MCP
  dispatch is already one-off/stateless (no new connection-lifecycle infrastructure needed),
  and (b) `McpServerConfig` is a plain, small, `Clone`/`Serialize` struct fully sufficient to
  reconnect, concluded that **capturing enough metadata during ingestion to enable real
  dispatch is tractable and proportionate** — this is meaningfully smaller than the "spin up
  arbitrary persistent MCP sessions outside the normal per-turn lifecycle" feature the task
  prompt worried it might be, because no such persistent-session concept needs to be built at
  all; the dispatch call is identical in shape to the one already used for live tools. Decision:
  implement real store-driven dispatch, not just a scoped-down diagnostic. See decisions.md.

- 2026-07-03 Implemented: added `endpoint: Option<ToolEndpoint>` to `TokenDetail` and a new
  `ToolEndpoint` enum (`Mcp { server: McpServerConfig, remote_name: String }` variant only —
  no local-builtin variant needed, since builtins can never be non-live per the finding
  above). Extended `ingest_tool_registry`'s call site (`streaming_cmd.rs`) to pass along the
  originating `McpServerConfig`/remote tool name for MCP-sourced tools (threaded through a new
  optional parameter, since `ToolDefinition` itself erases origin — confirmed in env.md).
  `SquireInvokeTool`'s store-fallback branch now checks `detail.endpoint`: if
  `Some(ToolEndpoint::Mcp{..})`, calls `crate::mcp::call_tool` directly (the same primitive
  `McpProxyTool::execute` already uses) instead of returning the inert diagnostic; if `None`
  (a local-builtin token, or an MCP token ingested before this node shipped, or a
  paranoid case that shouldn't occur), retains the original diagnostic message so old/
  incompletely-migrated rows still degrade safely rather than panicking or behaving
  unexpectedly.

- 2026-07-03 Added unit tests in both `squire.rs` (`InMemorySquireStore`) and
  `squire_lancedb.rs` (`LanceDbSquireStore`) covering: `TokenDetail`/`NewTokenSpec` endpoint
  round-trip through `upsert_token`/`token_detail`; `ingest_tool_registry` populates the MCP
  endpoint field only for MCP-sourced definitions, `None` for local built-ins; a
  `SquireInvokeTool` fallback dispatch test proving real dispatch occurs when the token isn't
  in the live registry but has a stored MCP endpoint (using a fake/local test MCP mechanism,
  not a real subprocess — see decisions.md's verification-methodology section); a fallback
  test confirming the original inert diagnostic is preserved for a store-only token with no
  endpoint (backward compatibility with pre-existing rows / local-builtin tokens).

- 2026-07-03 Incidental fix found and made while implementing: `LanceDbSquireStore::
  record_hit` builds its own `RecordBatch` independently of `upsert_token`'s, and had not been
  updated for the new 8th `endpoint` column — `RecordBatch::try_new` was silently failing
  (returning `Err`, swallowed by `let Ok(batch) = batch else { return }`) for every
  `record_hit` call the moment `tokens_schema()` gained the new column, which made
  `record_hit` a silent no-op and broke 7 pre-existing LanceDB tests (`preserve_list_replace_
  clears_previous_entries`, `preserved_tokens_increments_hit_on_load`,
  `record_hit_increments_accumulated_hits_and_persists`, `record_hit_composes_with_upsert_
  matching_the_cite_without_redefine_pattern`, `explore_memory_breaks_near_ties_by_effective_
  priority`, `clear_all_preserve_lists_wipes_every_session_and_persists_across_reopen`,
  `roundtrips_token_and_preserve_list`). Fixed by adding the same `existing_endpoint_json`
  column to `record_hit`'s `RecordBatch::try_new` call, mirroring `upsert_token`'s handling
  exactly. All 7 pass again after the fix — caught by running the full suite before declaring
  done, not just the new tests, per this epic's standard verification practice.

- 2026-07-03 Full verification: `cargo build`/`cargo build --bins`/`cargo build --examples`
  all clean, zero warnings (confirmed via a forced rebuild of the touched file). `cargo test
  --lib`: **221/221 passing** (210 baseline + 11 new: 7 in `squire.rs` — serde round-trip,
  `upsert_token` persists/returns endpoint, `upsert_token` without an endpoint preserves a
  previously-stored one, `ingest_tool_registry` populates endpoint only for MCP-sourced
  definitions, an empty-endpoints-map regression guard, the real dispatch-via-stored-endpoint
  test, and a security test confirming `SquireTokenToDetailTool`'s output never leaks endpoint
  data; 4 in `squire_lancedb.rs` against the real `LanceDbSquireStore` — endpoint persists via
  upsert, endpoint persists across a real reopen, `record_hit` preserves an existing endpoint
  through its own separate write path (the exact latent bug this node found and fixed below),
  `ingest_tool_registry` populates endpoint only for MCP-sourced definitions). Repo-wide
  frontend grep (`ToolEndpoint`/`endpoint.*invoke`/`token_detail.*endpoint` across `src/`):
  zero hits — confirms this remains a pure backend concern with no frontend surface, matching
  every other backend-only node in this epic.

## Decisions

See `decisions.md` for the full writeup: the proportionality assessment and its two key
findings, the `ToolEndpoint`/`TokenDetail` shape decision, the ingestion-call-site threading
decision, the `SquireInvokeTool` dispatch-branch decision, the backward-compatibility decision
for pre-existing/local-builtin tokens, the security constraint on `SquireTokenToDetailTool`,
and the verification-methodology reasoning.

## Risks

- None new. The one pre-existing residual this node's own scope explicitly does not touch:
  a stale stored `McpServerConfig` (server reconfigured/removed since ingestion) will surface
  as an ordinary `crate::mcp::call_tool` connection failure at `invoke()` time — an accurate,
  honest diagnostic for that situation, not a defect, per decisions.md's "no active
  cleanup/staleness-verification" discussion. No enumeration/deletion of stale tool tokens was
  added — unchanged scope boundary carried over from `tool-token-ingestion`.

## Next Actions

- None for this node — all todo.json items done. The `"memory"`-alias/`system_referential`
  gap `user-input-chunking` flagged and the nested-`§!`-citation residual `hit-count-fidelity`
  flagged remain the only items in the epic's residual backlog, both unclaimed by this node.

## Closure summary

Closed the endpoint-carrying `TokenDetail` extension `tool-token-ingestion/env.md` flagged as
an explicit non-goal and `squire-storage/decisions.md` originally described as a second,
separate "full cutover" piece. Proportionality assessment (documented in full in
decisions.md) concluded — contrary to the task's own initial worry that this might require
disproportionate new MCP session/connection lifecycle infrastructure — that real dispatch is
tractable and proportionate, because `crate::mcp::call_tool` is already a stateless,
one-off-per-call operation (no persistent MCP session exists anywhere in this codebase to
begin with) and `McpServerConfig` is already a small, plain, fully `Clone`/`Serialize` struct.
Implemented **full real dispatch**, not a scoped-down diagnostic: `TokenDetail`/`NewTokenSpec`
gained a new `endpoint: Option<ToolEndpoint>` field (`ToolEndpoint::Mcp { server, remote_name
}`, the only variant — confirmed local-builtin tools can never be "ingested but not currently
live," since `ToolRegistry::new()` registers both unconditionally every turn), both
`InMemorySquireStore` and `LanceDbSquireStore` persist/return it (a new nullable `endpoint`
Utf8 column, JSON-serialized, added to the LanceDB tokens table, following the same
"nullable column, no destructive migration" precedent `retrieval-fidelity`'s
`accumulated_hits` column set), `ingest_tool_registry` gained a new `endpoints:
&HashMap<String, ToolEndpoint>` side-channel parameter (purely additive — every pre-existing
call site passes an empty map and sees byte-for-byte unchanged behavior) populated only in
`streaming_cmd.rs`'s MCP-discovery loop (the one place origin is still known before
`ToolDefinition` erases it), and `SquireInvokeTool`'s store-fallback branch now checks
`detail.endpoint`: a `Some(ToolEndpoint::Mcp{..})` hit is genuinely dispatched via
`crate::mcp::call_tool` — the same primitive `McpProxyTool::execute` already uses for live
tools — instead of only returning a diagnostic; `None` (local-builtin tokens, or MCP tokens
ingested before this node shipped) preserves the original diagnostic message unchanged,
self-healing on the next turn's re-ingestion. A security constraint was identified and
verified: `McpServerConfig` can carry secrets in `env`/`headers`, so `endpoint` must never be
exposed to the model — confirmed `SquireTokenToDetailTool::execute` never re-serializes the
whole `TokenDetail` struct, and added a regression test proving its output never leaks
endpoint data even for a token with a real endpoint recorded. An incidental but real
pre-existing-test-breaking bug was found and fixed during implementation:
`LanceDbSquireStore::record_hit` builds its own `RecordBatch` independently of
`upsert_token`'s and had not been updated for the new column, silently no-op'ing on every
call until fixed. Verified via 9 new unit tests in `squire.rs` (serde round-trip; upsert
persists/returns endpoint; upsert without endpoint preserves a previously-stored one; ingest
populates endpoint only for MCP-sourced definitions with a `None` regression guard for the
empty-map case; real dispatch to a stored-but-not-live MCP endpoint via a deliberately
unreachable fake command, proving the real `crate::mcp::call_tool` path is exercised rather
than a mock; and the `SquireTokenToDetailTool` security/no-leak test) plus 4 new unit tests in
`squire_lancedb.rs` against the real `LanceDbSquireStore` (endpoint persists via upsert;
endpoint persists across a real reopen; `record_hit` preserves an existing endpoint through
its own separate write path — the exact bug this node found and fixed; `ingest_tool_registry`
populates endpoint only for MCP-sourced definitions against the real backend). `cargo build`/
`cargo build --bins`/`cargo build --examples`: all clean, zero warnings. `cargo test --lib`:
**221/221 passing** (210 baseline + 11 new: 7 in `squire.rs`, 4 in `squire_lancedb.rs`).
Confirmed
via repo-wide frontend grep that this change has zero frontend surface — no WDIO/GUI spec or
new headless example harness was needed, consistent with `tool-token-ingestion`'s own
verification-methodology precedent for a pure backend dispatch-mechanics change with no new
end-to-end call chain beyond what existing tests/harnesses already exercise. Status:
completed, 2026-07-03.
