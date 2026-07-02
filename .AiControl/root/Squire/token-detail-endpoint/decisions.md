# Decisions

## Proportionality assessment: full dispatch is tractable and proportionate — NOT the disproportionate feature the task prompt worried it might be

The task prompt (and `tool-token-ingestion/env.md`'s original non-goal framing) both
anticipated that closing this gap for real might require "session/connection lifecycle
management outside the normal per-turn flow" — i.e. something structurally bigger than most of
what this epic has built so far, on the order of the disproportionate-infrastructure concerns
this epic has deliberately declined elsewhere (e.g. `retrieval-fidelity/decisions.md`'s
rejection of a full context-composition audit trail, `tool-token-ingestion/decisions.md`'s
rejection of an active tool-token sweep/cleanup mechanism). Before assuming that's true, this
node read the actual MCP dispatch code in full rather than reasoning from the task's own
framing, and found two facts that change the calculus substantially:

**Finding 1 — MCP dispatch in this codebase is already stateless and one-off-per-call.**
`crate::mcp::call_tool(server: McpServerConfig, name: String, arguments: Value)`
(`src-tauri/src/mcp/mod.rs`) does not reuse any persistent connection — every single call
spawns a fresh `StdioMcpClient::connect`, runs `initialize()`, calls the one tool, and returns;
`StdioMcpClient`'s `Drop` impl kills the child process immediately afterward. This is not a
hypothetical fallback path — it is the *exact* mechanism `McpProxyTool::execute` (the live
registry's own, already-working, currently-shipping MCP dispatch path) uses for every ordinary
MCP tool call today, live or not. There is no "session," no connection pool, no persistent
subprocess anywhere in this codebase's MCP layer to begin with — so there is no existing
lifecycle for a not-currently-live dispatch to awkwardly bolt onto or duplicate. Reconnecting
"on demand" to a specific MCP server for one `invoke()` call is not a new category of
operation; it is calling the same function, `crate::mcp::call_tool`, that the live path already
calls, with the same argument shape it already takes.

**Finding 2 — `McpServerConfig` is a small, plain, fully `Clone`/`Serialize`/`Deserialize`
struct** (`id`, `name`, `transport`, `command`, `args`, `url`, `enabled`, `env`, `headers`) —
exactly what `call_tool` needs, and exactly the kind of value that's trivial to persist
alongside a token at ingestion time and read back later. No new "endpoint description" schema
had to be invented; the type that already fully describes "how to reach this MCP server" already
exists and is already used for the live path.

**What would still be missing without this node:** `ToolDefinition` (the registry's own
tool-metadata type, and what `ingest_tool_registry` reads from) erases MCP-vs-local origin and
the specific `McpServerConfig`/remote-tool-name pair once a tool is registered — this
information exists at the point `streaming_cmd.rs` builds `McpProxyTool` instances (it has
`server.clone()` and `remote_tool_name` right there), but is never captured by
`ingest_tool_registry`, which only ever sees the erased `ToolDefinition`. This is the one
genuinely missing piece, and it is a data-plumbing gap (thread two more pieces of already-
available information through one more optional parameter), not a missing capability.

**Conclusion: closing this gap for real is proportionate.** It requires:
1. A new field on `TokenDetail` (and the underlying stored-token shape, both backends) to carry
   an optional endpoint descriptor.
2. Threading the already-available `McpServerConfig`/remote-tool-name pair from
   `streaming_cmd.rs`'s MCP-discovery loop into `ingest_tool_registry`'s call, for MCP-sourced
   tools only.
3. One new branch in `SquireInvokeTool`'s existing store-fallback path calling
   `crate::mcp::call_tool` directly when a stored endpoint is present, instead of always
   returning the inert diagnostic.

None of this requires inventing a connection pool, a reconnection-retry policy, a "which
servers are currently reachable" cache, or any lifecycle concept beyond what `call_tool`
already does per ordinary call. This is a smaller, more mechanical change than, for instance,
`user-input-chunking`'s chunking-boundary design or `raw-partition-storage`'s new table +
extraction-helper pair — both of which this epic judged proportionate and shipped. Declining to
build this while capable of doing so cheaply would itself be the disproportionate choice here
(favoring caution over a clearly warranted, clearly bounded fix) — unlike, say,
`retrieval-fidelity`'s declined context-composition audit trail, where the *cost* side of the
trade was genuinely large (scanning arbitrary content flowing through arbitrary call sites) for
a comparatively narrow benefit. Here the cost is small and mechanical, and the benefit is
exactly what ss-9/this gap was filed to achieve: token discoverability that also actually
works when invoked.

**What is still deliberately NOT built, to keep this proportionate:**
- No local-builtin `ToolEndpoint` variant. Confirmed via `ToolRegistry::new()`'s body (reads
  in full this session) that both local built-ins (`TerminalTool`, `WebFetchTool`) are
  registered unconditionally on every call with no config/enablement gate — a local/built-in
  tool can never actually be "ingested but not currently live." Building an endpoint variant
  for a scenario that provably cannot occur would be speculative, so `ToolEndpoint` has exactly
  one variant (`Mcp`), not a local-builtin placeholder variant.
- No retry/backoff/timeout policy beyond what `call_tool` already has (a 30s timeout,
  unchanged). Adding a bespoke retry policy for the not-currently-live case specifically would
  be new lifecycle machinery this node's own finding says isn't needed — `call_tool`'s existing
  behavior (fail with a clear error string on timeout/connect failure) is reused as-is.
- No "verify the server is still enabled in config" pre-check before dispatching. The stored
  `McpServerConfig` is used exactly as captured at ingestion time — if the server config has
  since changed (different command, different id) the dispatch will simply fail with
  `call_tool`'s own connection-failure error, which is an accurate, honest diagnostic for "this
  stored connection info is stale," not a case needing special handling. This mirrors
  `tool-token-ingestion/decisions.md`'s own "no active cleanup, let staleness surface
  naturally" philosophy for stale tokens generally.
- No change to `SquireExploreTool`'s live-registry-primary read path for `"tool"`/
  `"tool_skill"` — unchanged from `tool-token-ingestion`, still correct for the reasons that
  node documented.
- No enumeration/deletion of stale tool tokens — unchanged scope boundary from
  `tool-token-ingestion`.

## `TokenDetail`/`ToolEndpoint` shape: one new optional field, one new small enum

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ToolEndpoint {
    /// Enough connection info to re-dispatch an MCP tool call purely from
    /// stored metadata, without the tool being present in this turn's live
    /// ToolRegistry. `server` is the exact McpServerConfig captured at
    /// ingestion time (see ingest_tool_registry); `remote_name` is the
    /// tool's own name as advertised by that server's tools/list response
    /// (may differ from the registry's locally-sanitized name).
    Mcp {
        server: McpServerConfig,
        remote_name: String,
    },
}

pub struct TokenDetail {
    pub short_desc: String,
    pub full_desc: Option<String>,
    #[serde(default)]
    pub endpoint: Option<ToolEndpoint>,
}
```

`#[serde(default)]` on `endpoint` and its `Option` wrapper together make this fully backward
compatible: every existing token (all non-tool token types; tool tokens ingested by an
already-deployed `tool-token-ingestion` before this node existed) simply has `endpoint: None`,
which `SquireInvokeTool`'s fallback treats identically to today's behavior (the original inert
diagnostic). `NewTokenSpec` (the write-side type) gets the same new optional field for
symmetry, so `upsert_token`'s existing signature doesn't need a second, endpoint-specific
method.

**Considered and rejected: putting the endpoint info inside `full_desc`'s existing JSON
envelope instead of a new typed field.** `full_desc` is a `String` (its content is a
convention, not a typed contract — different token types already put different JSON shapes in
it). Encoding endpoint info there would require `SquireInvokeTool` to parse `full_desc` as JSON
and pattern-match a schema no other reader depends on, entangling a purely internal dispatch
concern with a field whose primary purpose is being *shown* to the model (`full_desc` is
returned directly by `SquireTokenToDetailTool` for the model to read). Leaking connection
strings/credentials-adjacent config (`McpServerConfig.env`/`headers` can carry secrets) into a
field the model itself can read via `token_to_detail(detail_level="full")` would be a real
information-disclosure concern. A separate, non-model-visible field is both cleaner and safer.

## `SquireTokenToDetailTool` must NOT expose `endpoint` to the model

Confirmed by reading `SquireTokenToDetailTool::execute` in full: it currently returns
`short_desc`/`full_desc` (mapped from `TokenDetail`) directly as the tool's visible output to
the model. Since `McpServerConfig` can carry `env`/`headers` (potentially including secrets
like API keys for authenticated MCP servers), `endpoint` must never be serialized into
anything `SquireTokenToDetailTool` hands back to the model. This node's implementation must
double-check that tool's output construction explicitly does not include the new field (it
won't, by construction, if the tool continues to build its output from `short_desc`/`full_desc`
alone and never re-serializes the whole `TokenDetail` struct) — flagged here as an explicit
security-relevant constraint to verify during implementation and cover with a test, not just
asserted by the shape of the code.

