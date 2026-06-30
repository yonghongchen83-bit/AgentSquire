use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use serde_json::json;
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use tokio::sync::mpsc;

use super::provider::{
    ChatRequest, ChatRole, FinishReason, LlmError, LlmProvider, StreamEvent, ToolCall,
};
use super::thinking::normalize_level;

pub struct OpenAIProvider {
    client: Client,
    api_key: String,
    base_url: String,
    model: String,
    pub verbose: bool,
}

#[derive(Default, Clone)]
struct PendingToolCall {
    id: String,
    name: String,
    arguments: String,
}

fn wire_log_path() -> PathBuf {
    crate::state::config::config_dir().join("provider-wire.log")
}

fn append_wire_log(entry: &str) {
    let path = wire_log_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(
            file,
            "\n===== {} =====\n{}",
            chrono::Utc::now().to_rfc3339(),
            entry
        );
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
        Self {
            client: Client::new(),
            api_key,
            base_url: base_url.unwrap_or_else(|| "https://api.openai.com/v1".into()),
            model,
            verbose: false,
        }
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
                if let Some(ref rc) = m.reasoning_content {
                    msg["reasoning_content"] = json!(rc);
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

        let thinking_level = normalize_level(request.thinking_level.clone());
        if is_deepseek_family(&model) {
            // DeepSeek docs: thinking toggle uses {"thinking":{"type":"enabled|disabled"}}
            // and reasoning_effort supports "high" | "max".
            match thinking_level.as_str() {
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
                append_wire_log(&request_log);
                tx.send(StreamEvent::Log(request_log)).await.ok();
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
                        append_wire_log(&log_line);
                        tx.send(StreamEvent::Log(log_line)).await.ok();
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
                        "<<< RESPONSE {} {}\n{}",
                        status.as_u16(),
                        status.canonical_reason().unwrap_or(""),
                        body_text,
                    );
                    append_wire_log(&response_log);
                    tx.send(StreamEvent::Log(response_log)).await.ok();
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
                            let detail = if trimmed.chars().count() > 500 {
                                truncate_chars(trimmed, 500)
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

            if verbose {
                let opened_log = format!(
                    "<<< RESPONSE {} {} (stream opened)",
                    status.as_u16(),
                    status.canonical_reason().unwrap_or(""),
                );
                append_wire_log(&opened_log);
                tx.send(StreamEvent::Log(opened_log)).await.ok();
            }

            let mut done = false;
            let mut emitted_done = false;
            let mut sse_buffer = String::new();
            let mut response_tool_args: HashMap<String, (String, String)> = HashMap::new();
            let mut pending_tool_calls: HashMap<usize, PendingToolCall> = HashMap::new();
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

                sse_buffer.push_str(&String::from_utf8_lossy(&bytes));

                while let Some(newline_idx) = sse_buffer.find('\n') {
                    let mut line = sse_buffer[..newline_idx].to_string();
                    sse_buffer = sse_buffer[newline_idx + 1..].to_string();
                    line = line.trim().to_string();

                    if line.is_empty() {
                        continue;
                    }

                    if verbose {
                        let raw_line_log = format!("<<< SSE RAW LINE: {:?}", line);
                        append_wire_log(&raw_line_log);
                        tx.send(StreamEvent::Log(raw_line_log)).await.ok();
                    }

                    if line == "data: [DONE]" {
                        if verbose {
                            let done_log = "<<< STREAM DONE sentinel received".to_string();
                            append_wire_log(&done_log);
                            tx.send(StreamEvent::Log(done_log)).await.ok();
                        }
                        if !emitted_done {
                            tx.send(StreamEvent::Done(FinishReason::Stop)).await.ok();
                            emitted_done = true;
                        }
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

                    if val["choices"].is_null() {
                        if let Some(event_type) = val["type"].as_str() {
                            match event_type {
                                "response.output_text.delta" => {
                                    if let Some(text) = val["delta"].as_str() {
                                        tx.send(StreamEvent::Chunk(text.to_string())).await.ok();
                                    }
                                }
                                "response.reasoning.delta"
                                | "response.reasoning_text.delta"
                                | "response.reasoning_summary.delta" => {
                                    if let Some(text) = val["delta"].as_str() {
                                        tx.send(StreamEvent::Thinking(text.to_string())).await.ok();
                                    }
                                }
                                "response.completed" | "message_stop" => {
                                    if verbose {
                                        let event_log = format!("<<< STREAM event={} ", event_type);
                                        append_wire_log(&event_log);
                                        tx.send(StreamEvent::Log(event_log)).await.ok();
                                    }
                                    if !emitted_done {
                                        tx.send(StreamEvent::Done(FinishReason::Stop)).await.ok();
                                        emitted_done = true;
                                    }
                                    done = true;
                                }
                                "response.failed" => {
                                    let msg = val["error"]["message"]
                                        .as_str()
                                        .unwrap_or("Response stream failed");
                                    tx.send(StreamEvent::Error(msg.to_string())).await.ok();
                                    tx.send(StreamEvent::Done(FinishReason::Error)).await.ok();
                                    emitted_done = true;
                                    done = true;
                                }
                                "response.output_item.added" => {
                                    let item = &val["item"];
                                    if item["type"].as_str() == Some("function_call") {
                                        let key = item["id"]
                                            .as_str()
                                            .or_else(|| item["call_id"].as_str())
                                            .unwrap_or("")
                                            .to_string();
                                        let name = item["name"].as_str().unwrap_or("").to_string();
                                        let args =
                                            item["arguments"].as_str().unwrap_or("").to_string();
                                        if !key.is_empty() && !name.is_empty() {
                                            response_tool_args.insert(key, (name, args));
                                        }
                                    }
                                }
                                "response.function_call_arguments.delta" => {
                                    let key = val["item_id"]
                                        .as_str()
                                        .or_else(|| val["call_id"].as_str())
                                        .unwrap_or("")
                                        .to_string();
                                    let delta = val["delta"].as_str().unwrap_or("");
                                    if !key.is_empty() && !delta.is_empty() {
                                        if let Some((_, args)) = response_tool_args.get_mut(&key) {
                                            args.push_str(delta);
                                        }
                                    }
                                }
                                "response.function_call_arguments.done"
                                | "response.output_item.done" => {
                                    let item = if event_type == "response.output_item.done" {
                                        &val["item"]
                                    } else {
                                        &val
                                    };

                                    if event_type == "response.output_item.done"
                                        && item["type"].as_str() != Some("function_call")
                                    {
                                        // Not a function call item; ignore.
                                    } else {
                                        let key = item["item_id"]
                                            .as_str()
                                            .or_else(|| item["id"].as_str())
                                            .or_else(|| item["call_id"].as_str())
                                            .unwrap_or("")
                                            .to_string();
                                        let args_inline =
                                            item["arguments"].as_str().unwrap_or("").to_string();
                                        let name_inline =
                                            item["name"].as_str().unwrap_or("").to_string();

                                        if !key.is_empty() {
                                            let (name, args_text) =
                                                if let Some((stored_name, stored_args)) =
                                                    response_tool_args.remove(&key)
                                                {
                                                    let merged_name = if name_inline.is_empty() {
                                                        stored_name
                                                    } else {
                                                        name_inline
                                                    };
                                                    let merged_args = if args_inline.is_empty() {
                                                        stored_args
                                                    } else {
                                                        args_inline
                                                    };
                                                    (merged_name, merged_args)
                                                } else {
                                                    (name_inline, args_inline)
                                                };

                                            if !name.is_empty() {
                                                if let Ok(args_json) =
                                                    serde_json::from_str::<serde_json::Value>(
                                                        if args_text.trim().is_empty() {
                                                            "{}"
                                                        } else {
                                                            args_text.as_str()
                                                        },
                                                    )
                                                {
                                                    let tool_call = ToolCall {
                                                        id: key,
                                                        name,
                                                        arguments: args_json,
                                                    };
                                                    tx.send(StreamEvent::ToolCall(tool_call))
                                                        .await
                                                        .ok();
                                                }
                                            }
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                        if done {
                            break;
                        }
                        continue;
                    }

                    let choices = match val["choices"].as_array() {
                        Some(c) => c,
                        None => continue,
                    };

                    for choice in choices {
                        let delta = &choice["delta"];

                        let content_text = delta["content"].as_str().unwrap_or("");
                        let reasoning_text = delta["reasoning_content"].as_str().unwrap_or("");

                        if !content_text.is_empty() {
                            tx.send(StreamEvent::Chunk(content_text.to_string()))
                                .await
                                .ok();
                        } else if !reasoning_text.is_empty() {
                            tx.send(StreamEvent::Thinking(reasoning_text.to_string()))
                                .await
                                .ok();
                        }

                        if let Some(tcs) = delta["tool_calls"].as_array() {
                            for tc in tcs {
                                accumulate_classic_tool_call_chunk(&mut pending_tool_calls, tc);
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
                            if matches!(reason, FinishReason::ToolCalls) {
                                for tool_call in flush_pending_tool_calls(&mut pending_tool_calls) {
                                    tx.send(StreamEvent::ToolCall(tool_call)).await.ok();
                                }
                            }
                            if verbose {
                                let reason_label = finish.as_str().unwrap_or("stop");
                                let finish_log =
                                    format!("<<< STREAM finish_reason={} ", reason_label);
                                append_wire_log(&finish_log);
                                tx.send(StreamEvent::Log(finish_log)).await.ok();
                            }
                            tx.send(StreamEvent::Done(reason)).await.ok();
                            emitted_done = true;
                        }
                    }
                }
            }

            // Handle final partial line if stream ended without trailing newline.
            if !done {
                let line = sse_buffer.trim();
                if !line.is_empty() && line != "data: [DONE]" {
                    if let Some(data) = line.strip_prefix("data: ") {
                        if let Ok(val) = serde_json::from_str::<serde_json::Value>(data) {
                            if val["type"].as_str() == Some("response.completed")
                                || val["type"].as_str() == Some("message_stop")
                            {
                                tx.send(StreamEvent::Done(FinishReason::Stop)).await.ok();
                                emitted_done = true;
                            } else if let Some(choices) = val["choices"].as_array() {
                                for choice in choices {
                                    let finish = &choice["finish_reason"];
                                    if !finish.is_null() {
                                        let reason = match finish.as_str() {
                                            Some("stop") => FinishReason::Stop,
                                            Some("length") => FinishReason::Length,
                                            Some("tool_calls") => FinishReason::ToolCalls,
                                            _ => FinishReason::Stop,
                                        };
                                        tx.send(StreamEvent::Done(reason)).await.ok();
                                        emitted_done = true;
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if !emitted_done {
                if verbose {
                    let fallback_log =
                        "<<< STREAM closed without finish_reason, applying fallback=stop"
                            .to_string();
                    append_wire_log(&fallback_log);
                    tx.send(StreamEvent::Log(fallback_log)).await.ok();
                }
                tx.send(StreamEvent::Done(FinishReason::Stop)).await.ok();
            }
        });

        Ok(rx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn accumulates_incremental_classic_tool_calls() {
        let mut pending = HashMap::new();

        let chunks = vec![
            json!({
                "index": 0,
                "id": "call_00_Btv7wxzaKle8PKCgxFTj4416",
                "type": "function",
                "function": { "name": "run_terminal", "arguments": "" }
            }),
            json!({ "index": 0, "function": { "arguments": "{" } }),
            json!({ "index": 0, "function": { "arguments": "\"command\"" } }),
            json!({ "index": 0, "function": { "arguments": ": " } }),
            json!({ "index": 0, "function": { "arguments": "\"pwd && ls -la\"" } }),
            json!({ "index": 0, "function": { "arguments": "}" } }),
        ];

        for chunk in &chunks {
            accumulate_classic_tool_call_chunk(&mut pending, chunk);
        }

        let tool_calls = flush_pending_tool_calls(&mut pending);
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].id, "call_00_Btv7wxzaKle8PKCgxFTj4416");
        assert_eq!(tool_calls[0].name, "run_terminal");
        assert_eq!(tool_calls[0].arguments["command"], "pwd && ls -la");
    }
}
