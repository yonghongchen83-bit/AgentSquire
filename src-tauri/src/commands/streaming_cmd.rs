use super::utils::{derive_session_title_from_message, is_valid_tool_schema};
use super::AppState;
use crate::agent::context_adapter::{ContextManagerAdapter, LegacyContextAdapter, TurnOutcome};
use crate::agent::squire::{SquireContextAdapter, SquireExploreTool, SquireInvokeTool, SquireTokenToDetailTool};
use crate::agent::{self, McpProxyTool, PendingApprovals, PendingAskUserQuestions, ToolDanger, ToolRegistry};
use crate::llm::provider::{ChatMessage, ChatRole, ChatRequest, FinishReason, StreamEvent, ToolCall};
use crate::state::config::{McpServerConfig, SquirePrefetchConfig};
use crate::storage::conversation_store::{ContextMode, MessageRole, NewMessage, SessionId};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, State};

fn emit_stream_status(app: &AppHandle, status: &str) {
    let _ = app.emit("stream-status", status.to_string());
}

/// sa-4: whether raw per-token model output should be forwarded live to the
/// `stream-chunk` UI channel as it arrives. Legacy mode's content is always
/// display-ready prose, so it streams live as before. Squire mode's raw
/// content is protocol JSON containing unexpanded `§!`/`§^` sigils until
/// `SquireContextAdapter::finalize_turn` parses and expands it — forwarding
/// it live would violate the spec's display-boundary guarantee ("no protocol
/// artefacts are ever visible to the user", `context_squire_spec_v2.md` §14).
/// Extracted as a small pure function so the mode-gating policy itself is
/// unit-testable independent of the surrounding Tauri/streaming orchestration
/// (which has no test harness today — see `commands::streaming_cmd` has no
/// `mod tests` because of its `AppHandle`/`State` dependencies).
fn should_stream_live_chunks(context_mode: ContextMode) -> bool {
    !matches!(context_mode, ContextMode::Squire)
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

/// sa-5: waits for the user's answer to a paused ask_user question, with the
/// same periodic-nudge watchdog UX as `await_approval_with_watchdog` (see
/// `ask-user-loop/decisions.md` — a stuck ask_user question should look and
/// feel like a stuck approval prompt, not a silently different pattern).
/// Returns `None` if the sender was dropped without ever answering (e.g. the
/// turn task itself is being aborted concurrently — see the abandonment
/// handling note in decisions.md); callers should treat that the same as an
/// aborted turn, not retry.
async fn await_answer_with_watchdog(
    app: &AppHandle,
    rx: tokio::sync::oneshot::Receiver<String>,
) -> Option<String> {
    let start = Instant::now();
    tokio::pin!(rx);

    loop {
        tokio::select! {
            answer = &mut rx => {
                return answer.ok();
            }
            _ = tokio::time::sleep(Duration::from_secs(10)) => {
                let elapsed = start.elapsed().as_secs();
                emit_stream_status(
                    app,
                    &format!("Waiting for your answer... ({}s)", elapsed),
                );
                if elapsed >= 30 {
                    let _ = app.emit(
                        "output:append",
                        serde_json::json!({
                            "source": "chat",
                            "line": format!(
                                "INFO: Squire ask_user question still pending after {}s. User action is required.",
                                elapsed
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
    pending_ask_user_state: State<'_, PendingAskUserQuestions>,
    session_id: String,
    content: String,
    provider_name: Option<String>,
    model: Option<String>,
    thinking_level: Option<String>,
) -> Result<(), String> {
    let sid =
        SessionId::parse_str(&session_id).map_err(|e| format!("Invalid session ID: {}", e))?;

    let store_arc = state.store.read().map_err(|e| e.to_string())?.clone();
    store_arc
        .append_message(NewMessage {
            session_id: sid,
            role: crate::storage::conversation_store::MessageRole::User,
            content: content.clone(),
            thinking_content: None,
        })
        .await
        .map_err(|e| e.to_string())?;

    let session = store_arc
        .get_session(sid)
        .await
        .map_err(|e| e.to_string())?;

    if session.session.title.trim().eq_ignore_ascii_case("new chat") {
        if let Some(generated_title) = derive_session_title_from_message(&content) {
            let _ = store_arc
                .update_session_title(sid, generated_title)
                .await;
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

    let (enabled_mcp_servers, verbose_logging, squire_prefetch, disabled_tools): (
        Vec<McpServerConfig>,
        bool,
        SquirePrefetchConfig,
        Vec<String>,
    ) = {
        let cfg = state.config.read().map_err(|e| e.to_string())?;
        (
            cfg.mcp_servers.iter().filter(|s| s.enabled).cloned().collect(),
            cfg.verbose_logging,
            cfg.squire_prefetch.clone(),
            cfg.disabled_tools.clone(),
        )
    };

    let project_path = state
        .project_path
        .read()
        .map(|p| p.clone())
        .unwrap_or_default();

    let store = state.store.read().map_err(|e| e.to_string())?.clone();
    let squire_store = state.squire_store.read().map_err(|e| e.to_string())?.clone();
    let app_clone = app.clone();
    let pending = pending_state.pending.clone();
    let pending_ask_user = pending_ask_user_state.pending.clone();
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
            if !disabled_tools.iter().any(|t| t == "subagent") {
                tool_registry.register(Box::new(agent::SubagentTool {
                    app_handle: app_clone.clone(),
                    store: store.clone(),
                    enabled_mcp_servers: enabled_mcp_servers.clone(),
                    provider: provider_arc.clone(),
                    model: selected_model.clone(),
                    provider_name: selected_provider_name.clone(),
                    verbose_logging,
                    project_path: project_path.clone(),
                }));
            }
            let mut used_names: HashSet<String> = tool_registry
                .definitions()
                .into_iter()
                .map(|d| d.name)
                .collect();
            // token-detail-endpoint: side-channel map from a tool's registry
            // (local) name to enough metadata to re-dispatch it purely from
            // stored data later, even if its server isn't live in some
            // future turn. Populated only for MCP-sourced tools below —
            // ToolDefinition itself erases this origin once registered, so
            // it must be captured here, at the one point origin is still
            // known. See `agent::squire::ingest_tool_registry`'s doc comment
            // and `token-detail-endpoint/decisions.md`.
            let mut tool_endpoints: HashMap<String, agent::squire::ToolEndpoint> = HashMap::new();

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

                            tool_endpoints.insert(
                                local_name.clone(),
                                agent::squire::ToolEndpoint::Mcp {
                                    server: server.clone().into(),
                                    remote_name: remote_tool_name.clone(),
                                },
                            );

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

            // ss-9: ingest the full, just-assembled tool registry (local
            // built-ins + MCP-discovered tools) into the Squire store as
            // `tool`-typed tokens, so `explore(resource_type="tool_skill")`
            // has real rows to find. Runs every turn, both context modes.
            agent::squire::ingest_tool_registry(&tool_registry, squire_store.as_ref(), &tool_endpoints).await;

            // Capture tool_defs for build_turn_input BEFORE registering Squire
            // tools — build_turn_input already prepends built_in_tool_definitions()
            // (explore, token_to_detail), so including them in base_tools would
            // create duplicates.
            let base_tool_defs = tool_registry.definitions();

            // Squire mode (Q5): register Squire-specific tools into the main
            // registry alongside real tools. The model discovers tools via
            // explore() and calls them directly by name — no invoke proxy.
            if session.session.context_mode == ContextMode::Squire {
                tool_registry.register(Box::new(SquireExploreTool {
                    store: squire_store.clone(),
                    tool_defs: base_tool_defs.clone(),
                    session_id: session.session.id,
                }));
                tool_registry.register(Box::new(SquireTokenToDetailTool {
                    store: squire_store.clone(),
                }));
                // Replace the default JSON-file-backed TodoTreeTool with a
                // Squire-store-backed token-driven version.
                tool_registry.register(Box::new(agent::TodoTreeTool::for_store(
                    squire_store.clone(),
                    session.session.id,
                )));
                // Register the decision tree tool (Squire store only).
                tool_registry.register(Box::new(agent::DecisionTreeTool::new(
                    squire_store.clone(),
                    session.session.id,
                )));
                // Register the invoke proxy tool. The dispatch loop rewrites
                // "invoke" calls to the real tool before execution, so the
                // frontend sees the actual tool name (not "invoke").
                tool_registry.register(Box::new(SquireInvokeTool));
            }

            let tool_registry = Arc::new(tool_registry);
            let tool_defs = base_tool_defs;

            let mut adapter: Box<dyn ContextManagerAdapter> = match session.session.context_mode {
                ContextMode::Legacy => Box::new(LegacyContextAdapter),
                ContextMode::Squire => Box::new(SquireContextAdapter::new_with_prefetch(
                    squire_store.clone(),
                    squire_prefetch.clone(),
                )),
            };

            // sa-4: gate the live `stream-chunk` UI channel by context mode —
            // see `should_stream_live_chunks` for the full rationale.
            let stream_live_chunks = should_stream_live_chunks(session.session.context_mode);

            // Single dispatch registry for both modes. In Squire mode the
            // registry includes explore + token_to_detail alongside real tools.
            let dispatch_registry: Arc<ToolRegistry> = tool_registry.clone();

            let turn_input = match adapter.build_turn_input(&session, &tool_defs).await {
                Ok(ti) => ti,
                Err(e) => {
                    emit_stream_status(&app_clone, "Failed to build turn context");
                    let _ = app_clone.emit("stream-error", e);
                    return;
                }
            };
            let mut messages: Vec<ChatMessage> = turn_input.messages;
            let turn_tools = turn_input.tools;

            loop {
                emit_stream_status(&app_clone, "Contacting model...");
                let request = ChatRequest {
                    model: selected_model.clone(),
                    messages: messages.clone(),
                    tools: turn_tools.clone(),
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
                            // sa-4: suppress the live per-token display event
                            // in Squire mode — `full_response` still
                            // accumulates normally for `finalize_turn` to
                            // parse/expand once the turn closes. Legacy mode
                            // streams as before.
                            if stream_live_chunks {
                                let _ = app_clone.emit("stream-chunk", text);
                            }
                        }
                        StreamEvent::Thinking(text) => {
                            full_thinking.push_str(&text);
                            let _ = app_clone.emit("stream-thinking", text);
                        }
                        StreamEvent::ToolCall(mut tc) => {
                            // In Squire mode, rewrite "invoke" tool calls so
                            // the frontend sees the real tool name (not "invoke").
                            // The AI calls invoke(token_id, params) but the UI
                            // should render it as if the tool was called directly.
                            if session.session.context_mode == ContextMode::Squire
                                && tc.name == "invoke"
                            {
                                if let Some(real_name) = tc
                                    .arguments
                                    .get("token_id")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string())
                                {
                                    let real_args = tc
                                        .arguments
                                        .get("params")
                                        .cloned()
                                        .unwrap_or(serde_json::json!({}));
                                    tc.name = real_name;
                                    tc.arguments = real_args;
                                }
                            }
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
                        if !full_response.is_empty() && stream_live_chunks {
                            let _ = app_clone.emit("stream-chunk", "\n\n");
                        }

                        // Persist the text+thinking assistant message before tool calls
                        // so the conversation history includes reasoning_content for DeepSeek.
                        // Per the OpenAI API spec, tool_calls belong ON the assistant message
                        // itself — not a separate message. DeepSeek validates that
                        // reasoning_content is passed back on the same message that carried it
                        // originally, so merge everything into ONE assistant message.
                        let msg_content = std::mem::take(&mut full_response);
                        let msg_thinking = if !full_thinking.is_empty() {
                            Some(std::mem::take(&mut full_thinking))
                        } else {
                            None
                        };
                        if !msg_content.is_empty() || msg_thinking.is_some() || !tool_calls.is_empty() {
                            messages.push(ChatMessage {
                                role: ChatRole::Assistant,
                                content: msg_content.clone(),
                                tool_call_id: None,
                                tool_calls: Some(tool_calls.clone()),
                                reasoning_content: msg_thinking.clone(),
                            });
                            // Save to DB immediately so reasoning_content persists across turns
                            let _ = store
                                .append_message(NewMessage {
                                    session_id: sid,
                                    role: MessageRole::Assistant,
                                    content: msg_content,
                                    thinking_content: msg_thinking,
                                })
                                .await;
                        }

                        for tc in &tool_calls {
                            emit_stream_status(&app_clone, &format!("Invoking tool {}", tc.name));
                            let tool = dispatch_registry.get(&tc.name);
                            let result = if let Some(tool) = tool {
                                if tool.danger() == ToolDanger::Destructive {
                                    let (tx, rx) = tokio::sync::oneshot::channel();
                                    {
                                        let mut p = pending.lock().await;
                                        p.insert(tc.id.clone(), tx);
                                    }

                                    let mut approval_event = serde_json::json!({
                                        "call_id": tc.id,
                                        "tool_name": tc.name,
                                        "arguments": tc.arguments,
                                    });

                                    // Enrich with command analysis for terminal tools
                                    if tc.name == "run_terminal" {
                                        let cmd = tc.arguments.get("command").and_then(|v| v.as_str()).unwrap_or("");
                                        let args: Vec<String> = tc.arguments
                                            .get("args")
                                            .and_then(|v| v.as_array())
                                            .map(|a| {
                                                a.iter().filter_map(|v| v.as_str().map(String::from)).collect()
                                            })
                                            .unwrap_or_default();
                                        let workdir = tc.arguments.get("workdir").and_then(|v| v.as_str());

                                        if !project_path.is_empty() {
                                            let analysis = crate::commands::utils::analyze_terminal_command(
                                                cmd, &args, workdir, &project_path,
                                            );
                                            approval_event["commandAnalysis"] =
                                                serde_json::to_value(&analysis).unwrap_or_default();
                                        }
                                    }
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
                                            if stream_live_chunks {
                                                let _ = app_clone.emit(
                                                    "stream-chunk",
                                                    format!("[Executing {}...]\n", tc.name),
                                                );
                                            }
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

                            // reasoning_content is already on the assistant message pushed
                            // above — not needed on individual tool result messages.
                            if let Err(e) = adapter
                                .handle_tool_loop_step(tc, &result, &mut messages)
                                .await
                            {
                                emit_stream_status(&app_clone, "Failed to update turn context");
                                let _ = app_clone.emit("stream-error", e);
                                return;
                            }
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
                        let thinking = if !full_thinking.is_empty() {
                            Some(std::mem::take(&mut full_thinking))
                        } else {
                            None
                        };
                        // sa-5: kept so the AskUser branch below can push the
                        // model's own question-bearing response into message
                        // history — `finalize_turn` takes `content` by value.
                        let raw_assistant_content = content.clone();
                        match adapter
                            .finalize_turn(sid, content, thinking, &mut messages, store.as_ref())
                            .await
                        {
                            Ok(TurnOutcome::Done) => {
                                let _ = app_clone.emit("stream-done", "");
                                return;
                            }
                            Ok(TurnOutcome::Retry) => {
                                emit_stream_status(&app_clone, "Response rejected, retrying...");
                                continue;
                            }
                            Ok(TurnOutcome::AskUser { question }) => {
                                // Spec §8.2/§9.3: surface the question to the
                                // user, collect an answer, append both to the
                                // turn's message history, and resume
                                // generation. See ask-user-loop/decisions.md
                                // for the full pause/resume design (mirrors
                                // the existing destructive-tool-call approval
                                // flow).
                                let question_id = uuid::Uuid::new_v4().to_string();
                                let (tx, rx) = tokio::sync::oneshot::channel();
                                {
                                    let mut p = pending_ask_user.lock().await;
                                    p.insert(question_id.clone(), tx);
                                }

                                let _ = app_clone.emit(
                                    "stream-ask-user-pending",
                                    serde_json::json!({
                                        "question_id": question_id,
                                        "session_id": sid,
                                        "question": question,
                                    })
                                    .to_string(),
                                );
                                emit_stream_status(&app_clone, "Waiting for your answer...");

                                match await_answer_with_watchdog(&app_clone, rx).await {
                                    Some(answer) => {
                                        // Preserve the model's own
                                        // question-bearing response in
                                        // history (mirrors `reject` keeping
                                        // the rejected response before
                                        // appending the rejection payload),
                                        // then feed the answer back in the
                                        // same structured-JSON idiom the
                                        // model's system prompt already
                                        // expects for turn continuations.
                                        messages.push(ChatMessage {
                                            role: ChatRole::Assistant,
                                            content: raw_assistant_content,
                                            tool_call_id: None,
                                            tool_calls: None,
                                            reasoning_content: None,
                                        });
                                        messages.push(ChatMessage {
                                            role: ChatRole::User,
                                            content: serde_json::json!({ "user_answer": answer })
                                                .to_string(),
                                            tool_call_id: None,
                                            tool_calls: None,
                                            reasoning_content: None,
                                        });
                                        emit_stream_status(&app_clone, "Answer received, resuming...");
                                        continue;
                                    }
                                    None => {
                                        // Sender dropped without answering
                                        // (turn task being aborted, or the
                                        // pending-question entry was cleared
                                        // some other way) — end the turn
                                        // quietly rather than erroring; this
                                        // is the same shape as an aborted
                                        // stream, not a failure to surface.
                                        emit_stream_status(&app_clone, "Stopped waiting for answer");
                                        return;
                                    }
                                }
                            }
                            Ok(TurnOutcome::Failed { reason, failed_content }) => {
                                // The real Q6 UX lives in `SquireContextAdapter::
                                // reject_and_record`: it already persisted a
                                // visible chat message (reason + the failed
                                // response) and a structured diagnostic record
                                // before returning this outcome. Orchestration's
                                // job here is just to end the turn and let the
                                // frontend's existing stream-error handling
                                // (which reloads the conversation from the
                                // store) pick up that persisted message —
                                // no separate diagnostic re-log needed.
                                emit_stream_status(&app_clone, "Squire compliance check failed");
                                if verbose_logging {
                                    let _ = app_clone.emit(
                                        "output:append",
                                        serde_json::json!({
                                            "source": "chat",
                                            "line": format!(
                                                "[squire] compliance failure after exhausting retries. reason={}\nfinal response:\n{}",
                                                reason, failed_content
                                            ),
                                            "timestamp": chrono::Utc::now().to_rfc3339(),
                                        }),
                                    );
                                }
                                let _ = app_clone.emit(
                                    "stream-error",
                                    format!(
                                        "Squire compliance failure after exhausting retries: {}",
                                        reason
                                    ),
                                );
                                return;
                            }
                            Err(e) => {
                                emit_stream_status(&app_clone, "Failed to finalize turn");
                                let _ = app_clone.emit("stream-error", e);
                                return;
                            }
                        }
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

#[cfg(test)]
#[path = "streaming_cmd_test.rs"]
mod tests;
