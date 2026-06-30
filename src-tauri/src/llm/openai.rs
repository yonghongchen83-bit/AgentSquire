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
    pub verbose: bool,
}

impl OpenAIProvider {
    pub fn new(api_key: String, model: String, base_url: Option<String>) -> Self {
        Self {
            client: Client::new(),
            api_key,
            base_url: base_url.unwrap_or_else(|| "https://api.openai.com/v1".into()),
            model,
            verbose: false,
        }
    }
}

#[async_trait]
impl LlmProvider for OpenAIProvider {
    fn provider_name(&self) -> &'static str {
        "openai"
    }

    fn verbose(&self) -> bool {
        self.verbose
    }

    fn supports_model(&self, model: &str) -> bool {
        model.starts_with("gpt-") || model.starts_with("o1") || model.starts_with("o3")
    }

    async fn chat(&self, request: ChatRequest) -> Result<mpsc::Receiver<StreamEvent>, LlmError> {
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
                if let Some(ref calls) = m.tool_calls {
                    let arr: Vec<serde_json::Value> = calls
                        .iter()
                        .map(|tc| {
                            json!({
                                "id": tc.id,
                                "type": "function",
                                "function": {
                                    "name": tc.name,
                                    "arguments": serde_json::to_string(&tc.arguments).unwrap_or_default()
                                }
                            })
                        })
                        .collect();
                    msg["tool_calls"] = json!(arr);
                    if m.content.is_empty() {
                        msg["content"] = serde_json::Value::Null;
                    }
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
            let tools: Vec<serde_json::Value> = request
                .tools
                .iter()
                .map(|t| {
                    json!({
                        "type": "function",
                        "function": {
                            "name": t.name,
                            "description": t.description,
                            "parameters": t.input_schema,
                        }
                    })
                })
                .collect();
            body["tools"] = json!(tools);
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
        let verbose = self.verbose();

        tokio::spawn(async move {
            if verbose {
                let body_pretty = serde_json::to_string_pretty(&body).unwrap_or_default();
                tx.send(StreamEvent::Log(format!(
                    ">>> REQUEST to {}\n{}",
                    if base_url.ends_with("/chat/completions") || base_url.ends_with("/responses") {
                        base_url.clone()
                    } else {
                        format!("{}/chat/completions", base_url)
                    },
                    body_pretty,
                )))
                .await
                .ok();
            }

            let response = client
                .post(
                    if base_url.ends_with("/chat/completions") || base_url.ends_with("/responses") {
                        base_url.clone()
                    } else {
                        format!("{}/chat/completions", base_url)
                    },
                )
                .header("Authorization", format!("Bearer {}", api_key))
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await;

            let resp = match response {
                Ok(r) => r,
                Err(e) => {
                    if verbose {
                        tx.send(StreamEvent::Log(format!("<<< CONNECTION FAILED: {}", e)))
                            .await
                            .ok();
                    }
                    tx.send(StreamEvent::Error(
                        "Connection failed: unable to reach the server".to_string(),
                    ))
                    .await
                    .ok();
                    tx.send(StreamEvent::Done(FinishReason::Error)).await.ok();
                    return;
                }
            };

            let status = resp.status();

            if !status.is_success() {
                let body_text = resp.text().await.unwrap_or_default();
                if verbose {
                    tx.send(StreamEvent::Log(format!(
                        "<<< RESPONSE {} {}\n{}",
                        status.as_u16(),
                        status.canonical_reason().unwrap_or(""),
                        body_text,
                    )))
                    .await
                    .ok();
                }
                let err = match status.as_u16() {
                    401 => LlmError::Auth,
                    429 => LlmError::RateLimited,
                    _ => {
                        let msg = if body_text.is_empty() {
                            format!("HTTP {}", status)
                        } else {
                            // Try to extract API error message, fall back to raw body
                            let trimmed = body_text.trim();
                            // Limit to 500 chars to avoid flooding
                            let detail = if trimmed.len() > 500 {
                                format!("{}...", &trimmed[..500])
                            } else {
                                trimmed.to_string()
                            };
                            format!("HTTP {}: {}", status, detail)
                        };
                        LlmError::Api(msg)
                    }
                };
                tx.send(StreamEvent::Error(err.to_string())).await.ok();
                tx.send(StreamEvent::Done(FinishReason::Error)).await.ok();
                return;
            }

            let mut done = false;
            let mut carry = String::new();
            let mut stream = resp.bytes_stream();
            while let Some(chunk_result) = stream.next().await {
                if done {
                    break;
                }

                let bytes = match chunk_result {
                    Ok(b) => b,
                    Err(e) => {
                        tx.send(StreamEvent::Error(e.to_string())).await.ok();
                        tx.send(StreamEvent::Done(FinishReason::Error)).await.ok();
                        return;
                    }
                };

                let text = format!("{}{}", carry, String::from_utf8_lossy(&bytes));
                carry.clear();
                let mut lines = text.split('\n').peekable();
                while let Some(line) = lines.next() {
                    if lines.peek().is_none() && !text.ends_with('\n') {
                        carry = line.to_string();
                        break;
                    }

                    let line = line.trim();
                    if line.is_empty() {
                        continue;
                    }

                    if verbose {
                        tx.send(StreamEvent::Log(format!("<<< RAW {}", line)))
                            .await
                            .ok();
                    }

                    if line == "data: [DONE]" {
                        tx.send(StreamEvent::Done(FinishReason::Stop)).await.ok();
                        done = true;
                        break;
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

                        let content_text = delta["content"].as_str().unwrap_or("");
                        let reasoning_text = delta["reasoning_content"]
                            .as_str()
                            .or_else(|| delta["reasoning"].as_str())
                            .unwrap_or("");

                        if !content_text.is_empty() {
                            if verbose {
                                tx.send(StreamEvent::Log(format!(
                                    "<<< CHUNK content: {}",
                                    content_text
                                )))
                                .await
                                .ok();
                            }
                            tx.send(StreamEvent::Chunk(content_text.to_string()))
                                .await
                                .ok();
                        }
                        if !reasoning_text.is_empty() {
                            if verbose {
                                tx.send(StreamEvent::Log(format!(
                                    "<<< CHUNK thinking: {}",
                                    reasoning_text
                                )))
                                .await
                                .ok();
                            }
                            tx.send(StreamEvent::ThinkingChunk(reasoning_text.to_string()))
                                .await
                                .ok();
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
                                        tx.send(StreamEvent::ToolCall(tool_call)).await.ok();
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
                            done = true;
                            break;
                        }
                    }
                }
            }

            // Process any trailing final line that may arrive without a newline terminator.
            if !done {
                let line = carry.trim();
                if line == "data: [DONE]" {
                    tx.send(StreamEvent::Done(FinishReason::Stop)).await.ok();
                }
            }
        });

        Ok(rx)
    }
}
