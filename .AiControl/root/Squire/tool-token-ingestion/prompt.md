# Prompt

Wire `squire-storage/todo.json`'s **ss-9**, deliberately deferred as out of scope by
`squire-storage` itself and reconfirmed as genuinely separate scope by `retrieval-fidelity`:

> FOLLOW-UP (not in this node's scope, not claimed): real tool-token ingestion - a write path
> that turns MCP/local tool discovery into persisted, invocable SquireStore token rows (with
> an endpoint-carrying extension to TokenDetail), so SquireInvokeTool's store-fallback path can
> actually resolve and invoke something instead of only returning a diagnostic error. Needed
> before Q5's "Squire as sole gateway" vision is fully realized for tools discovered via
> explore(resource_type='tool_skill') that aren't already local/MCP registry entries.

Concretely: `explore(resource_type="tool_skill")` and `SquireInvokeTool`'s lookup path both
expect discoverable tools (MCP tools, local/built-in tools) to exist as rows in `SquireStore`
(structured tokens with type `tool_skill`/`tool`), so the model can discover and invoke them
through the Q5 strict tool boundary (explore/token_to_detail/invoke only). No write path
currently exists that takes the app's actual tool registry (MCP-discovered tools + local/
built-in tools) and populates the store with corresponding tokens — `squire-storage`'s session
left `SquireInvokeTool`'s lookup as an additive fallback (tries the live `ToolRegistry` first,
falls back to `store.token_detail()`) specifically because of this ingestion gap.

Deliverables:
- Design and implement the ingestion write path: when the app's tool registry is built/
  refreshed (the existing per-turn `ToolRegistry` construction in `streaming_cmd.rs` is the
  one real trigger point — confirm this by reading the actual code, don't assume a
  connect/disconnect event system exists), write/update corresponding `tool_skill`-typed
  tokens into the active `SquireStore`.
- The ingestion logic itself must be backend-agnostic — call `SquireStore::upsert_token` (or
  add a new trait method only if truly necessary; check the existing trait shape first) so it
  works against both `InMemorySquireStore` and `LanceDbSquireStore` without duplicating logic
  per backend.
- Decide and document in decisions.md:
  - Token ID scheme: must be stable/deterministic so re-ingestion updates rather than
    duplicates existing rows.
  - Content/summary shape: what goes in `short_desc` vs. `full_desc`, matching how
    `explore()`'s existing token shape and the spec's tool full-description format (§3.1:
    "Standard MCP tool schema — name, description, input schema") expect it.
  - Tool removal/staleness handling: stale token cleanup vs. leave as informational history —
    your judgment, documented with rationale.
- Add real unit tests for the ingestion logic, in the same style as the existing test suites in
  `squire.rs`/`squire_lancedb.rs` (see `retrieval-fidelity`'s tests for the most recent
  precedent of testing shared backend-agnostic logic against both stores).
- Run `cargo build` + `cargo test --lib` from `src-tauri/` (expect clean build, 158/158 passing
  baseline — `protoc` may be needed for a cold build, see `handoff.md`).
- If practical, verify manually/e2e that a real tool now shows up via
  `explore(resource_type="tool_skill")` in an actual Squire-mode session, using the configured
  test provider and the existing WDIO+tauri-driver e2e setup (see `ask-user-loop`'s session for
  the precedent methodology) — but don't block on this if disproportionately expensive; unit
  tests plus a clear code-path trace are an acceptable fallback (`stream-sigil-fix`'s
  precedent), just note explicitly which was done.

Reference: `../squire-storage/todo.json` ss-9; `../squire-storage/decisions.md` (the
`SquireStore` trait contract, the `SquireInvokeTool` additive-fallback design and its exact
pointer to what full cutover would need); `../squire-adapter/decisions.md` (Q5 tool boundary:
explore/token_to_detail/invoke, the two-registry `dispatch_registry`/full-`tool_registry`
design); `../context_squire_spec_v2.md` §3.1/§3.2 (token record fields, tool full-description
format), §6.1 (`explore()`'s `tool_skill` flat-array return shape); `src-tauri/src/agent/squire.rs`
(`SquireStore` trait, `SquireExploreTool`/`SquireInvokeTool`, `StoredToken`/`TokenSummary`
shapes); `src-tauri/src/storage/squire_lancedb.rs` (`LanceDbSquireStore`, `squire_tokens`
schema); `src-tauri/src/agent/mod.rs` (`ToolRegistry`, `McpProxyTool`); `src-tauri/src/mcp/mod.rs`
(MCP tool discovery); `src-tauri/src/commands/streaming_cmd.rs` (where `ToolRegistry` is
actually built each turn — the real trigger-point candidate).

Out of scope (do NOT fix here — separately tracked, deliberately deferred):
- User-input auto-chunking (`USR_TN_NNN` tokens)
- Raw-partition audit-log storage
- `retrieval-fidelity/todo.json` rf-13 (fuller hit-count-event fidelity)
- Extending `TokenDetail`/`SquireInvokeTool` with a real MCP-endpoint-carrying invocation path
  (squire-storage/decisions.md's "full cutover" second half — this node only needs tokens to
  exist and be discoverable/describable, not to change how `invoke()` dispatches)
- Any frontend UI work (this is a backend-only write path with no new user-facing surface)
