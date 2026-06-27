use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use serde_json::json;
use tokio::sync::mpsc;

use super::provider::{
    ChatRequest, ChatRole, FinishReason, LlmError, LlmProvider, StreamEvent, ToolCall,
};

pub struct OpenAIProvider {
    client: Client,
    api_key: String,
    base_url: String,
    model: String,
}

impl OpenAIProvider {
    pub fn new(api_key: String, model: String, base_url: Option<String>) -> Self {
        Self {
            client: Client::new(),
            api_key,
            base_url: base_url.unwrap_or_else(|| "https://api.openai.com/v1".into()),
            model,
        }
    }
}

#[async_trait]
impl LlmProvider for OpenAIProvider {
    fn provider_name(&self) -> &'static str {
        "openai"
    }

    fn supports_model(&self, model: &str) -> bool {
        model.starts_with("gpt-") || model.starts_with("o1") || model.starts_with("o3")
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
            .map(|m| {
                let role = match m.role {
                    ChatRole::System => "system",
                    ChatRole::User => "user",
                    ChatRole::Assistant => "assistant",
                    ChatRole::Tool => "tool",
                };
                let mut msg = json!({
                    "role": role,
                    "content": m.content,
                });
                if let Some(ref id) = m.tool_call_id {
                    msg["tool_call_id"] = json!(id);
                }
                msg
            })
            .collect();

        let mut body = json!({
            "model": model,
            "messages": messages,
            "stream": true,
        });

        if !request.tools.is_empty() {
            body["tools"] = json!(request.tools);
        }
        if let Some(t) = request.temperature {
            body["temperature"] = json!(t);
        }
        if let Some(m) = request.max_tokens {
            body["max_tokens"] = json!(m);
        }

        let client = self.client.clone();
        let api_key = self.api_key.clone();
        let base_url = self.base_url.clone();

        tokio::spawn(async move {
            let response = client
                .post(format!("{}/chat/completions", base_url))
                .header("Authorization", format!("Bearer {}", api_key))
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
                    if line.is_empty() || line == "data: [DONE]" {
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

                    let choices = match val["choices"].as_array() {
                        Some(c) => c,
                        None => continue,
                    };

                    for choice in choices {
                        let delta = &choice["delta"];

                        if let Some(content) = delta["content"].as_str() {
                            if !content.is_empty() {
                                tx.send(StreamEvent::Chunk(content.to_string()))
                                    .await
                                    .ok();
                            }
                        }

                        if let Some(tcs) = delta["tool_calls"].as_array() {
                            for tc in tcs {
                                if let Some(func) = tc["function"].as_object() {
                                    let id = tc["id"].as_str().unwrap_or("").to_string();
                                    let name = func["name"].as_str().unwrap_or("").to_string();
                                    let args_str = func["arguments"].as_str().unwrap_or("{}");
                                    if let Ok(args) = serde_json::from_str(args_str) {
                                        let tool_call = ToolCall {
                                            id,
                                            name,
                                            arguments: args,
                                        };
                                        tx.send(StreamEvent::ToolCall(tool_call))
                                            .await
                                            .ok();
                                    }
                                }
                            }
                        }

                        let finish = &choice["finish_reason"];
                        if !finish.is_null() {
                            let reason = match finish.as_str() {
                                Some("stop") => FinishReason::Stop,
                                Some("length") => FinishReason::Length,
                                Some("tool_calls") => FinishReason::ToolCalls,
                                _ => FinishReason::Stop,
                            };
                            tx.send(StreamEvent::Done(reason)).await.ok();
                        }
                    }
                }
            }
        });

        Ok(rx)
    }
}
