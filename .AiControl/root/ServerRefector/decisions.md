# Decisions — ServerRefector

## ADR-like Decisions

### 1. ModelInstance owns all model parameters

`ModelInstance` bundles provider + model + endpoint + api_key + options into a single serializable struct.
The alternative (passing separate fields) leads to the current ad-hoc pattern.
**Decision:** `ModelInstance` is the single value that describes "which model to call with what parameters."

### 2. ModelOptions is an enum-map / typed-bag, not a JSON value

Strong typing over `serde_json::Value` — each option (`thinking_level`, `reasoning_effort`, `max_tokens`, `temperature`) is a named field. Providers that don't support an option simply ignore it.
**Decision:** `ModelOptions` is a struct with `Option<T>` fields, not a `HashMap<String, Value>`.

### 3. RuntimeContext is the single input to Engine::run()

The engine should not reach into Tauri's `AppState`. All dependencies are injected via `RuntimeContext`.
**Decision:** `Engine::run(context: RuntimeContext)` is the only entry point. No `AppHandle` or `State` inside the engine.

### 4. RuntimeConfig uses a typed struct + an extensible HashMap

Core config fields (`verbose_logging`, `squire_prefetch`) are typed fields on `RuntimeConfig`.
Test-specific or engine-specific flags go into `test_config: HashMap<String, String>`.
**Decision:** Two-tier config — typed for known fields, map for extensibility.

### 5. Phase 2 is part of the engine, not the command handler

Currently Phase 2 is spawned from `send_message_impl`. After refactoring, `SquireEngine` internally handles Phase 2 lifecycle.
**Decision:** Phase 2 orchestration moves into the engine.

### 6. Existing provider traits remain

We don't change the `LlmProvider` trait signature drastically — we add `ModelInstance` as a parameter to `chat()`. The trait itself stays in `provider-core`.
**Decision:** Evolve, don't replace, the provider trait.

