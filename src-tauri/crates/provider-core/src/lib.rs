use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::mpsc;

// ── ModelInstance ──────────────────────────────────────────────────────
/// A complete description of which model to call, with what parameters.
///
/// Bundles provider identity, model ID, endpoint override, API key override,
/// and per-model options into a single serializable value. This is the
/// "model config" that the UI selects/modifies and the engine passes to
/// providers. Tests can construct a `ModelInstance` directly for headless
/// testing without any UI or config file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelInstance {
    /// Identifies which provider entry in the registry (e.g. "openai", "my-custom").
    pub provider_name: String,
    /// The model ID string (e.g. "gpt-4o", "claude-sonnet-4-20250514").
    pub model: String,
    /// Optional override for the provider's base URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    /// Optional ephemeral API key override. If `None`, the provider falls
    /// back to its stored key from config.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// Per-model options (thinking level, temperature, etc.).
    pub options: ModelOptions,
}

impl ModelInstance {
    /// Create a new `ModelInstance` with the given provider, model, and
    /// default options.
    pub fn new(provider_name: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            provider_name: provider_name.into(),
            model: model.into(),
            endpoint: None,
            api_key: None,
            options: ModelOptions::default(),
        }
    }

    /// Apply this instance's model + options onto a `ChatRequest`.
    /// Overrides `request.model`, `thinking_level`, `temperature`, and
    /// `max_tokens` in place.
    pub fn apply_to_request(&self, request: &mut ChatRequest) {
        request.model = self.model.clone();
        request.thinking_level = self.options.thinking_level.clone();
        request.temperature = self.options.temperature;
        request.max_tokens = self.options.max_tokens;
    }
}

/// Per-model configuration parameters.
///
/// Strongly typed — each supported option is a named `Option<T>` field.
/// Providers that don't support a particular option simply ignore it.
/// The `extra` map catches provider-specific parameters not covered by
/// the typed fields.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelOptions {
    /// Thinking/reasoning level: "none", "low", "mid", "high", or "default".
    /// Mapped to `reasoning_effort` for OpenAI o-series, `thinking` for
    /// Anthropic Claude, etc.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_level: Option<String>,
    /// Reasoning effort override for o-series models ("low", "medium", "high").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<String>,
    /// Maximum tokens to generate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    /// Temperature for sampling (0.0 – 2.0).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// Provider-specific parameters not covered by the typed fields above.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub extra: HashMap<String, String>,
}

impl Default for ModelOptions {
    fn default() -> Self {
        Self {
            thinking_level: None,
            reasoning_effort: None,
            max_tokens: None,
            temperature: None,
            extra: HashMap::new(),
        }
    }
}

// ── End ModelInstance ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChatRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub tools: Vec<ToolDefinition>,
    pub thinking_level: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FinishReason {
    Stop,
    Length,
    ToolCalls,
    Error,
}

#[derive(Debug, Clone)]
pub enum StreamEvent {
    Chunk(String),
    Thinking(String),
    ToolCall(ToolCall),
    Log(String),
    Done(FinishReason),
    Error(String),
}

#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("API error: {0}")]
    Api(String),
    #[error("Rate limited")]
    RateLimited,
    #[error("Authentication failed")]
    Auth,
    #[error("Context length exceeded")]
    ContextLength,
    #[error("Request failed: {0}")]
    Request(String),
}

#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Send a chat request and return a stream of events.
    async fn chat(&self, request: ChatRequest) -> Result<mpsc::Receiver<StreamEvent>, LlmError>;

    /// Send a chat request using a `ModelInstance` for model/options override.
    ///
    /// Default implementation calls `self.chat(request)` after applying the
    /// model instance's model + options onto the request. Providers that need
    /// to use the instance's `endpoint` or `api_key` overrides should override
    /// this method.
    async fn chat_with_instance(
        &self,
        instance: &ModelInstance,
        mut request: ChatRequest,
    ) -> Result<mpsc::Receiver<StreamEvent>, LlmError> {
        instance.apply_to_request(&mut request);
        self.chat(request).await
    }

    fn supports_model(&self, model: &str) -> bool;
    fn provider_name(&self) -> &'static str;
    fn verbose(&self) -> bool {
        false
    }
}
