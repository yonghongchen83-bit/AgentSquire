# Decisions

## Trait evolution: `ContextManagerAdapter::finalize_turn` now supports retry

`finalize_turn` signature changed from `(session_id, content, thinking, store) -> Result<(), String>` to `(session_id, content, thinking, messages: &mut Vec<ChatMessage>, store) -> Result<TurnOutcome, String>`, where `TurnOutcome` is `Done | Retry | Failed { reason, failed_content }`. `LegacyContextAdapter` always returns `Done` (trivial case, `messages` unused). `send_message_impl`'s `FinishReason::Stop | Length` branch now matches on the outcome: `Done` ends the turn as before, `Retry` does `continue` (loops back into `provider.chat()` with the adapter's appended continuation message), `Failed` emits `stream-error` with the reason plus an `output:append` diagnostic carrying the failed content.

This was necessary because Q6 ("keep internal validation retries up to `max_retries`... on exhaustion, surface a compliance-failure error") requires the model to be re-prompted on a rejected response, which the three-seam design from `../adapter-core/decisions.md` didn't originally provide a path for. `finalize_turn` was the only seam positioned to make this call (it's where the model's terminal output is inspected), so it was widened rather than adding a fourth seam.

## `SquireStore` trait — the contract `squire-storage` implements

Defined in `src-tauri/src/agent/squire.rs`. Covers exactly what the adapter and built-in tools need: token existence/upsert, relationships, preserve-list get/set, `explore_memory` (type + query + num_hops + max_results -> scored summaries), `token_detail`, and a per-session turn counter (`current_turn`/`increment_turn`). `num_hops` is accepted but `InMemorySquireStore` (the in-process stand-in shipped in this node) does no real graph traversal — it's a flat filter over an in-memory map. `squire-storage` implements the same trait against LanceDB's structured/raw partitions and triplet store (Q4); no other code needs to change when that lands — `AppState.squire_store: Arc<dyn SquireStore>` just gets constructed with the real impl in `setup_cmd.rs` instead of `InMemorySquireStore::new()`.

`InMemorySquireStore` lives in `AppState` (constructed once at app startup), not reconstructed per-turn like the local `ToolRegistry` is — so it does persist across turns within one app session, just not across restarts. This was a deliberate choice over a per-call throwaway store, since preserve-lists and accumulated tokens need to survive at least until squire-storage lands.

## Tool exposure (Q5): two registries, not one

`send_message_impl` now builds two things after MCP discovery: the full `tool_registry` (unchanged — all local + MCP tools) and, for Squire-mode sessions only, a second `dispatch_registry` containing exactly `explore`, `token_to_detail`, `invoke`. The tool-call execution loop (`dispatch_registry.get(&tc.name)`) uses the mode-appropriate one; `SquireContextAdapter::build_turn_input` independently returns only the 3 built-in `ToolDefinition`s regardless of what's passed as `base_tools`, so the model's `ChatRequest.tools` never includes anything else either. Both the schema the model sees and the names orchestration will actually execute are locked to the 3 built-ins — a hallucinated call to e.g. `run_terminal` gets "Unknown tool" from `dispatch_registry`, not silent execution against the full registry.

`SquireInvokeTool` is the only place that holds a reference to the full `Arc<ToolRegistry>` — it proxies `invoke(token_id, params)` to `tool_registry.get(token_id)`, which is today's stand-in for a real LanceDB tool-token store: **the invocable token space is exactly the set of names in the real tool registry** (MCP tool names included). When `squire-storage` lands with real tool-token ingestion, `invoke_tool`'s lookup should move from `tool_registry.get(token_id)` to `store.token_detail(token_id)` resolving to an MCP endpoint. This is the one place `squire-storage` needs to change in `squire.rs` when it lands.

`SquireInvokeTool::danger()` unconditionally returns `Destructive` — every `invoke()` call requires user approval, since the tool's static `danger()` has no access to `params.token_id` to know what's actually being proxied. This is conservative (extra approval prompts for read-only proxied calls) rather than risky; revisit once token metadata can carry a per-token danger classification.

## Protocol adaptation: system prompt is a Rust constant, not `include_str!`

Q1 already established "system-prompt superset is runtime truth for v1; spec doc must be synced" (protocol-doc-sync's job). `SQUIRE_SYSTEM_PROMPT` in `squire.rs` is a condensed rewrite of `../context_squire_system_prompt_v2.md`, not an `include_str!` of it — the two live in different places (`.AiControl/` is a planning workspace, not a build input) and needed adaptation anyway: our transport is provider-native tool-calling (`ChatRequest.tools` + `ToolCall`), so the prompt doesn't need to explain a `system_prompt` field in the request JSON the way the spec's illustrative wire format does — the `ChatMessage::System` role already carries it. The per-turn JSON built in `build_turn_input` only contains `user_request`/`prefetched_tokens`/`preserved_tokens`, not a duplicated `system_prompt` field.

`build_turn_input` does **not** replay `session.messages` history the way `LegacyContextAdapter` does — it takes only the latest user message as `user_request`. This is the entire point of Squire mode (curated context, not growing history), so replaying history here would defeat the feature.

## Validation gates implemented (Q6, spec §8.3)

`validate_squire_response` checks: `ask_user`+`content` mutual exclusion, empty close response, undisplayable `§!TokenID` references (checked against the store plus the response's own `new_tokens`), and unclosed `§^` spans. The fifth rule in the spec table — `invoke()` called on a non-invocable token — is a call-time check, not a close-time one, so it lives in `SquireInvokeTool::execute` instead (returns `is_error: true` with `"non-invocable token {id}"`, matching the spec's rejection-reason string).

Sigil parsing (`extract_inline_refs`, `extract_spans`, `strip_span_markers`) is hand-rolled string splitting on `'§'`/`"§^"` rather than regex (no regex dependency in this crate) — see the unit tests in `squire.rs` for the exact boundary behavior (whitespace/next-`§` termination, no nesting).

## Known gaps deliberately left open (not silently absorbed into this node's scope)

- **sa-4**: raw model output (with sigils still in it) is streamed live to the frontend via `stream-chunk` *before* `finalize_turn` ever parses/expands it. Squire's "protocol artefacts are never visible to the user" guarantee is violated for the live stream today (final persisted history is correct — `finalize_turn` stores the expanded clean-prose version). Fixing this means not emitting `stream-chunk` at all in Squire mode and instead emitting the expanded content once at turn close — an orchestration streaming change, not an adapter change. Flagged in `todo.json`, not claimed by any node yet.
- **sa-5**: the `ask_user` response-field loop (spec §9.3, the "complex clarifications" path from Q2) isn't wired — `finalize_turn` returns `Err` if the model populates `ask_user`, since resubmitting with an accumulated `user_request` needs a UI round-trip that doesn't exist. `TOOL_AskUser` via `invoke()` (Q2's other, "lightweight" path) works today through the ordinary tool-call loop and is unaffected.

Neither gap blocks `squire-storage` (pure storage-layer work) or `rejection-ux` (consumes `TurnOutcome::Failed`, which is fully wired) from starting.
