# Prompt — ServerRefector

We are refactoring the server-side architecture to introduce two core abstractions:

## 1. ModelInstance

**Problem:** Currently, provider selection is scattered — the frontend passes `provider_name`, `model`, `thinking_level` as separate IPC args. The engine loop reconstructs these ad-hoc. Testing requires instantiating real providers.

**Solution:** Define a `ModelInstance` struct/value that bundles everything needed to talk to a specific model:
- `provider_name: String` — identifies which provider (e.g. "openai", "anthropic")
- `model: String` — the model ID (e.g. "gpt-4o", "claude-sonnet-4-20250514")
- `endpoint: Option<String>` — override base URL
- `api_key: Option<String>` — the key to use (stored or ephemeral)
- `options: ModelOptions` — structured bag of per-model options:
  - `thinking_level: Option<ThinkingLevel>` (none/low/medium/high)
  - `reasoning_effort: Option<String>` (for o-series models)
  - `max_tokens: Option<u32>`
  - `temperature: Option<f32>`
  - Any other model-specific parameters

**Key properties:**
- It is serializable (can be stored, passed across IPC, reconstructed)
- The engine loop uses `ModelInstance` to construct the actual provider SDK call — no ad-hoc field passing
- The chat UI selects/modifies a `ModelInstance` (could support multiple named instances)
- Tests just set `ModelInstance` directly → headless testing without UI

**From the provider side:** `LlmProvider::chat()` takes a `ModelInstance` (or a subset) instead of separate model/thinking_level args. The provider uses `ModelInstance.model` to pick the right API path and `ModelInstance.options` for parameters.

## 2. RuntimeContext

**Problem:** The engine loop (`send_message_impl`) directly grabs `State<'_, AppState>` and picks individual fields. This makes the engine untestable without Tauri, and tightly couples orchestration to the Tauri runtime.

**Solution:** Define a `RuntimeContext` struct that bundles all dependencies an engine needs:
- `workspace: Arc<dyn WorkspaceProvider>` — project path, file system access
- `session: Arc<dyn ConversationStore>` — chat history
- `squire_store: Arc<dyn SquireStore>` — Squire persistent memory
- `provider_registry: Arc<ProviderRegistry>` — available LLM providers
- `mcp_tools: Vec<McpTool>` — available MCP tools
- `config: RuntimeConfig` — key/value bag including:
  - `verbose_logging: bool`
  - `squire_prefetch: SquirePrefetchConfig`
  - Test-specific flags: `test_config["log_timing"]`, etc.
- `signal: CancellationToken` — for cancellation

**Key properties:**
- The engine trait (`Engine::run(context: RuntimeContext)`) takes a single `RuntimeContext` — no AppState dependency
- In the real app: `setup_app_impl()` builds `RuntimeContext` from `AppState` and passes it to the engine
- In tests: tests build `RuntimeContext` with `InMemorySquireStore`, `RecordingStore`, mock providers → full headless integration testing
- `RuntimeConfig` provides an extensible key/value map so engines and providers can query custom config without changing the struct

## Implementation Order

1. Define `ModelInstance` + `ModelOptions` in `provider-core` (or a new `model-instance` crate)
2. Refactor `LlmProvider::chat()` to accept `&ModelInstance`
3. Update `OpenAIProvider` and `AnthropicProvider` to read from `ModelInstance`
4. Define `RuntimeContext` in a new `engine-core` crate (or in `provider-core`)
5. Define `Engine` trait with `run(context: RuntimeContext)`
6. Extract the engine loop from `send_message_impl` into a `SquireEngine` implementing `Engine`
7. Update `send_message_impl` to build `RuntimeContext` and call the engine
8. Write headless integration tests using mock `RuntimeContext`

