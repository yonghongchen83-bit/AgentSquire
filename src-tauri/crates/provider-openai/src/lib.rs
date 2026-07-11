use async_trait::async_trait;
use futures::StreamExt;
use provider_core::{
    ChatRequest, ChatRole, FinishReason, LlmError, LlmProvider, StreamEvent, ToolCall,
};
use reqwest::Client;
use serde_json::json;
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use tokio::sync::mpsc;

pub struct OpenAIProvider {
    client: Client,
    api_key: String,
    base_url: String,
    model: String,
    pub verbose: bool,
    wire_log_path: Option<PathBuf>,
}

#[derive(Default, Clone)]
struct PendingToolCall {
    id: String,
    name: String,
    arguments: String,
}

fn append_wire_log(path: Option<&PathBuf>, entry: &str) {
    let Some(path) = path else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(
            file,
            "\n===== {} =====\n{}",
            chrono::Utc::now().to_rfc3339(),
            entry
        );
    }
}

fn normalize_level(level: Option<String>) -> String {
    let raw = level.unwrap_or_else(|| "default".to_string()).to_lowercase();
    match raw.as_str() {
        "default" | "none" | "low" | "mid" | "high" => raw,
        _ => "default".to_string(),
    }
}

fn accumulate_classic_tool_call_chunk(
    pending_tool_calls: &mut HashMap<usize, PendingToolCall>,
    tc: &serde_json::Value,
) {
    let slot = tc["index"].as_u64().map(|v| v as usize).unwrap_or(0);
    let pending = pending_tool_calls.entry(slot).or_default();

    if let Some(id) = tc["id"].as_str() {
        pending.id = id.to_string();
    }

    if let Some(func) = tc["function"].as_object() {
        if let Some(name) = func.get("name").and_then(|v| v.as_str()) {
            pending.name = name.to_string();
        }
        if let Some(arg_chunk) = func.get("arguments").and_then(|v| v.as_str()) {
            pending.arguments.push_str(arg_chunk);
        }
    }
}

fn flush_pending_tool_calls(
    pending_tool_calls: &mut HashMap<usize, PendingToolCall>,
) -> Vec<ToolCall> {
    let mut ordered_slots: Vec<usize> = pending_tool_calls.keys().copied().collect();
    ordered_slots.sort_unstable();

    let mut tool_calls = Vec::new();
    for slot in ordered_slots {
        if let Some(pending) = pending_tool_calls.remove(&slot) {
            if pending.id.is_empty() || pending.name.is_empty() {
                continue;
            }

            let args_json =
                serde_json::from_str::<serde_json::Value>(if pending.arguments.trim().is_empty() {
                    "{}"
                } else {
                    pending.arguments.as_str()
                });

            if let Ok(arguments) = args_json {
                tool_calls.push(ToolCall {
                    id: pending.id,
                    name: pending.name,
                    arguments,
                });
            }
        }
    }

    tool_calls
}

impl OpenAIProvider {
    pub fn new(api_key: String, model: String, base_url: Option<String>) -> Self {
        let client = Client::builder()
            .connect_timeout(std::time::Duration::from_secs(15)) // 15s to establish connection
            .build()
            .expect("Failed to build reqwest Client");
        Self {
            client,
            api_key,
            base_url: base_url.unwrap_or_else(|| "https://api.openai.com/v1".into()),
            model,
            verbose: false,
            wire_log_path: None,
        }
    }

    pub fn with_wire_log_path(mut self, path: PathBuf) -> Self {
        self.wire_log_path = Some(path);
        self
    }
}

fn truncate_chars(input: &str, max_chars: usize) -> String {
    let mut chars = input.chars();
    let head: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{}...", head)
    } else {
        head
    }
}

fn effort_for_level(level: &str) -> Option<&'static str> {
    match level {
        "low" => Some("low"),
        "mid" => Some("medium"),
        "high" => Some("high"),
        _ => None,
    }
}

fn is_openai_reasoning_family(model: &str) -> bool {
    let m = model.to_lowercase();
    m.starts_with("o1") || m.starts_with("o3") || m.starts_with("o4") || m.starts_with("gpt-5")
}

