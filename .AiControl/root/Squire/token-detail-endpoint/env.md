# Env

- Parent node: root/Squire
- Node path: root/Squire/token-detail-endpoint
- Objective: figure out whether/how to close the gap `tool-token-ingestion/env.md` explicitly
  scoped out and `squire-storage/decisions.md` originally described as a second, separate
  "full cutover" piece — an endpoint-carrying `TokenDetail` extension so `SquireInvokeTool`
  could actually dispatch a tool call using only information persisted in `SquireStore`,
  rather than requiring the tool to already be a live `ToolRegistry` entry for the same turn.
- Scope: read the full prior context (see Durable facts below), assess proportionality, and
  implement whichever of the following two outcomes that assessment supports:
  (a) real store-driven dispatch (extend `TokenDetail`/ingestion with enough metadata to
  reconstruct an `McpServerConfig` and call `crate::mcp::call_tool` directly from
  `SquireInvokeTool`'s fallback path, bypassing the live registry entirely), or
  (b) a smaller, well-scoped diagnostic improvement (capture enough metadata during ingestion
  to name the specific originating MCP server in the fallback's error message, without
  building new connection/dispatch logic). Whichever is chosen must be documented thoroughly
  in decisions.md with the reasoning, per this epic's established practice for judgment calls.
- Non-goal (if outcome (b) is chosen): building any new MCP session/connection lifecycle
  management, a "reconnect to an arbitrary server on demand" feature beyond what
  `crate::mcp::call_tool` already does per-call, or any UI surface (this remains a backend-only
  gap — no user-facing surface change is expected either way, matching every other pure-backend
  node in this epic since `tool-token-ingestion`).
- Non-goal (regardless of outcome): the `"memory"`-alias/`system_referential` gap
  `user-input-chunking` flagged; the nested-`§!`-citation residual `hit-count-fidelity`
  flagged; any frontend/UI work.
- Depends on: `tool-token-ingestion` (`ingest_tool_registry`, `tool_token_id`, `tool_full_desc`
  — the exact metadata currently captured per tool token), `squire-storage`
  (`SquireInvokeTool`'s registry-primary/store-fallback dispatch shape, `TokenDetail`'s
  current two-field shape, the original "full cutover" framing this node re-examines), the
  real MCP client code (`src-tauri/src/mcp/mod.rs`: `discover_tools`/`call_tool`/
  `McpServerConfig`), and `streaming_cmd.rs`'s per-turn `ToolRegistry`/MCP-discovery
  construction.
- Status: active, 2026-07-03.

## Durable facts (read this session)

- `TokenDetail` (`src-tauri/src/agent/squire.rs`) currently has exactly two fields:
  `short_desc: String`, `full_desc: Option<String>`. No endpoint/connection-carrying field
  exists on it or on `NewTokenSpec` (the write-side counterpart) today.
- `ingest_tool_registry`/`tool_token_id`/`tool_full_desc` (`tool-token-ingestion`) write one
  `tool`-typed token per `ToolRegistry::definitions()` entry: `id = registry name verbatim`,
  `short_desc = description`, `full_desc = {"name","description","input_schema"}` JSON. Origin
  (local built-in vs. MCP, and if MCP, which server) is **not** captured anywhere in the
  written token — `ToolDefinition` itself (`name`, `description`, `input_schema`) erases that
  distinction once a tool is registered into `ToolRegistry`, per `tool-token-ingestion/
  env.md`'s own explicit note. The *only* place origin is still recoverable post-hoc is by
  string-parsing a token id matching the `mcp_{server_id}_{tool_id}` naming convention applied
  once at registration time in `streaming_cmd.rs` (sanitized alphanumeric-only components, not
  reversible to the original server id/tool name in general, only to the sanitized form).
- `SquireInvokeTool::execute` (`src-tauri/src/agent/squire.rs`): tries
  `self.tool_registry.get(token_id)` first (primary), falls back to
  `self.store.token_detail(token_id).await` (returns a generic "recorded... but has no
  invocable endpoint bound yet" diagnostic on a hit, "non-invocable token" on a miss). This
  fallback path is the exact site this node's gap concerns.
- `SquireInvokeTool` is constructed per-turn in `streaming_cmd.rs` with exactly two fields:
  `tool_registry: Arc<ToolRegistry>`, `store: Arc<dyn SquireStore>`. It does **not** currently
  hold a reference to `AppState.config` (where `AppConfig.mcp_servers: Vec<McpServerConfig>`
  lives) or to any MCP client/connection type.
- `crate::mcp::call_tool(server: McpServerConfig, name: String, arguments: Value)`
  (`src-tauri/src/mcp/mod.rs`) is the **only** MCP dispatch primitive in the codebase, and —
  critically — it is already a one-off, stateless call: it spawns a fresh `StdioMcpClient`,
  connects, initializes, calls the one tool, and (via `StdioMcpClient`'s `Drop` impl) kills the
  child process, every single time it's invoked. There is no persistent MCP session/connection
  object anywhere in this codebase to reuse or extend — `McpProxyTool::execute` (the live
  registry's own MCP dispatch path) calls `crate::mcp::call_tool` exactly this way, per call,
  today. This means "reconnecting to an MCP server on demand" is not new infrastructure this
  epic would have to invent — it is the exact mechanism every live MCP tool call already uses;
  the only genuinely new piece would be *locating* the right `McpServerConfig` to pass, when
  the tool isn't in the live `ToolRegistry` (and therefore not in `enabled_mcp_servers` this
  turn) to begin with.
- `McpServerConfig` (`src-tauri/src/state/config.rs`) fields: `id`, `name`, `transport`,
  `command`, `args: Vec<String>`, `url: Option<String>`, `enabled: bool`,
  `env: HashMap<String,String>`, `headers: HashMap<String,String>` — i.e. everything needed to
  reconnect to a specific server is a plain, `Serialize`/`Deserialize`/`Clone` struct.
  `AppConfig.mcp_servers: Vec<McpServerConfig>` is read today only via
  `state.config.read()...cfg.mcp_servers` in `streaming_cmd.rs` — `SquireInvokeTool` has no
  access to `AppState` at all currently (only `ToolRegistry`/`SquireStore`).
- `ToolRegistry::new()` unconditionally registers exactly two local built-ins
  (`TerminalTool`, `WebFetchTool`) on every call, with no config/enablement gate — confirmed by
  reading its body. A local/built-in tool can never be "ingested but not currently live": every
  turn's freshly-constructed registry always contains both, by construction. The
  not-currently-live scenario this node's gap describes is therefore only possible for
  MCP-sourced tools (a server since disabled in config, or a server whose `discover_tools` call
  failed/timed out this particular turn — both already-logged-as-warning cases in
  `streaming_cmd.rs`, not new scenarios this node introduces).
