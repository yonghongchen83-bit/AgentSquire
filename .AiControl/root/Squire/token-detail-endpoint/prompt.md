# Prompt

Close (or deliberately, documentedly scope down) the endpoint-carrying `TokenDetail`
extension `tool-token-ingestion/env.md` flagged as an explicit non-goal, originally described
in `squire-storage/decisions.md` as a second, separate "full cutover" piece beyond ingestion
itself:

> If a future node wants full production parity here, it needs: (a) a discovery-to-ingestion
> write path (likely in `streaming_cmd.rs` alongside MCP tool discovery) [DONE —
> `tool-token-ingestion`], and (b) an endpoint-carrying extension to the token schema, neither
> of which existed before this node and neither of which this node's stated scope (storage
> layer) covers.

**The gap, precisely:** `tool-token-ingestion` made tools discoverable/describable via
`SquireStore` (real rows for `explore(resource_type="tool_skill")` and `token_to_detail`), but
`SquireInvokeTool`'s dispatch is unchanged — registry-primary, store-fallback-to-diagnostic. A
tool that's ingested-but-not-currently-live (e.g. from an MCP server connected in a previous
turn but not this one) is discoverable and describable but not invocable — `invoke()`
dead-ends with "recorded but no invocable endpoint bound yet."

Deliverables:

- Read `../tool-token-ingestion/decisions.md` and `env.md` in full — exactly what was/wasn't
  built and why the endpoint extension was scoped out at the time, including the specific
  reasoning about `SquireInvokeTool`'s fallback being "correct but currently inert" for
  genuinely-gone tools.
- Read `../squire-storage/decisions.md`'s "`SquireInvokeTool` redirect" section in full — the
  original "full cutover" framing and its own reasoning for why a hard cutover wasn't
  implementable at the time.
- Read the actual code in full: `src-tauri/src/agent/squire.rs` (`SquireInvokeTool`,
  `SquireStore` trait, `TokenDetail`/`NewTokenSpec`/`StoredToken`, `ingest_tool_registry`/
  `tool_token_id`/`tool_full_desc`) to see exactly what metadata is captured per tool token
  today and whether it's enough to reconnect/dispatch or whether new fields are needed. Also
  read `src-tauri/src/commands/streaming_cmd.rs`'s per-turn `ToolRegistry` construction (local
  built-ins + MCP `tools/list` discovery) and `src-tauri/src/mcp/mod.rs` (`discover_tools`/
  `call_tool`/`McpServerConfig`/`StdioMcpClient`) to understand what it would take to
  construct a one-off connection to a specific MCP server purely from stored metadata.
- Verify baseline first: from `src-tauri/`, `cargo build && cargo test --lib` (expect
  clean/210 passing; `protoc` may be needed for a cold build — see `../handoff.md`).
- Before implementing anything, make and document a clear-eyed proportionality assessment in
  decisions.md: is fully closing this gap (real dispatch to a non-live tool) proportionate,
  given this epic's established pattern of choosing lighter-weight, well-documented
  approximations over disproportionate new infrastructure (`tool-token-ingestion`'s staleness
  handling, `rejection-ux`'s Q7 handling, etc.)? Specifically consider:
  - Whether a **local/built-in** tool can ever actually be "not currently live" (check
    `ToolRegistry::new()`'s registration logic) — if not, this whole gap may only be
    meaningful for MCP-sourced tools.
  - For **MCP** tools: what real dispatch would require — locating the originating server's
    connection config from stored metadata, and forwarding the call. Read `crate::mcp::
    call_tool`'s actual implementation carefully before assuming this requires new
    session/connection lifecycle infrastructure — confirm whether it already is (or isn't) a
    stateless, one-off-per-call operation, since that materially changes how large this
    feature actually is.
  - It is entirely acceptable to conclude "closing this fully is disproportionate; here is a
    smaller, well-scoped improvement instead" (e.g. a clearer diagnostic naming the specific
    offline MCP server) — or to conclude full dispatch is in fact tractable and proportionate,
    if the investigation supports it. Either conclusion must be documented thoroughly with the
    reasoning that led there.
- Implement whichever scope the assessment supports, plus real unit tests (both
  `InMemorySquireStore` and `LanceDbSquireStore` if the `SquireStore` trait or `TokenDetail`
  shape changes; parity is this epic's established convention).
- Verify via unit tests plus a headless example harness if genuinely useful (following the
  `tool_token_ingestion_e2e.rs` pattern) — backend-only, no frontend surface expected; confirm
  via a repo-wide frontend grep per this epic's established practice.
- Update this node's `state.md`/`decisions.md`/`todo.json` as you go.
- Update `../state.md`'s "Child Nodes" list and `../handoff.md` to reflect this session's
  work, current build/test status, and remaining backlog (this item now resolved, whether via
  full implementation or a documented deliberate scope-down).

Out of scope (do NOT change here):
- The `"memory"`-alias/`system_referential` gap `../user-input-chunking` flagged
- The nested-`§!`-citation residual `../hit-count-fidelity` flagged
- Any frontend/UI work — this remains a backend-only concern regardless of outcome
- Building a general-purpose persistent MCP connection/session manager beyond what
  `crate::mcp::call_tool`'s existing one-off-per-call model already provides, unless the
  proportionality assessment concludes this is specifically and narrowly needed