fn is_deepseek_family(model: &str) -> bool {
    let m = model.to_lowercase();
    m.starts_with("deepseek-")
}

fn is_openai_compat_reasoning_effort_family(model: &str) -> bool {
    let m = model.to_lowercase();
    m.contains("qwen")
        || m.contains("grok")
        || m.contains("glm")
        || m.contains("kimi")
        || m.contains("minimax")
        || m.contains("nemotron")
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
        // OpenAI-native models
        if model.starts_with("gpt-") || model.starts_with("o1") || model.starts_with("o3") {
            return true;
        }
        // OpenRouter hosts models from many providers — accept anything
        // that looks like a model name (non-empty, no spaces).
        let m = model.trim();
        !m.is_empty() && !m.contains(' ')
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
            let temp = OpenAIProvider {
                client: self.client.clone(),
                api_key: override_key
                    .map(|k| k.clone())
                    .unwrap_or_else(|| self.api_key.clone()),
                base_url: override_endpoint
                    .map(|e| e.clone())
                    .unwrap_or_else(|| self.base_url.clone()),
                model: self.model.clone(),
                verbose: self.verbose,
                wire_log_path: self.wire_log_path.clone(),
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
                if matches!(m.role, ChatRole::Tool) {
                    if let Some(ref id) = m.tool_call_id {
                        msg["tool_call_id"] = json!(id);
                    }
                }
                if let Some(ref rc) = m.reasoning_content {
                    msg["reasoning_content"] = json!(rc);
                }
                if let Some(ref calls) = m.tool_calls {
                    if !calls.is_empty() {
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
                }
                msg
            })
            .collect();

        let mut body = json!({
            "model": model,
            "messages": messages,
            "stream": true,
        });

        let thinking_level = normalize_level(request.thinking_level.clone());
        if is_deepseek_family(&model) {
            match thinking_level.as_str() {
                "default" => {}
                "none" => {
                    body["thinking"] = json!({ "type": "disabled" });
                }
                "high" => {
                    body["thinking"] = json!({ "type": "enabled" });
                    body["reasoning_effort"] = json!("max");
                }
                "low" | "mid" => {
                    body["thinking"] = json!({ "type": "enabled" });
                    body["reasoning_effort"] = json!("high");
                }
                _ => {
                    body["thinking"] = json!({ "type": "enabled" });
                    body["reasoning_effort"] = json!("high");
                }
            }
        } else if let Some(effort) = effort_for_level(&thinking_level) {
            if is_openai_reasoning_family(&model) {
                body["reasoning"] = json!({ "effort": effort });
            }
            if is_openai_compat_reasoning_effort_family(&model) {
                body["reasoning_effort"] = json!(effort);
            }
        }

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
        let wire_log_path = self.wire_log_path.clone();

        tokio::spawn(async move {
            if verbose {
                let body_pretty = serde_json::to_string_pretty(&body).unwrap_or_default();
                let request_url = if base_url.ends_with("/chat/completions")
                    || base_url.ends_with("/responses")
                {
                    base_url.clone()
                } else {
                    format!("{}/chat/completions", base_url)
                };
                let request_log = format!(">>> REQUEST to {}\n{}", request_url, body_pretty);
                append_wire_log(wire_log_path.as_ref(), &request_log);
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
                        let log_line = format!("<<< CONNECTION FAILED: {}", e);
                        append_wire_log(wire_log_path.as_ref(), &log_line);
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
                    let response_log = format!(
                        "<<< {} {}\n{}",
                        status.as_u16(),
                        status.canonical_reason().unwrap_or(""),
                        body_text,
                    );
                    append_wire_log(wire_log_path.as_ref(), &response_log);
                }
                let err = match status.as_u16() {
                    401 => LlmError::Auth,
                    429 => LlmError::RateLimited,
                    _ => {
                        let msg = if body_text.is_empty() {
                            format!("HTTP {} ({})", status.as_u16(), status)
                        } else {
                            let short = truncate_chars(&body_text, 500);
                            format!("HTTP {}: {}", status.as_u16(), short)
                        };
                        LlmError::Api(msg)
                    }
                };
                tx.send(StreamEvent::Error(err.to_string())).await.ok();
                tx.send(StreamEvent::Done(FinishReason::Error)).await.ok();
                return;
            }

            if verbose {
                let log_line = format!(
                    "<<< RESPONSE {} {} (stream opened)",
                    status.as_u16(),
                    status.canonical_reason().unwrap_or(""),
                );
                append_wire_log(wire_log_path.as_ref(), &log_line);
            }

            let mut stream = resp.bytes_stream();
            let mut pending_tool_calls: HashMap<usize, PendingToolCall> = HashMap::new();
            let mut emitted_done = false;

            while let Some(chunk_res) = stream.next().await {
                let chunk = match chunk_res {
                    Ok(c) => c,
                    Err(e) => {
                        tx.send(StreamEvent::Error(e.to_string())).await.ok();
                        tx.send(StreamEvent::Done(FinishReason::Error)).await.ok();
                        emitted_done = true;
                        break;
                    }
                };

                let text = String::from_utf8_lossy(&chunk);
                for raw_line in text.split('\n') {
                    let line = raw_line.trim();
                    if line.is_empty() {
                        continue;
                    }
                    if !line.starts_with("data:") {
                        continue;
                    }
                    let payload = line.trim_start_matches("data:").trim();
                    if payload == "[DONE]" {
                        let tool_calls = flush_pending_tool_calls(&mut pending_tool_calls);
                        for call in tool_calls {
                            tx.send(StreamEvent::ToolCall(call)).await.ok();
                        }
                        tx.send(StreamEvent::Done(FinishReason::Stop)).await.ok();
                        emitted_done = true;
                        break;
                    }

                    let parsed: serde_json::Value = match serde_json::from_str(payload) {
                        Ok(v) => v,
                        Err(_) => {
                            continue;
                        }
                    };

                    if let Some(choices) = parsed.get("choices").and_then(|v| v.as_array()) {
                        for choice in choices {
                            if let Some(delta) = choice.get("delta") {
                                if let Some(content) = delta.get("content").and_then(|v| v.as_str()) {
                                    tx.send(StreamEvent::Chunk(content.to_string())).await.ok();
                                }

                                if let Some(reasoning) = delta
                                    .get("reasoning_content")
                                    .and_then(|v| v.as_str())
                                {
                                    tx.send(StreamEvent::Thinking(reasoning.to_string())).await.ok();
                                }

                                if let Some(tcalls) = delta.get("tool_calls").and_then(|v| v.as_array()) {
                                    for tc in tcalls {
                                        accumulate_classic_tool_call_chunk(
                                            &mut pending_tool_calls,
                                            tc,
                                        );
                                    }
                                }
                            }

                            if let Some(reason) = choice.get("finish_reason").and_then(|v| v.as_str()) {
                                if reason == "tool_calls" {
                                    let tool_calls = flush_pending_tool_calls(&mut pending_tool_calls);
                                    for call in tool_calls {
                                        tx.send(StreamEvent::ToolCall(call)).await.ok();
                                    }
                                    tx.send(StreamEvent::Done(FinishReason::ToolCalls)).await.ok();
                                    emitted_done = true;
                                    break;
                                }
                                if reason == "stop" {
                                    let tool_calls = flush_pending_tool_calls(&mut pending_tool_calls);
                                    for call in tool_calls {
                                        tx.send(StreamEvent::ToolCall(call)).await.ok();
                                    }
                                    tx.send(StreamEvent::Done(FinishReason::Stop)).await.ok();
                                    emitted_done = true;
                                    break;
                                }
                            }
                        }
                    }
                }

                if emitted_done {
                    break;
                }
            }

            if !emitted_done {
                let tool_calls = flush_pending_tool_calls(&mut pending_tool_calls);
                for call in tool_calls {
                    tx.send(StreamEvent::ToolCall(call)).await.ok();
                }
                tx.send(StreamEvent::Done(FinishReason::Stop)).await.ok();
            }
        });

        Ok(rx)
    }
}
