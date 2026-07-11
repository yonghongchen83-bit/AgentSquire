use async_trait::async_trait;
use futures::StreamExt;
use provider_core::{
    ChatRequest, ChatRole, FinishReason, LlmError, LlmProvider, StreamEvent, ToolCall,
};
use reqwest::Client;
use serde_json::json;
use tokio::sync::mpsc;

pub struct AnthropicProvider {
    client: Client,
    api_key: String,
    base_url: String,
    model: String,
    pub verbose: bool,
}

impl AnthropicProvider {
    pub fn new(api_key: String, model: String, base_url: Option<String>) -> Self {
        Self {
            client: Client::new(),
            api_key,
            base_url: base_url.unwrap_or_else(|| "https://api.anthropic.com/v1".into()),
            model,
            verbose: false,
        }
    }
}

fn normalize_level(level: Option<String>) -> String {
    let raw = level.unwrap_or_else(|| "mid".to_string()).to_lowercase();
    match raw.as_str() {
        "default" | "none" | "low" | "mid" | "high" => raw,
        _ => "mid".to_string(),
    }
}

fn anthropic_budget_tokens(level: &str) -> Option<u32> {
    match level {
        "low" => Some(1024),
        "mid" => Some(4096),
        "high" => Some(8192),
        _ => None,
    }
}

