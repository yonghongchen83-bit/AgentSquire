# Env

- Parent node: root/Squire
- Node path: root/Squire/tool-token-ingestion
- Objective: Close `squire-storage/todo.json`'s ss-9 — implement a real write path that
  ingests the app's actual tool registry (local/built-in tools + MCP-discovered tools) into
  `SquireStore` as `tool_skill`-typed (`token_type = "tool"`) token rows, so
  `explore(resource_type="tool_skill")` can surface real tools from the store side (not just
  the always-live `ToolRegistry` read path `SquireExploreTool` already has), and so
  `SquireInvokeTool`'s existing store-fallback lookup (`squire-storage/decisions.md`'s
  additive-fallback design) has something real to find instead of dead-ending on an inert,
  never-populated store.
- Scope: a new backend-agnostic ingestion function (`agent::squire::ingest_tool_registry` or
  similar) that takes `&ToolRegistry` + `Arc<dyn SquireStore>` and upserts one token per tool
  via the existing `SquireStore::upsert_token` trait method (no new trait method, no
  per-backend duplication); a call site wiring it into `streaming_cmd.rs`'s existing per-turn
  `tool_registry` construction (the one real trigger point that already exists — see Durable
  facts below); unit tests for the ingestion function's token-shape/ID-scheme/
  update-not-duplicate behavior.
- Non-goal: user-input auto-chunking (`USR_TN_NNN` tokens), raw-partition audit-log storage,
  `retrieval-fidelity/todo.json` rf-13 (fuller hit-count-event fidelity), giving `invoke()` a
  real MCP-endpoint-carrying `TokenDetail` extension (squire-storage/decisions.md flagged this
  as a second, separate piece of "full cutover" — this node only needs ingested tokens to be
  discoverable via `explore()`/`token_to_detail()`; `SquireInvokeTool`'s registry-primary/
  store-fallback dispatch shape is unchanged, still correct, and still out of this node's scope
  to alter). No frontend UI work — ingestion is a pure backend write path with no user-facing
  surface of its own (the effect is visible only through `explore()`'s existing results, already
  wired to the frontend via the ordinary tool-call loop).
- Depends on: squire-adapter (`SquireStore` trait, `SquireExploreTool`/`SquireInvokeTool`,
  the Q5 strict tool boundary), squire-storage (`LanceDbSquireStore`, the `SquireInvokeTool`
  fallback design and its explicit ss-9 pointer), retrieval-fidelity (`accumulated_hits`/
  `effective_priority` — ingestion's repeated per-turn upserts interact with these, see
  Decisions), the real `ToolRegistry`/MCP-discovery code in `agent/mod.rs`,
  `commands/streaming_cmd.rs`, `mcp/mod.rs`.
- Status: active, 2026-07-03.

## Durable facts (read this session, not previously written down anywhere in the epic)

- `ToolRegistry` (`src-tauri/src/agent/mod.rs`) is **rebuilt from scratch on every single
  turn**, inside the `tokio::spawn`'d task in `streaming_cmd.rs`'s `send_message_impl` — not
  once at app startup, not cached across turns. Each turn: `ToolRegistry::new()` (registers
  `TerminalTool`/`WebFetchTool`, the only two local built-ins currently registered — see
  `ToolRegistry::new`'s body, several others are commented out), then a loop over
  `state.config`'s enabled `McpServerConfig`s calling `crate::mcp::discover_tools(server)`
  (a real stdio JSON-RPC handshake + `tools/list` call per enabled server, ~seconds of I/O),
  registering each result as an `McpProxyTool` under a sanitized `mcp_{server_id}_{tool_id}`
  name (alphanumeric-only, collision-suffixed). This is the one real, already-existing trigger
  point ss-9 needs — there is no separate "MCP server connect/disconnect" event system to hook
  into; discovery already happens fresh every turn as an unavoidable side effect of building
  `ChatRequest.tools` for that turn, for **both** Legacy and Squire mode sessions (this
  per-turn rebuild is not Squire-specific).
- `ToolRegistry::definitions()` returns `Vec<ToolDefinition { name, description, input_schema }>`
  — exactly the three fields needed to build a `tool_skill` token per the spec's §3.1 "Tool:
  Standard MCP tool schema (name, description, input schema)" full-description format. MCP
  origin vs. local/built-in is not distinguishable from `ToolDefinition` alone (`McpProxyTool`
  vs. e.g. `TerminalTool` erase this distinction once registered) — not needed for this node's
  scope (token shape doesn't need to record origin; `token_id = name` already disambiguates
  MCP tools via their `mcp_{server_id}_{tool_id}` naming scheme applied at registration time).
- `SquireExploreTool::execute`'s existing `"tool"`/`"tool_skill"` branch already reads live
  from `self.tool_registry.definitions()` directly (bypassing the store entirely) — this was
  `squire-adapter`'s original, still-correct design for guaranteeing the model always sees
  *currently real, invocable* tools without a staleness window. This node's ingestion does
  **not** change that read path (still correct, still the primary source for `"tool"`/
  `"tool_skill"` explore results) — it only makes the *store side* stop being permanently
  empty, so `SquireInvokeTool`'s fallback and any future `explore()` refinement that wants to
  consult the store (e.g. combining live registry tools with store-recorded historical/
  MCP-server-currently-offline tools) has real rows to find. See decisions.md for why this
  node does not also rewire `SquireExploreTool`'s primary read path.
- `SquireStore::upsert_token` is already exactly the right shape for this: `(NewTokenSpec {
  id, token_type, short_desc, full_desc }, creation_turn)`. No new trait method was needed —
  confirmed by reading the full trait in `agent/squire.rs` before starting. Calling it
  repeatedly with the same `id` for a tool whose registration is unchanged (the common case,
  every ordinary turn) increments `accumulated_hits` every single call, per both stores'
  existing "regardless" upsert semantics (`squire-storage/decisions.md`'s literal spec §9.4
  step 5 quote) — see decisions.md for why this node accepts that side effect rather than
  adding a new no-op-if-unchanged code path to the shared trait contract.
- `creation_turn` for a tool token has no natural "session turn" to attach to — tool discovery
  happens once per turn but is not scoped to any one session's turn counter (`SquireStore`'s
  `current_turn`/`increment_turn` are keyed by `SessionId`, and tool discovery in
  `streaming_cmd.rs` happens before the adapter/session-specific logic, shared across both
  context modes). This node passes `0` as `creation_turn` for tool tokens — see decisions.md.
