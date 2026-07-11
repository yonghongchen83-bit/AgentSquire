# Decisions — ModelInstance

## D1. ModelInstance lives in provider-core

`ModelInstance` is a core concept that both providers and the engine need. It goes in `provider-core` alongside `LlmProvider` trait to avoid circular dependencies.

## D2. ModelOptions is a struct with Option<T> fields

Strongly typed over `serde_json::Value`. Each supported option has a named field with `Option<T>`. An `extra: HashMap<String, String>` field catches provider-specific options.

## D3. API key in ModelInstance is optional

In the normal flow, the key comes from `ProviderConfig` (stored config). `ModelInstance.api_key` allows ephemeral override (e.g., temporary key from UI). If `None`, the provider falls back to its stored key.

## D4. ProviderRegistry resolves ModelInstance

`registry.resolve_model_instance(name, model, options)` looks up the provider entry, merges its stored config (endpoint, api_key) with overrides, and produces a complete `ModelInstance`. This keeps the engine loop simple.
