use super::utils::{derive_session_title_from_message, is_valid_tool_schema};
use super::AppState;
use crate::agent::{self, McpProxyTool, PendingApprovals, ToolDanger, ToolRegistry};
use crate::llm::provider::{ChatMessage, ChatRequest, ChatRole, FinishReason, StreamEvent, ToolCall};
use crate::state::config::McpServerConfig;
use crate::storage::conversation_store::{NewMessage, SessionId};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, State};

fn emit_stream_status(app: &AppHandle, status: &str) {
    let _ = app.emit("stream-status", status.to_string());
}

async fn execute_tool_with_watchdog<F>(
    app: &AppHandle,
    tool_name: &str,
    call_id: &str,
    fut: F,
) -> agent::ToolResult
where
    F: std::future::Future<Output = agent::ToolResult>,
{
    let start = Instant::now();
    let mut warned_blocked = false;
    tokio::pin!(fut);

    loop {
        tokio::select! {
            result = &mut fut => {
                return result;
            }
            _ = tokio::time::sleep(Duration::from_secs(5)) => {
                let elapsed = start.elapsed().as_secs();
                emit_stream_status(
                    app,
                    &format!("Tool {} still running ({}s)", tool_name, elapsed),
                );

                if !warned_blocked && elapsed >= 20 {
                    warned_blocked = true;
                    let hint = super::utils::blocked_hint_for_tool(tool_name);
                    let _ = app.emit(
                        "output:append",
                        serde_json::json!({
                            "source": "chat",
                            "line": format!(
                                "WARNING: Tool execution appears blocked. tool={}, call_id={}, elapsed={}s, hint={}",
                                tool_name, call_id, elapsed, hint
                            ),
                            "timestamp": chrono::Utc::now().to_rfc3339(),
                        }),
                    );
                }
            }
        }
    }
}

async fn await_approval_with_watchdog(
    app: &AppHandle,
    tool_name: &str,
    rx: tokio::sync::oneshot::Receiver<bool>,
) -> bool {
    let start = Instant::now();
    tokio::pin!(rx);

    loop {
        tokio::select! {
            decision = &mut rx => {
                return matches!(decision, Ok(true));
            }
            _ = tokio::time::sleep(Duration::from_secs(10)) => {
                let elapsed = start.elapsed().as_secs();
                emit_stream_status(
                    app,
                    &format!("Waiting for approval: {} ({}s)", tool_name, elapsed),
                );
                if elapsed >= 30 {
                    let _ = app.emit(
                        "output:append",
                        serde_json::json!({
                            "source": "chat",
                            "line": format!(
                                "INFO: Tool approval still pending. tool={}, elapsed={}s. User action is required.",
                                tool_name, elapsed
                            ),
                            "timestamp": chrono::Utc::now().to_rfc3339(),
                        }),
                    );
                }
            }
        }
    }
}

