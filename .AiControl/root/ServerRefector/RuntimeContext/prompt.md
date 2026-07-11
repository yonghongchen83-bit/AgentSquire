# Prompt — RuntimeContext

Design and implement `RuntimeContext` and the `Engine` trait to decouple orchestration from Tauri's AppState.

## Requirements

1. Define `RuntimeContext` struct:
   - `workspace: Option<Arc<dyn WorkspaceProvider>>` — project path, file system
   - `session: Arc<dyn ConversationStore>` — chat history store
   - `squire_store: Arc<dyn SquireStore>` — Squire persistent memory
   - `provider_registry: Arc<ProviderRegistry>` — LLM provider access
   - `mcp_tools: Vec<McpToolSpec>` — available MCP tools
   - `config: RuntimeConfig` — typed + extensible config bag
   - `cancellation: CancellationToken` — for stream cancellation
   - `app_handle: Option<AppHandle>` — optional, for subagent MCP tool (legacy bridge)

2. Define `RuntimeConfig` struct:
   - `verbose_logging: bool`
   - `squire_prefetch: SquirePrefetchConfig`
   - `disabled_tools: Vec<String>`
   - `test_config: HashMap<String, String>` — extensible key/value for test flags (e.g., `log_timing`, `fail_on_error`)
   - `extra: HashMap<String, String>` — general extensibility

3. Define `Engine` trait:
   ```rust
   #[async_trait]
   pub trait Engine: Send + Sync {
       async fn run(
           self: Box<Self>,
           ctx: RuntimeContext,
           request: ChatRequest,
       ) -> Result<()>;
   }
   ```

4. Extract `SquireEngine` from `send_message_impl`:
   - Move the main loop (Phase 1 → tool execution → Phase 2) into `SquireEngine::run()`
   - `SquireEngine` implements `Engine` trait
   - It uses `ContextManagerAdapter` internally (already abstracted)

5. Update `send_message_impl`:
   - Build `RuntimeContext` from `AppState` fields
   - Call `SquireEngine::run(ctx, request).await`
   - Keep IPC-level concerns (event emission) in the command handler or pass an event emitter through context

6. Write headless integration tests:
   - Build `RuntimeContext` with `InMemorySquireStore`, `RecordingStore`, mock providers
   - Call `SquireEngine::run()` directly
   - Verify streaming events, tool calls, Phase 2 behavior without Tauri

## Design Constraints

- **Engine does NOT import tauri.** No `AppHandle`, no `State<>`, no Tauri types in engine code.
- **Event emission** (to frontend) goes through a trait or callback in RuntimeContext, not direct Tauri event APIs.
- **Subagent tool** currently holds `AppHandle` — this is legacy. For now, keep `AppHandle` as optional in RuntimeContext with a TODO to remove.
