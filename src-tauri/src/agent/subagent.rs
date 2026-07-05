use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use tauri::Emitter;
use tauri::Manager;

use super::{McpProxyTool, TodoTreeTool, Tool, ToolDanger, ToolRegistry, ToolResult};

/// ── Subagent Tool ──
///
/// Spawns a child LLM conversation (subagent) that runs independently with
/// access to the full tool registry. The subagent's progress is streamed to
/// the frontend via Tauri events, and the result is returned when complete.
/// Used as a regular tool in Legacy mode; discoverable through `invoke` in
/// Squire mode.
pub struct SubagentTool {
    pub app_handle: tauri::AppHandle,
    pub store: Arc<dyn crate::storage::conversation_store::ConversationStore>,
    pub enabled_mcp_servers: Vec<crate::state::config::McpServerConfig>,
    pub provider: Arc<dyn crate::llm::provider::LlmProvider>,
    pub model: String,
    pub provider_name: String,
    pub verbose_logging: bool,
    pub project_path: String,
}

#[async_trait]
impl Tool for SubagentTool {
    fn name(&self) -> &str {
        "subagent"
    }

    fn description(&self) -> &str {
        "Spawn a sub-agent to work on a task independently. The sub-agent gets full tool access and reports back when done."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "task": {
                    "type": "string",
                    "description": "The task to delegate to the sub-agent"
                }
            },
            "required": ["task"]
        })
    }

    fn danger(&self) -> ToolDanger {
        ToolDanger::Safe
    }

    async fn execute(&self, call_id: &str, args: Value) -> ToolResult {
        let task = match args.get("task").and_then(|v| v.as_str()) {
            Some(t) => t,
            None => {
                return ToolResult {
                    call_id: call_id.to_string(),
                    output: "Missing required argument: task".to_string(),
                    is_error: true,
                }
            }
        };

        let session_title = if task.len() > 60 {
            format!("Subagent: {}...", &task[..57])
        } else {
            format!("Subagent: {}", task)
        };

        // Create a new hidden session for the subagent
        let new_session = match self
            .store
            .create_session(
                crate::storage::conversation_store::NewSession {
                    title: session_title,
                    context_mode: Some(crate::storage::conversation_store::ContextMode::Legacy),
                },
            )
            .await
        {
            Ok(s) => s,
            Err(e) => {
                return ToolResult {
                    call_id: call_id.to_string(),
                    output: format!("Failed to create subagent session: {}", e),
                    is_error: true,
                }
            }
        };

        let subagent_session_id = new_session.id;

        // Append the task as a user message
        let _ = self
            .store
            .append_message(
                crate::storage::conversation_store::NewMessage {
                    session_id: subagent_session_id,
                    role: crate::storage::conversation_store::MessageRole::User,
                    content: task.to_string(),
                    thinking_content: None,
                },
            )
            .await;

        // Clone everything needed for the background task
        let app = self.app_handle.clone();
        let store = self.store.clone();
        let enabled_mcp_servers = self.enabled_mcp_servers.clone();
        let provider = self.provider.clone();
        let model = self.model.clone();
        let provider_name = self.provider_name.clone();
        let verbose = self.verbose_logging;
        let parent_call_id = call_id.to_string();
        let task_string = task.to_string();
        let _project_path = self.project_path.clone();
        let subagent_id_str = subagent_session_id.to_string();

        // Emit created event
        let _ = app.emit(
            "subagent-created",
            serde_json::json!({
                "session_id": subagent_id_str,
                "parent_call_id": parent_call_id,
                "task": task_string,
                "provider_name": provider_name,
                "model": model,
            }),
        );

        // Spawn background task that runs the subagent
        let handle = tokio::spawn(async move {
            use crate::llm::provider::{
                ChatMessage as LmChatMessage, ChatRequest, ChatRole, FinishReason, StreamEvent,
                ToolCall,
            };
            use crate::storage::conversation_store::NewMessage;
            use std::collections::HashSet;

            // Build a fresh tool registry for the subagent (same pattern as the parent)
            let mut sub_tool_registry = ToolRegistry::new();
            // Scope the subagent's todo tree to its own hidden session.
            sub_tool_registry.register(Box::new(TodoTreeTool::for_session(
                &subagent_session_id.to_string(),
            )));
            let mut used_names: HashSet<String> = sub_tool_registry
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

                            sub_tool_registry.register(Box::new(McpProxyTool {
                                local_name: local_name.clone(),
                                local_description,
                                schema: tool.input_schema.clone(),
                                server: server.clone(),
                                remote_name: remote_tool_name.clone(),
                            }));
                        }
                    }
                    Err(e) => {
                        if verbose {
                            let _ = app.emit(
                                "output:append",
                                serde_json::json!({
                                    "source": "subagent",
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
            }

            let tool_registry = Arc::new(sub_tool_registry);
            let tool_defs = tool_registry.definitions();

            let mut messages: Vec<LmChatMessage> = Vec::new();

            // Load the user message from the subagent session
            match store.get_session(subagent_session_id).await {
                Ok(session) => {
                    for msg in &session.messages {
                        let role = match msg.role {
                            crate::storage::conversation_store::MessageRole::User => ChatRole::User,
                            crate::storage::conversation_store::MessageRole::Assistant => {
                                ChatRole::Assistant
                            }
                            crate::storage::conversation_store::MessageRole::System => {
                                ChatRole::System
                            }
                        };
                        messages.push(LmChatMessage {
                            role,
                            content: msg.content.clone(),
                            tool_call_id: None,
                            tool_calls: None,
                            reasoning_content: msg.thinking_content.clone(),
                        });
                    }
                }
                Err(e) => {
                    let _ = app.emit(
                        "subagent-error",
                        serde_json::json!({
                            "session_id": subagent_session_id.to_string(),
                            "error": format!("Failed to load session: {}", e),
                        }),
                    );
                    return;
                }
            }

            // Add a system message to guide the subagent
            if messages.is_empty() || !matches!(messages[0].role, ChatRole::System) {
                messages.insert(
                    0,
                    LmChatMessage {
                        role: ChatRole::System,
                        content: "You are a helpful sub-agent. You have access to tools to help complete the task. Work autonomously and report your findings.".to_string(),
                        tool_call_id: None,
                        tool_calls: None,
                        reasoning_content: None,
                    },
                );
            }

            // Main subagent loop — at most 10 tool-call rounds
            let mut max_tool_rounds = 10;
            loop {
                if max_tool_rounds == 0 {
                    let _ = app.emit(
                        "subagent-error",
                        serde_json::json!({
                            "session_id": subagent_session_id.to_string(),
                            "error": "Subagent exceeded maximum tool call rounds".to_string(),
                        }),
                    );
                    return;
                }
                max_tool_rounds -= 1;

                let request = ChatRequest {
                    model: model.clone(),
                    messages: messages.clone(),
                    tools: tool_defs.clone(),
                    thinking_level: None,
                    temperature: None,
                    max_tokens: None,
                };

                if verbose {
                    let _ = app.emit(
                        "output:append",
                        serde_json::json!({
                            "source": "subagent",
                            "line": format!("[subagent] >>> REQUEST ({} messages)", messages.len()),
                            "timestamp": chrono::Utc::now().to_rfc3339(),
                        }),
                    );
                }

                let mut stream = match provider.chat(request).await {
                    Ok(s) => s,
                    Err(e) => {
                        let _ = app.emit(
                            "subagent-error",
                            serde_json::json!({
                                "session_id": subagent_session_id.to_string(),
                                "error": format!("Subagent LLM request failed: {}", e),
                            }),
                        );
                        return;
                    }
                };

                let mut full_response = String::new();
                let mut tool_calls: Vec<ToolCall> = Vec::new();
                let mut finish_reason: Option<FinishReason> = None;

                while let Some(event) = stream.recv().await {
                    match event {
                        StreamEvent::Chunk(text) => {
                            full_response.push_str(&text);
                            let _ = app.emit(
                                "subagent-chunk",
                                serde_json::json!({
                                    "session_id": subagent_session_id.to_string(),
                                    "text": text,
                                }),
                            );
                        }
                        StreamEvent::Thinking(text) => {
                            let think_chunk = format!("[thinking] {}", text);
                            let _ = app.emit(
                                "subagent-chunk",
                                serde_json::json!({
                                    "session_id": subagent_session_id.to_string(),
                                    "text": think_chunk,
                                }),
                            );
                        }
                        StreamEvent::ToolCall(tc) => {
                            tool_calls.push(tc);
                        }
                        StreamEvent::Log(msg) => {
                            if verbose {
                                let _ = app.emit(
                                    "output:append",
                                    serde_json::json!({
                                        "source": "subagent",
                                        "line": format!("[subagent] {}", msg),
                                        "timestamp": chrono::Utc::now().to_rfc3339(),
                                    }),
                                );
                            }
                        }
                        StreamEvent::Done(reason) => {
                            finish_reason = Some(reason);
                            break;
                        }
                        StreamEvent::Error(err) => {
                            let _ = app.emit(
                                "subagent-error",
                                serde_json::json!({
                                    "session_id": subagent_session_id.to_string(),
                                    "error": format!("Subagent stream error: {}", err),
                                }),
                            );
                            return;
                        }
                    }
                }

                if finish_reason.is_none() {
                    finish_reason = Some(FinishReason::Stop);
                }

                let reason = finish_reason.unwrap();

                match reason {
                    FinishReason::ToolCalls => {
                        // Persist the assistant message with tool calls
                        let _ = store
                            .append_message(NewMessage {
                                session_id: subagent_session_id,
                                role: crate::storage::conversation_store::MessageRole::Assistant,
                                content: full_response.clone(),
                                thinking_content: None,
                            })
                            .await;

                        // Push assistant message with tool calls to local messages
                        messages.push(LmChatMessage {
                            role: ChatRole::Assistant,
                            content: full_response,
                            tool_call_id: None,
                            tool_calls: Some(tool_calls.clone()),
                            reasoning_content: None,
                        });

                        // Execute each tool call (auto-approved — subagent has implicit approval)
                        for tc in &tool_calls {
                            let tool_result = if let Some(tool) = tool_registry.get(&tc.name) {
                                tool.execute(&tc.id, tc.arguments.clone()).await
                            } else {
                                ToolResult {
                                    call_id: tc.id.clone(),
                                    output: format!("Unknown tool: {}", tc.name),
                                    is_error: true,
                                }
                            };

                            // Push tool result to messages
                            messages.push(LmChatMessage {
                                role: ChatRole::Tool,
                                content: tool_result.output.clone(),
                                tool_call_id: Some(tc.id.clone()),
                                tool_calls: None,
                                reasoning_content: None,
                            });

                            if verbose {
                                let _ = app.emit(
                                    "output:append",
                                    serde_json::json!({
                                        "source": "subagent",
                                        "line": format!(
                                            "[subagent] tool {} = {} bytes, is_error={}",
                                            tc.name,
                                            tool_result.output.len(),
                                            tool_result.is_error
                                        ),
                                        "timestamp": chrono::Utc::now().to_rfc3339(),
                                    }),
                                );
                            }
                        }

                        // Continue the loop for another LLM call
                        continue;
                    }
                    FinishReason::Stop | FinishReason::Length => {
                        // Persist the final assistant message
                        let _ = store
                            .append_message(NewMessage {
                                session_id: subagent_session_id,
                                role: crate::storage::conversation_store::MessageRole::Assistant,
                                content: full_response.clone(),
                                thinking_content: None,
                            })
                            .await;

                        let _ = app.emit(
                            "subagent-done",
                            serde_json::json!({
                                "session_id": subagent_session_id.to_string(),
                                "result": full_response,
                                "is_error": false,
                            }),
                        );

                        if verbose {
                            let _ = app.emit(
                                "output:append",
                                serde_json::json!({
                                    "source": "subagent",
                                    "line": format!(
                                        "[subagent] completed: {} chars",
                                        full_response.len()
                                    ),
                                    "timestamp": chrono::Utc::now().to_rfc3339(),
                                }),
                            );
                        }

                        return;
                    }
                    FinishReason::Error => {
                        let _ = app.emit(
                            "subagent-error",
                            serde_json::json!({
                                "session_id": subagent_session_id.to_string(),
                                "error": "Subagent LLM returned an error finish reason".to_string(),
                            }),
                        );
                        return;
                    }
                }
            }
        });

        // Store handle so the subagent can be stopped/cancelled
        let sid_str = subagent_session_id.to_string();
        if let Some(state) = self.app_handle.try_state::<crate::commands::AppState>() {
            let mut tasks = state.subagent_tasks.lock().await;
            tasks.insert(sid_str, handle);
        }

        // Return immediately — subagent runs in background
        ToolResult {
            call_id: call_id.to_string(),
            output: format!(
                "[Subagent started on task: {}]\nSession: {}\nThe subagent is working independently and will report back when done.",
                task, subagent_session_id
            ),
            is_error: false,
        }
    }
}