#[async_trait]
impl LlmProvider for AnthropicProvider {
    fn provider_name(&self) -> &'static str {
        "anthropic"
    }

    fn verbose(&self) -> bool {
        self.verbose
    }

    fn supports_model(&self, model: &str) -> bool {
        model.starts_with("claude-")
    }

    async fn chat_with_instance(
        &self,
        instance: &provider_core::ModelInstance,
        request: provider_core::ChatRequest,
    ) -> Result<mpsc::Receiver<StreamEvent>, LlmError> {
        let override_endpoint = instance
            .endpoint
            .as_ref()
            .filter(|e| *e != &self.base_url);
        let override_key = instance.api_key.as_ref().filter(|k| *k != &self.api_key);

        if override_endpoint.is_some() || override_key.is_some() {
            let temp = AnthropicProvider {
                client: self.client.clone(),
                api_key: override_key
                    .map(|k| k.clone())
                    .unwrap_or_else(|| self.api_key.clone()),
                base_url: override_endpoint
                    .map(|e| e.clone())
                    .unwrap_or_else(|| self.base_url.clone()),
                model: self.model.clone(),
                verbose: self.verbose,
            };
            let mut req = request;
            instance.apply_to_request(&mut req);
            temp.chat(req).await
        } else {
            let mut req = request;
            instance.apply_to_request(&mut req);
            self.chat(req).await
        }
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

        let thinking_level = normalize_level(request.thinking_level.clone());
        let thinking_budget = anthropic_budget_tokens(&thinking_level);

        let default_max_tokens = if thinking_budget.is_some() { 16384 } else { 4096 };

        let mut body = json!({
            "model": model,
            "messages": messages,
            "max_tokens": request.max_tokens.unwrap_or(default_max_tokens),
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
        if let Some(budget_tokens) = thinking_budget {
            body["thinking"] = json!({
                "type": "enabled",
                "budget_tokens": budget_tokens,
            });
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
                    if base_url.ends_with("/messages") {
                        base_url.clone()
                    } else {
                        format!("{}/messages", base_url)
                    },
                    body_pretty,
                )))
                .await
                .ok();
            }

            let response = client
                .post(if base_url.ends_with("/messages") {
                    base_url.clone()
                } else {
                    format!("{}/messages", base_url)
                })
                .header("x-api-key", &api_key)
                .header("anthropic-version", "2023-06-01")
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
                    tx.send(StreamEvent::Error(e.to_string())).await.ok();
                    tx.send(StreamEvent::Done(FinishReason::Error)).await.ok();
                    return;
                }
            };

            if !resp.status().is_success() {
                let status = resp.status();
                let err_text = resp.text().await.unwrap_or_default();
                if verbose {
                    tx.send(StreamEvent::Log(format!(
                        "<<< RESPONSE {} {}\n{}",
                        status.as_u16(),
                        status.canonical_reason().unwrap_or(""),
                        err_text,
                    )))
                    .await
                    .ok();
                }
                let err = match status.as_u16() {
                    401 => LlmError::Auth,
                    429 => LlmError::RateLimited,
                    _ => LlmError::Api(format!("HTTP {}: {}", status, err_text)),
                };
                tx.send(StreamEvent::Error(err.to_string())).await.ok();
                tx.send(StreamEvent::Done(FinishReason::Error)).await.ok();
                return;
            }

            if verbose {
                tx.send(StreamEvent::Log(format!(
                    "<<< RESPONSE {} {} (stream opened)",
                    resp.status().as_u16(),
                    resp.status().canonical_reason().unwrap_or(""),
                )))
                .await
                .ok();
            }

            let mut emitted_done = false;
            let mut stream = resp.bytes_stream();
            let mut partial_json = String::new();
            let mut pending_tool_use: std::collections::HashMap<String, (String, serde_json::Map<String, serde_json::Value>)> =
                std::collections::HashMap::new();

            while let Some(chunk_result) = stream.next().await {
                let chunk = match chunk_result {
                    Ok(c) => c,
                    Err(e) => {
                        tx.send(StreamEvent::Error(e.to_string())).await.ok();
                        tx.send(StreamEvent::Done(FinishReason::Error)).await.ok();
                        emitted_done = true;
                        break;
                    }
                };

                let text = String::from_utf8_lossy(&chunk);
                for line in text.lines() {
                    if let Some(data) = line.strip_prefix("data: ") {
                        if data == "[DONE]" {
                            if !emitted_done {
                                tx.send(StreamEvent::Done(FinishReason::Stop)).await.ok();
                                emitted_done = true;
                            }
                            break;
                        }

                        let payload = if partial_json.is_empty() {
                            data.to_string()
                        } else {
                            partial_json.push_str(data);
                            partial_json.clone()
                        };

                        let event: serde_json::Value = match serde_json::from_str(&payload) {
                            Ok(v) => {
                                partial_json.clear();
                                v
                            }
                            Err(_) => {
                                partial_json = payload;
                                continue;
                            }
                        };

                        let event_type = event["type"].as_str().unwrap_or("");

                        match event_type {
                            "content_block_delta" => {
                                if let Some(delta) = event.get("delta") {
                                    if let Some(text) = delta.get("text").and_then(|v| v.as_str()) {
                                        tx.send(StreamEvent::Chunk(text.to_string())).await.ok();
                                    }
                                    if let Some(text) = delta.get("thinking").and_then(|v| v.as_str()) {
                                        tx.send(StreamEvent::Thinking(text.to_string())).await.ok();
                                    }

                                    if let Some(partial) = delta
                                        .get("partial_json")
                                        .and_then(|v| v.as_str())
                                    {
                                        if let Some(index) = event["index"].as_u64() {
                                            let key = index.to_string();
                                            let entry = pending_tool_use
                                                .entry(key.clone())
                                                .or_insert_with(|| {
                                                    let name = event
                                                        .get("content_block")
                                                        .and_then(|cb| cb.get("name"))
                                                        .and_then(|v| v.as_str())
                                                        .unwrap_or_default()
                                                        .to_string();
                                                    (name, serde_json::Map::new())
                                                });

                                            let mut merged = serde_json::to_string(&entry.1)
                                                .unwrap_or_else(|_| "{}".to_string());
                                            merged.push_str(partial);
                                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&merged) {
                                                if let Some(obj) = v.as_object() {
                                                    entry.1 = obj.clone();
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            "content_block_stop" => {
                                if let Some(index) = event["index"].as_u64() {
                                    let key = index.to_string();
                                    if let Some((name, args)) = pending_tool_use.remove(&key) {
                                        if !name.is_empty() {
                                            let tool_call = ToolCall {
                                                id: format!("anthropic-tool-{}", key),
                                                name,
                                                arguments: serde_json::Value::Object(args),
                                            };
                                            tx.send(StreamEvent::ToolCall(tool_call)).await.ok();
                                        }
                                    }
                                }
                            }
                            "message_stop" => {
                                if !emitted_done {
                                    tx.send(StreamEvent::Done(FinishReason::Stop)).await.ok();
                                    emitted_done = true;
                                }
                            }
                            _ => {}
                        }
                    }
                }

                if emitted_done {
                    break;
                }
            }

            if !emitted_done {
                tx.send(StreamEvent::Done(FinishReason::Stop)).await.ok();
            }
        });

        Ok(rx)
    }
}