## Ingestion call-site threading: a new optional parameter on `ingest_tool_registry`, populated only in `streaming_cmd.rs`'s MCP-discovery loop

`ingest_tool_registry(registry: &ToolRegistry, store: &dyn SquireStore)` reads only
`ToolDefinition`s, which don't carry origin. Rather than trying to recover origin from the
already-erased `ToolDefinition` (e.g. parsing the `mcp_{server_id}_{tool_id}` name convention,
which is lossy/sanitized and was explicitly never meant to be reversible), this node threads
the origin information from where it's still available: `streaming_cmd.rs`'s MCP-discovery
loop already holds `server.clone()` and `remote_tool_name` at the exact point it constructs
each `McpProxyTool`. A new sibling map, `endpoints: &HashMap<String, ToolEndpoint>` (keyed by
the same local registry name used everywhere else, i.e. `tool_token_id`'s input), is built
alongside `tool_registry` in that same loop and passed to `ingest_tool_registry` as a new
parameter; local built-ins are simply absent from this map (confirmed impossible to need one,
per Finding 2's local-builtin discussion above), so `ingest_tool_registry` passes `None` for
any definition whose name isn't in the map.

**Considered and rejected: adding an `origin`/`endpoint` field to `ToolDefinition` itself.**
`ToolDefinition` is the LLM-facing tool-schema type (`name`, `description`, `input_schema`) —
it's serialized directly into `ChatRequest.tools` and shown to the model. Adding
connection/credential-adjacent metadata to it would risk the same information-disclosure
concern flagged above for `full_desc`, for a type whose entire contract today is "what the
model sees." A separate side-channel map local to the ingestion call site avoids ever putting
this data anywhere near model-visible serialization.

## `SquireInvokeTool` dispatch branch: check `detail.endpoint` before falling back to the diagnostic

```rust
match self.store.token_detail(token_id).await {
    Some(detail) => match detail.endpoint {
        Some(ToolEndpoint::Mcp { server, remote_name }) => {
            match crate::mcp::call_tool(server, remote_name, params).await {
                Ok((output, is_error)) => ToolResult { call_id: call_id.to_string(), output, is_error },
                Err(e) => ToolResult {
                    call_id: call_id.to_string(),
                    output: format!("MCP tool call failed (dispatched from Squire storage, server not live in this turn's registry): {}", e),
                    is_error: true,
                },
            }
        }
        None => ToolResult { /* unchanged prior diagnostic */ },
    },
    None => ToolResult { /* unchanged "non-invocable token" */ },
}
```

This preserves the registry-primary/store-fallback shape `squire-storage/decisions.md`
established exactly — the live registry is still tried first and is still authoritative
whenever the tool is actually live this turn (avoiding a redundant reconnect for the common
case where the tool IS live) — and only reaches for the new endpoint-driven dispatch when the
registry lookup already failed. The one prior "recorded but no invocable endpoint bound yet"
diagnostic path is preserved unchanged for the `endpoint: None` case (local-builtin tokens by
construction, or MCP tokens ingested by a pre-this-node build that haven't been re-ingested
since — `upsert_token`'s existing merge semantics mean the very next turn's ingestion will
backfill `endpoint` for any MCP tool that's live again, so this is a self-healing transient
state, not a permanent regression for old rows).

## Verification methodology: unit tests against a fake/local MCP mechanism, not a real subprocess — consistent with `tool_token_ingestion_e2e.rs`'s own precedent

`tool-token-ingestion/decisions.md`'s own headless-harness section already established the
precedent this node follows: "this machine does have a real configured MCP server binary...
but spinning up a live stdio JSON-RPC handshake was judged unnecessary ceremony" for testing
ingestion mechanics. The same reasoning applies more strongly here: this node's new logic is
entirely about *locating and forwarding to* the right endpoint, not about the MCP wire protocol
itself (already tested elsewhere, if at all, by `mcp::mod`'s own scope — out of this node's
scope to re-verify). Testing "does `SquireInvokeTool` call `crate::mcp::call_tool` with the
right arguments when the registry misses but the store has an endpoint" does not require a real
server; it requires confirming the correct branch is taken and the correct arguments would be
passed. Given `crate::mcp::call_tool` is a free function (not behind a trait/mockable seam) and
this node's `env.md` scope explicitly avoids adding new trait surface where avoidable, this
node instead:
- Unit-tests the pure data plumbing exhaustively (`TokenDetail`/`NewTokenSpec` endpoint
  round-trips through both backends; `ingest_tool_registry` populating the map correctly for
  MCP-sourced defs and `None` for local builtins).
- Adds one integration-style test that points a stored `ToolEndpoint::Mcp` at a deliberately
  invalid/unreachable command (e.g. a nonexistent executable path) and confirms
  `SquireInvokeTool::execute` (a) takes the new dispatch branch (not the old inert-diagnostic
  branch) and (b) surfaces `call_tool`'s own real connection-failure error text, proving the
  new branch really calls the real `crate::mcp::call_tool` function end-to-end (exercising the
  real stdio-spawn-and-fail path) rather than a mocked stand-in — this is a real, if
  failure-mode, exercise of the actual dispatch call, matching this epic's general preference
  for real-code-path tests over mocks wherever practical, without requiring a real running MCP
  server subprocess (which would need a working server binary present in CI/dev environments
  generally, a dependency this node avoids introducing).
- Adds a security-relevant test confirming `SquireTokenToDetailTool::execute`'s model-visible
  output never contains `endpoint`/server config data, per the security constraint documented
  above.

No new headless example harness (`*_e2e.rs`) was added — the existing
`tool_token_ingestion_e2e.rs` already exercises `ingest_tool_registry`/`SquireInvokeTool`
end-to-end against a real `LanceDbSquireStore`; this node's dispatch-branch addition is better
covered by the targeted unit tests above (which can assert on the exact error text a real
failed connection attempt produces) than by extending that harness, which has no LLM/model in
the loop to meaningfully exercise the model-facing `invoke()` call path beyond what a direct
unit test already does. Confirmed via repo-wide frontend grep that this remains a pure backend
concern with no user-facing surface, matching every other backend-only node in this epic.
