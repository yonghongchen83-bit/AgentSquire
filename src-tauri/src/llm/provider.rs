#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_message_roles() {
        let msg = ChatMessage {
            role: ChatRole::User,
            content: "hello".into(),
            tool_call_id: None,
            tool_calls: None,
        };
        assert!(matches!(msg.role, ChatRole::User));
        assert_eq!(msg.content, "hello");
    }

    #[test]
    fn test_tool_definition() {
        let def = ToolDefinition {
            name: "read_file".into(),
            description: "Read a file".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"}
                }
            }),
        };
        assert_eq!(def.name, "read_file");
        assert!(def.input_schema.get("type").is_some());
    }

    #[test]
    fn test_finish_reason_display() {
        match FinishReason::Stop {
            FinishReason::Stop => {}
            _ => panic!("expected Stop"),
        }
    }

    #[test]
    fn test_tool_call() {
        let tc = ToolCall {
            id: "call_123".into(),
            name: "search".into(),
            arguments: serde_json::json!({"query": "fn main"}),
        };
        assert_eq!(tc.id, "call_123");
        assert_eq!(tc.arguments["query"], "fn main");
    }

    #[test]
    fn test_chat_message_with_tool_calls() {
        let tc = ToolCall {
            id: "call_1".into(),
            name: "read_file".into(),
            arguments: serde_json::json!({"path": "/test"}),
        };
        let msg = ChatMessage {
            role: ChatRole::Assistant,
            content: String::new(),
            tool_call_id: None,
            tool_calls: Some(vec![tc]),
        };
        assert!(msg.tool_calls.is_some());
        assert_eq!(msg.tool_calls.as_ref().unwrap().len(), 1);
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("tool_calls"));
        assert!(json.contains("read_file"));
    }

    #[test]
    fn test_chat_message_tool_result() {
        let msg = ChatMessage {
            role: ChatRole::Tool,
            content: "file contents".into(),
            tool_call_id: Some("call_1".into()),
            tool_calls: None,
        };
        assert_eq!(msg.tool_call_id.as_deref(), Some("call_1"));
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("tool_call_id"));
    }

    #[test]
    fn test_chat_message_serialize_skips_empty() {
        let msg = ChatMessage {
            role: ChatRole::User,
            content: "hi".into(),
            tool_call_id: None,
            tool_calls: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(!json.contains("tool_calls"));
        assert!(!json.contains("tool_call_id"));
    }

    #[test]
    fn test_llm_error_display() {
        let err = LlmError::Api("rate limit".into());
        assert_eq!(err.to_string(), "API error: rate limit");
        let err = LlmError::Auth;
        assert_eq!(err.to_string(), "Authentication failed");
        let err = LlmError::RateLimited;
        assert_eq!(err.to_string(), "Rate limited");
        let err = LlmError::ContextLength;
        assert_eq!(err.to_string(), "Context length exceeded");
    }
}

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

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
    ToolCall(ToolCall),
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
    async fn chat(
        &self,
        request: ChatRequest,
    ) -> Result<mpsc::Receiver<StreamEvent>, LlmError>;

    fn supports_model(&self, model: &str) -> bool;
    fn provider_name(&self) -> &'static str;
}