pub async fn send_message_impl(
    app: AppHandle,
    state: State<'_, AppState>,
    pending_state: State<'_, PendingApprovals>,
    session_id: String,
    content: String,
    provider_name: Option<String>,
    model: Option<String>,
    thinking_level: Option<String>,
) -> Result<(), String> {
    let sid =
        SessionId::parse_str(&session_id).map_err(|e| format!("Invalid session ID: {}", e))?;

    state
        .store
        .append_message(NewMessage {
            session_id: sid,
            role: crate::storage::conversation_store::MessageRole::User,
            content: content.clone(),
            thinking_content: None,
        })
        .await
        .map_err(|e| e.to_string())?;

    let session = state
        .store
        .get_session(sid)
        .await
        .map_err(|e| e.to_string())?;

    if session.session.title.trim().eq_ignore_ascii_case("new chat") {
        if let Some(generated_title) = derive_session_title_from_message(&content) {
            let _ = state.store.update_session_title(sid, generated_title).await;
        }
    }

    let (provider_arc, selected_model, selected_provider_name) = {
        let reg = state.registry.read().map_err(|e| e.to_string())?;
        let name = provider_name
            .clone()
            .or_else(|| reg.default_name().map(|s| s.to_string()))
            .ok_or_else(|| "No default LLM provider configured".to_string())?;
        let entry = reg
            .get(&name)
            .ok_or_else(|| format!("Provider '{}' not found", name))?;
        let sm = model.clone().unwrap_or_else(|| entry.default_model.clone());
        (entry.provider.clone(), sm, name)
    };

    let (enabled_mcp_servers, verbose_logging): (Vec<McpServerConfig>, bool) = {
        let cfg = state.config.read().map_err(|e| e.to_string())?;
        (
            cfg.mcp_servers.iter().filter(|s| s.enabled).cloned().collect(),
            cfg.verbose_logging,
        )
    };

    let store = state.store.clone();
    let app_clone = app.clone();
    let pending = pending_state.pending.clone();
    let stream_tasks = state.stream_tasks.clone();
    let session_key = sid.to_string();

    if let Some(existing) = stream_tasks.lock().await.remove(&session_key) {
        existing.abort();
    }

    let stream_tasks_cleanup = stream_tasks.clone();
    let session_key_cleanup = session_key.clone();
    let handle = tokio::spawn(async move {
        let run = async {
            emit_stream_status(&app_clone, "Preparing tools...");
            let mut tool_registry = ToolRegistry::new();
            let mut used_names: HashSet<String> = tool_registry
                .definitions()
                .into_iter()
                .map(|d| d.name)
                .collect();

            for server in &enabled_mcp_servers {
                match crate::mcp::discover_tools(server.clone()).await {
                    Ok(tools) => {
                        for tool in tools {
                            let server_id = server
                                .id
                                .chars()
                                .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
                                .collect::<String>();
                            let remote_tool_name = tool.name.clone();
                            let tool_id = remote_tool_name
                                .chars()
                                .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
                                .collect::<String>();

                            let mut local_name = format!("mcp_{}_{}", server_id, tool_id);
                            let mut i = 2;
                            while used_names.contains(&local_name) {
                                local_name = format!("mcp_{}_{}_{}", server_id, tool_id, i);
                                i += 1;
                            }
                            used_names.insert(local_name.clone());

                            let local_description = format!(
                                "MCP tool '{}' from server '{}': {}",
                                remote_tool_name, server.name, tool.description
                            );

                            if !is_valid_tool_schema(&tool.input_schema) {
                                let _ = app_clone.emit(
                                    "output:append",
                                    serde_json::json!({
                                        "source": "chat",
                                        "line": format!(
                                            "WARNING: Skipping MCP tool '{}' from server '{}' because its input schema is not a plain object",
                                            remote_tool_name, server.name
                                        ),
                                        "timestamp": chrono::Utc::now().to_rfc3339(),
                                    }),
                                );
                                continue;
                            }

                            tool_registry.register(Box::new(McpProxyTool {
                                local_name: local_name.clone(),
                                local_description,
                                schema: tool.input_schema.clone(),
                                server: server.clone(),
                                remote_name: remote_tool_name.clone(),
                            }));
                        }
                    }
                    Err(e) => {
                        let _ = app_clone.emit(
                            "output:append",
                            serde_json::json!({
                                "source": "chat",
                                "line": format!(
                                    "WARNING: MCP discovery failed for server '{}': {}",
                                    server.name, e
                                ),
                                "timestamp": chrono::Utc::now().to_rfc3339(),
                            }),
                        );
                    }
                }
            }

            let tool_registry = Arc::new(tool_registry);
            let tool_defs = tool_registry.definitions();

            let mut messages: Vec<ChatMessage> = session
                .messages
                .iter()
                .map(|m| ChatMessage {
                    role: match m.role {
                        crate::storage::conversation_store::MessageRole::User => ChatRole::User,
                        crate::storage::conversation_store::MessageRole::Assistant => {
                            ChatRole::Assistant
                        }
                        crate::storage::conversation_store::MessageRole::System => ChatRole::System,
                    },
                    content: m.content.clone(),
                    tool_call_id: None,
                    tool_calls: None,
                    reasoning_content: m.thinking_content.clone(),
                })
                .collect();

            loop {
                emit_stream_status(&app_clone, "Contacting model...");
                let request = ChatRequest {
                    model: selected_model.clone(),
                    messages: messages.clone(),
                    tools: tool_defs.clone(),
                    thinking_level: thinking_level.clone(),
                    temperature: None,
                    max_tokens: None,
                };

                if verbose_logging {
                    let request_pretty = serde_json::to_string_pretty(&request).unwrap_or_default();
                    let _ = app_clone.emit(
                        "output:append",
                        serde_json::json!({
                            "source": "chat",
                            "line": format!("[orchestrator] >>> CHAT REQUEST\n{}", request_pretty),
                            "timestamp": chrono::Utc::now().to_rfc3339(),
                        }),
                    );
                }

                let mut stream = match provider_arc.chat(request).await {
                    Ok(s) => s,
                    Err(e) => {
                        emit_stream_status(&app_clone, "Model request failed");
                        let _ = app_clone.emit("stream-error", e.to_string());
                        return;
                    }
                };

                let mut full_response = String::new();
                let mut full_thinking = String::new();
                let mut tool_calls: Vec<ToolCall> = Vec::new();
                let mut finish_reason: Option<FinishReason> = None;
                let mut channel_closed_cleanly = false;

                while let Some(event) = stream.recv().await {
                    match event {
                        StreamEvent::Chunk(text) => {
                            full_response.push_str(&text);
                            let _ = app_clone.emit("stream-chunk", text);
                        }
                        StreamEvent::Thinking(text) => {
                            full_thinking.push_str(&text);
                            let _ = app_clone.emit("stream-thinking", text);
                        }
                        StreamEvent::ToolCall(tc) => {
                            tool_calls.push(tc.clone());
                            let _ = app_clone.emit("stream-tool-call", tc);
                        }
                        StreamEvent::Log(msg) => {
                            let _ = app_clone.emit(
                                "output:append",
                                serde_json::json!({
                                    "source": "chat",
                                    "line": msg,
                                    "timestamp": chrono::Utc::now().to_rfc3339(),
                                }),
                            );
                        }
                        StreamEvent::Done(reason) => {
                            emit_stream_status(&app_clone, "Model response received");
                            finish_reason = Some(reason);
                            break;
                        }
                        StreamEvent::Error(err) => {
                            emit_stream_status(&app_clone, "Model stream error");
                            let _ = app_clone.emit("stream-error", err);
                            return;
                        }
                    }
                }

                if finish_reason.is_none() {
                    channel_closed_cleanly = true;
                }

                let reason = match finish_reason {
                    Some(r) => r,
                    None => {
                        if channel_closed_cleanly
                            && (full_response.trim().is_empty() && tool_calls.is_empty())
                        {
                            if verbose_logging {
                                let _ = app_clone.emit(
                                    "output:append",
                                    serde_json::json!({
                                        "source": "chat",
                                        "line": format!(
                                            "ERROR: Provider stream channel closed with no output and no finish reason. provider={}, model={}. This is a provider protocol violation - check SSE wire log.",
                                            selected_provider_name, selected_model
                                        ),
                                        "timestamp": chrono::Utc::now().to_rfc3339(),
                                    }),
                                );
                            }
                            let _ = app_clone.emit(
                                "stream-error",
                                format!(
                                    "Provider closed stream without any output [provider={}, model={}]",
                                    selected_provider_name, selected_model
                                ),
                            );
                            return;
                        }

                        let inferred_reason = if !tool_calls.is_empty() {
                            FinishReason::ToolCalls
                        } else if !full_response.trim().is_empty() {
                            FinishReason::Stop
                        } else {
                            FinishReason::Error
                        };

                        if matches!(inferred_reason, FinishReason::Error) {
                            emit_stream_status(&app_clone, "Stream ended without usable output");
                            let _ = app_clone.emit(
                                "stream-error",
                                format!(
                                    "Stream ended without finish reason and no usable output [provider={}, model={}]",
                                    selected_provider_name, selected_model
                                ),
                            );
                            return;
                        }

                        let _ = app_clone.emit(
                            "output:append",
                            serde_json::json!({
                                "source": "chat",
                                "line": format!(
                                    "WARNING: Stream ended without finish reason; applying fallback completion path. provider={}, model={}, inferred={:?}",
                                    selected_provider_name, selected_model, inferred_reason
                                ),
                                "timestamp": chrono::Utc::now().to_rfc3339(),
                            }),
                        );

                        inferred_reason
                    }
                };

                match reason {
                    FinishReason::ToolCalls => {
                        emit_stream_status(&app_clone, "Model requested tool execution");
                        if verbose_logging {
                            let _ = app_clone.emit(
                                "output:append",
                                serde_json::json!({
                                    "source": "chat",
                                    "line": format!(
                                        "[orchestrator] <<< CHAT RESPONSE finish=tool_calls text_bytes={} tool_calls={}",
                                        full_response.len(),
                                        tool_calls.len()
                                    ),
                                    "timestamp": chrono::Utc::now().to_rfc3339(),
                                }),
                            );
                        }
                        if !full_response.is_empty() {
                            let _ = app_clone.emit("stream-chunk", "\n\n");
                        }

                        for tc in &tool_calls {
                            emit_stream_status(&app_clone, &format!("Invoking tool {}", tc.name));
                            let tool = tool_registry.get(&tc.name);
                            let result = if let Some(tool) = tool {
                                if tool.danger() == ToolDanger::Destructive {
                                    let (tx, rx) = tokio::sync::oneshot::channel();
                                    {
                                        let mut p = pending.lock().await;
                                        p.insert(tc.id.clone(), tx);
                                    }

                                    let approval_event = serde_json::json!({
                                        "call_id": tc.id,
                                        "tool_name": tc.name,
                                        "arguments": tc.arguments,
                                    });
                                    let _ = app_clone
                                        .emit("stream-tool-pending", approval_event.to_string());
                                    emit_stream_status(
                                        &app_clone,
                                        &format!("Waiting for approval: {}", tc.name),
                                    );

                                    match await_approval_with_watchdog(&app_clone, &tc.name, rx)
                                        .await
                                    {
                                        true => {
                                            emit_stream_status(
                                                &app_clone,
                                                &format!("Approval granted, running {}", tc.name),
                                            );
                                            let _ = app_clone.emit(
                                                "stream-chunk",
                                                format!("[Executing {}...]\n", tc.name),
                                            );
                                            execute_tool_with_watchdog(
                                                &app_clone,
                                                &tc.name,
                                                &tc.id,
                                                tool.execute(&tc.id, tc.arguments.clone()),
                                            )
                                            .await
                                        }
                                        _ => {
                                            emit_stream_status(
                                                &app_clone,
                                                &format!("Approval denied: {}", tc.name),
                                            );
                                            agent::ToolResult {
                                                call_id: tc.id.clone(),
                                                output: format!(
                                                    "Tool call '{}' was rejected by user",
                                                    tc.name
                                                ),
                                                is_error: true,
                                            }
                                        }
                                    }
                                } else {
                                    execute_tool_with_watchdog(
                                        &app_clone,
                                        &tc.name,
                                        &tc.id,
                                        tool.execute(&tc.id, tc.arguments.clone()),
                                    )
                                    .await
                                }
                            } else {
                                agent::ToolResult {
                                    call_id: tc.id.clone(),
                                    output: format!("Unknown tool: {}", tc.name),
                                    is_error: true,
                                }
                            };

                            let _ = app_clone.emit("stream-tool-result", &result);
                            if result.is_error {
                                emit_stream_status(&app_clone, &format!("Tool {} failed", tc.name));
                            } else {
                                emit_stream_status(&app_clone, &format!("Tool {} completed", tc.name));
                            }

                            // reasoning_content only on the first assistant message in this turn
                            let reasoning = if !full_thinking.is_empty() {
                                Some(std::mem::take(&mut full_thinking))
                            } else {
                                None
                            };

                            messages.push(ChatMessage {
                                role: ChatRole::Assistant,
                                content: String::new(),
                                tool_call_id: Some(tc.id.clone()),
                                tool_calls: Some(vec![ToolCall {
                                    id: tc.id.clone(),
                                    name: tc.name.clone(),
                                    arguments: tc.arguments.clone(),
                                }]),
                                reasoning_content: reasoning,
                            });

                            messages.push(ChatMessage {
                                role: ChatRole::Tool,
                                content: result.output.clone(),
                                tool_call_id: Some(tc.id.clone()),
                                tool_calls: None,
                                reasoning_content: None,
                            });
                        }

                        continue;
                    }
                    FinishReason::Stop | FinishReason::Length => {
                        emit_stream_status(&app_clone, "Completed");
                        let content = std::mem::take(&mut full_response);
                        if verbose_logging {
                            let _ = app_clone.emit(
                                "output:append",
                                serde_json::json!({
                                    "source": "chat",
                                    "line": format!("[orchestrator] <<< CHAT RESPONSE RAW\n{}", content),
                                    "timestamp": chrono::Utc::now().to_rfc3339(),
                                }),
                            );
                        }
                        if !content.is_empty() {
                            let thinking = if !full_thinking.is_empty() {
                                Some(std::mem::take(&mut full_thinking))
                            } else {
                                None
                            };
                            let _ = store
                                .append_message(NewMessage {
                                    session_id: sid,
                                    role: crate::storage::conversation_store::MessageRole::Assistant,
                                    content,
                                    thinking_content: thinking,
                                })
                                .await;
                        }
                        let _ = app_clone.emit("stream-done", "");
                        return;
                    }
                    FinishReason::Error => {
                        emit_stream_status(&app_clone, "LLM returned an error");
                        let _ = app_clone.emit("stream-error", "LLM returned an error");
                        return;
                    }
                }
            }
        };

        run.await;
        let mut tasks = stream_tasks_cleanup.lock().await;
        tasks.remove(&session_key_cleanup);
    });

    stream_tasks.lock().await.insert(session_key, handle);

    Ok(())
}
