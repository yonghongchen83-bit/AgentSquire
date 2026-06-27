use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use serde_json::json;
use tokio::sync::mpsc;

use super::provider::{
    ChatRequest, ChatRole, FinishReason, LlmError, LlmProvider, StreamEvent, ToolCall,
};

pub struct AnthropicProvider {
    client: Client,
    api_key: String,
    base_url: String,
    model: String,
}

impl AnthropicProvider {
    pub fn new(api_key: String, model: String, base_url: Option<String>) -> Self {
        Self {
            client: Client::new(),
            api_key,
            base_url: base_url.unwrap_or_else(|| "https://api.anthropic.com/v1".into()),
            model,
        }
    }
}

#[async_trait]
impl LlmProvider for AnthropicProvider {
    fn provider_name(&self) -> &'static str {
        "anthropic"
    }

    fn supports_model(&self, model: &str) -> bool {
        model.starts_with("claude-")
    }

    async fn chat(
        &self,
        request: ChatRequest,
    ) -> Result<mpsc::Receiver<StreamEvent>, LlmError> {
        let (tx, rx) = mpsc::channel(64);

        let model = if request.model.is_empty() {
            self.model.clone()
        } else {
            request.model
        };

        let messages: Vec<serde_json::Value> = request
            .messages
            .iter()
            .filter(|m| !matches!(m.role, ChatRole::System))
            .map(|m| {
                match m.role {
                    ChatRole::Tool => {
                        json!({
                            "role": "user",
                            "content": [
                                {
                                    "type": "tool_result",
                                    "tool_use_id": m.tool_call_id.as_deref().unwrap_or("unknown"),
                                    "content": m.content
                                }
                            ]
                        })
                    }
                    _ => {
                        let role = match m.role {
                            ChatRole::User => "user",
                            ChatRole::Assistant => "assistant",
                            _ => "user",
                        };
                        if let Some(ref calls) = m.tool_calls {
                            let mut content_arr: Vec<serde_json::Value> = Vec::new();
                            if !m.content.is_empty() {
                                content_arr.push(json!({
                                    "type": "text",
                                    "text": m.content
                                }));
                            }
                            for tc in calls {
                                content_arr.push(json!({
                                    "type": "tool_use",
                                    "id": tc.id,
                                    "name": tc.name,
                                    "input": tc.arguments
                                }));
                            }
                            json!({
                                "role": role,
                                "content": content_arr
                            })
                        } else {
                            json!({
                                "role": role,
                                "content": m.content
                            })
                        }
                    }
                }
            })
            .collect();

        let system: Vec<&str> = request
            .messages
            .iter()
            .filter(|m| matches!(m.role, ChatRole::System))
            .map(|m| m.content.as_str())
            .collect();

        let mut body = json!({
            "model": model,
            "messages": messages,
            "max_tokens": request.max_tokens.unwrap_or(4096),
            "stream": true,
        });

        if !system.is_empty() {
            body["system"] = json!(system.join("\n"));
        }

        if !request.tools.is_empty() {
            body["tools"] = json!(request.tools);
        }
        if let Some(t) = request.temperature {
            body["temperature"] = json!(t);
        }

        let client = self.client.clone();
        let api_key = self.api_key.clone();
        let base_url = self.base_url.clone();

        tokio::spawn(async move {
            let response = client
                .post(format!("{}/messages", base_url))
                .header("x-api-key", &api_key)
                .header("anthropic-version", "2023-06-01")
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await;

            let resp = match response {
                Ok(r) => r,
                Err(e) => {
                    tx.send(StreamEvent::Error(e.to_string())).await.ok();
                    tx.send(StreamEvent::Done(FinishReason::Error)).await.ok();
                    return;
                }
            };

            if !resp.status().is_success() {
                let status = resp.status();
                let err_text = resp.text().await.unwrap_or_default();
                let err = match status.as_u16() {
                    401 => LlmError::Auth,
                    429 => LlmError::RateLimited,
                    _ => LlmError::Api(format!("HTTP {}: {}", status, err_text)),
                };
                tx.send(StreamEvent::Error(err.to_string())).await.ok();
                tx.send(StreamEvent::Done(FinishReason::Error)).await.ok();
                return;
            }

            let mut stream = resp.bytes_stream();
            while let Some(chunk_result) = stream.next().await {
                let bytes = match chunk_result {
                    Ok(b) => b,
                    Err(e) => {
                        tx.send(StreamEvent::Error(e.to_string())).await.ok();
                        tx.send(StreamEvent::Done(FinishReason::Error)).await.ok();
                        return;
                    }
                };

                let text = String::from_utf8_lossy(&bytes);
                for line in text.lines() {
                    let line = line.trim();
                    if line.is_empty() {
                        continue;
                    }

                    let data = match line.strip_prefix("data: ") {
                        Some(d) => d,
                        None => continue,
                    };

                    let val: serde_json::Value = match serde_json::from_str(data) {
                        Ok(v) => v,
                        Err(_) => continue,
                    };

                    let event_type = val["type"].as_str().unwrap_or("");
                    match event_type {
                        "content_block_delta" => {
                            if let Some(text_val) = val["delta"]["text"].as_str() {
                                tx.send(StreamEvent::Chunk(text_val.to_string()))
                                    .await
                                    .ok();
                            }
                        }
                        "content_block_start" => {
                            if val["content_block"]["type"] == "tool_use" {
                                let tc = ToolCall {
                                    id: val["content_block"]["id"]
                                        .as_str()
                                        .unwrap_or("")
                                        .to_string(),
                                    name: val["content_block"]["name"]
                                        .as_str()
                                        .unwrap_or("")
                                        .to_string(),
                                    arguments: serde_json::Value::Object(Default::default()),
                                };
                                tx.send(StreamEvent::ToolCall(tc)).await.ok();
                            }
                        }
                        "message_delta" => {
                            if let Some(stop_reason) = val["delta"]["stop_reason"].as_str() {
                                let reason = match stop_reason {
                                    "end_turn" | "stop_sequence" => FinishReason::Stop,
                                    "max_tokens" => FinishReason::Length,
                                    "tool_use" => FinishReason::ToolCalls,
                                    _ => FinishReason::Stop,
                                };
                                tx.send(StreamEvent::Done(reason)).await.ok();
                            }
                        }
                        _ => {}
                    }
                }
            }
        });

        Ok(rx)
    }
}
