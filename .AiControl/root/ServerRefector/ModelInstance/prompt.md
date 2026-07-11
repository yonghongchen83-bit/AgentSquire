# Prompt — ModelInstance

Design and implement `ModelInstance` in `provider-core` crate.

## Requirements

1. Define `ModelInstance` struct:
   - `provider_name: String` — identifies the provider entry
   - `model: String` — the model ID string (e.g. "gpt-4o", "claude-sonnet-4-20250514")
   - `endpoint: Option<String>` — optional override base URL
   - `api_key: Option<String>` — optional API key (for ephemeral/per-instance keys)
   - `options: ModelOptions` — per-model options

2. Define `ModelOptions` struct:
   - `thinking_level: Option<ThinkingLevel>` — None/None/Low/Medium/High
   - `reasoning_effort: Option<String>` — for o-series models
   - `max_tokens: Option<u32>`
   - `temperature: Option<f32>`
   - Extensible via `extra: HashMap<String, String>` for provider-specific params

3. Both structs must be `Serialize + Deserialize + Clone + Debug + PartialEq`.

4. Update `LlmProvider` trait:
   - Change `chat()` signature to accept `&ModelInstance` (or at minimum `&ModelOptions` alongside model name)
   - Keep backward compatibility if possible

5. Update `OpenAIProvider` and `AnthropicProvider`:
   - Read `model`, `thinking_level`/`reasoning_effort`, `max_tokens`, `temperature` from `ModelInstance`
   - Construct API requests from the unified struct

6. Update `ProviderRegistry`:
   - Add a method to build a `ModelInstance` from provider name + model name + optional overrides
   - E.g., `registry.resolve_model_instance(provider_name, model, options) -> Result<ModelInstance>`

7. Update IPC layer:
   - `send_message` command accepts `ModelInstance` fields (or a reference to a named instance)
   - Frontend sends structured model selection instead of separate fields
