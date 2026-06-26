# ADR-0003: LLM Provider Abstraction via Rust Trait

**Status:** Accepted

**Date:** 2026-06-26

## Context

The app must support multiple LLM providers (OpenAI, Anthropic, local models via llama.cpp). We need to avoid coupling to any single provider's SDK or API shape. New providers should be addable without modifying existing conversation logic or tool execution code.

## Decision

Define a provider-agnostic `LlmProvider` trait in Rust. Each provider (OpenAI, Anthropic, local) implements this trait. The rest of the app depends only on the trait.

```rust
/// Core provider interface — the only API surface app logic touches.
#[async_trait]
trait LlmProvider {
    /// Send a chat request and receive a streaming response.
    async fn chat(
        &self,
        request: ChatRequest,
    ) -> Result<ChatResponse>;

    /// Optional: check if a specific model is available.
    fn supports_model(&self, model: &str) -> bool;
}

/// Provider registry: string key → boxed provider.
/// Retrieved from config at startup, injected via dependency.
type ProviderRegistry = HashMap<String, Box<dyn LlmProvider>>;
```

- `ChatRequest` / `ChatResponse` are plain data structs — no provider-specific types leak
- Streaming is handled via `tokio::sync::mpsc` channel inside `ChatResponse`
- Tool/function definitions are part of `ChatRequest`, tool results part of `ChatResponse`

## Consequences

### Positive

- Zero coupling to any specific provider SDK in app/business logic
- Adding a new provider = write one `impl LlmProvider` + register it, nothing else changes
- Can mock/fake providers in tests by implementing the trait
- Provider version bumps or SDK API changes are isolated to the impl file

### Negative

- Trait design must be right early — remodelling `ChatRequest`/`ChatResponse` later affects all impls
- Some provider-specific features (e.g., Anthropic's extended thinking) may not fit the common interface cleanly — may need optional methods or feature flags on the trait
