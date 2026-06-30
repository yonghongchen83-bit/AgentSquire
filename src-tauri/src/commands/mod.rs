use std::sync::{Arc, Mutex, RwLock};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::agent::{self, PendingApprovals, ToolDanger, ToolRegistry};
use crate::fs::ops::{self, FileEntry};
use crate::fs::watcher::{FileEvent, FileWatcher};
use crate::llm::provider::{
    ChatMessage, ChatRequest, ChatRole, FinishReason, StreamEvent, ToolCall,
};
use crate::llm::registry::{ProviderInfo, ProviderRegistry};
use crate::search::grep::{self, GrepReplaceOptions, SearchMatch, SearchOptions};
use crate::shell::exec::{self, CommandResult};
use crate::state::config::{self, AppConfig};
use crate::storage::conversation_store::{
    ConversationStore, NewMessage, NewSession, SessionId, SessionSummary, SessionWithMessages,
};
use crate::terminal::manager::PtyManager;

pub struct AppState {
    pub config: RwLock<AppConfig>,
    pub store: Arc<dyn ConversationStore>,
    pub registry: RwLock<ProviderRegistry>,
}

pub struct WatcherState {
    pub watcher: Mutex<FileWatcher>,
}

pub struct TerminalState {
    pub manager: PtyManager,
}

// ── Config ──

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> Result<AppConfig, String> {
    state
        .config
        .read()
        .map(|c| c.clone())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_config(new_config: AppConfig, state: State<'_, AppState>) -> Result<(), String> {
    config::save_config(&new_config).map_err(|e| e.to_string())?;
    *state.config.write().map_err(|e| e.to_string())? = new_config.clone();
    state
        .registry
        .write()
        .map_err(|e| e.to_string())?
        .rebuild_from_config(&new_config);
    Ok(())
}

#[tauri::command]
pub fn load_config() -> Result<AppConfig, String> {
    config::load_config().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn check_update() -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({
        "available": false,
        "version": null,
        "body": null,
    }))
}

// ── Conversations ──

#[tauri::command]
pub async fn list_conversations(state: State<'_, AppState>) -> Result<Vec<SessionSummary>, String> {
    state.store.list_sessions().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_conversation(
    state: State<'_, AppState>,
    id: String,
) -> Result<SessionWithMessages, String> {
    let session_id = SessionId::parse_str(&id).map_err(|e| format!("Invalid session ID: {}", e))?;
    state
        .store
        .get_session(session_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_conversation(
    state: State<'_, AppState>,
    title: String,
) -> Result<crate::storage::conversation_store::Session, String> {
    state
        .store
        .create_session(NewSession { title })
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_conversation(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let session_id = SessionId::parse_str(&id).map_err(|e| format!("Invalid session ID: {}", e))?;
    state
        .store
        .delete_session(session_id)
        .await
        .map_err(|e| e.to_string())
}

// ── Send Message (with tool support) ──

#[tauri::command]
pub async fn send_message(
    app: AppHandle,
    state: State<'_, AppState>,
    pending_state: State<'_, PendingApprovals>,
    session_id: String,
    content: String,
    provider_name: Option<String>,
    model: Option<String>,
) -> Result<(), String> {
    let sid =
        SessionId::parse_str(&session_id).map_err(|e| format!("Invalid session ID: {}", e))?;

    state
        .store
        .append_message(NewMessage {
            session_id: sid,
            role: crate::storage::conversation_store::MessageRole::User,
            content: content.clone(),
        })
        .await
        .map_err(|e| e.to_string())?;

    let session = state
        .store
        .get_session(sid)
        .await
        .map_err(|e| e.to_string())?;

    let verbose_logging = state
        .config
        .read()
        .map(|c| c.verbose_logging)
        .unwrap_or(false);

    let (provider_arc, selected_model) = {
        let reg = state.registry.read().map_err(|e| e.to_string())?;
        let name = provider_name
            .clone()
            .or_else(|| reg.default_name().map(|s| s.to_string()))
            .ok_or_else(|| "No default LLM provider configured".to_string())?;
        let entry = reg
            .get(&name)
            .ok_or_else(|| format!("Provider '{}' not found", name))?;
        let sm = model.clone().unwrap_or_else(|| entry.default_model.clone());
        (entry.provider.clone(), sm)
    };

    let store = state.store.clone();
    let app_clone = app.clone();
    let pending = pending_state.pending.clone();

    tokio::spawn(async move {
        let tool_registry = Arc::new(ToolRegistry::new());
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
            })
            .collect();

        loop {
            let request = ChatRequest {
                model: selected_model.clone(),
                messages: messages.clone(),
                tools: tool_defs.clone(),
                temperature: None,
                max_tokens: None,
            };

            let mut stream = match provider_arc.chat(request).await {
                Ok(s) => s,
                Err(e) => {
                    let _ = app_clone.emit("stream-error", e.to_string());
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
                        let _ = app_clone.emit("stream-chunk", text);
                    }
                    StreamEvent::ThinkingChunk(text) => {
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
                        finish_reason = Some(reason);
                        break;
                    }
                    StreamEvent::Error(err) => {
                        let _ = app_clone.emit("stream-error", err);
                        return;
                    }
                }
            }

            let reason = match finish_reason {
                Some(r) => r,
                None => {
                    // Some OpenAI-compatible backends (including Ollama variants) may close
                    // the stream without an explicit finish_reason. Infer a reasonable end state.
                    if !tool_calls.is_empty() {
                        FinishReason::ToolCalls
                    } else if !full_response.is_empty() {
                        FinishReason::Stop
                    } else {
                        let _ =
                            app_clone.emit("stream-error", "Stream ended without finish reason");
                        return;
                    }
                }
            };

            match reason {
                FinishReason::ToolCalls => {
                    if !full_response.is_empty() {
                        let _ = app_clone.emit("stream-chunk", "\n\n");
                    }

                    for tc in &tool_calls {
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

                                match rx.await {
                                    Ok(true) => {
                                        // Approved → execute
                                        let _ = app_clone.emit(
                                            "stream-chunk",
                                            format!("[Executing {}...]\n", tc.name),
                                        );
                                        tool.execute(&tc.id, tc.arguments.clone()).await
                                    }
                                    _ => {
                                        // Rejected or channel closed
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
                                tool.execute(&tc.id, tc.arguments.clone()).await
                            }
                        } else {
                            agent::ToolResult {
                                call_id: tc.id.clone(),
                                output: format!("Unknown tool: {}", tc.name),
                                is_error: true,
                            }
                        };

                        let _ = app_clone.emit("stream-tool-result", &result);

                        messages.push(ChatMessage {
                            role: ChatRole::Assistant,
                            content: String::new(),
                            tool_call_id: Some(tc.id.clone()),
                            tool_calls: Some(vec![ToolCall {
                                id: tc.id.clone(),
                                name: tc.name.clone(),
                                arguments: tc.arguments.clone(),
                            }]),
                        });

                        messages.push(ChatMessage {
                            role: ChatRole::Tool,
                            content: result.output.clone(),
                            tool_call_id: Some(tc.id.clone()),
                            tool_calls: None,
                        });
                    }

                    continue;
                }
                FinishReason::Stop | FinishReason::Length => {
                    let content = std::mem::take(&mut full_response);
                    if verbose_logging {
                        let _ = app_clone.emit(
                            "output:append",
                            serde_json::json!({
                                "source": "chat",
                                "line": format!("<<< FINAL_RESPONSE\n{}", content),
                                "timestamp": chrono::Utc::now().to_rfc3339(),
                            }),
                        );
                    }
                    if !content.is_empty() {
                        let _ = store
                            .append_message(NewMessage {
                                session_id: sid,
                                role: crate::storage::conversation_store::MessageRole::Assistant,
                                content,
                            })
                            .await;
                    }
                    let _ = app_clone.emit("stream-done", "");
                    return;
                }
                FinishReason::Error => {
                    let _ = app_clone.emit("stream-error", "LLM returned an error");
                    return;
                }
            }
        }
    });

    Ok(())
}

// ── Approve / Reject Tool Calls ──

#[tauri::command]
pub async fn approve_tool_call(
    pending_state: State<'_, PendingApprovals>,
    call_id: String,
) -> Result<(), String> {
    let sender = {
        let mut p = pending_state.pending.lock().await;
        p.remove(&call_id)
    };
    match sender {
        Some(sender) => sender
            .send(true)
            .map_err(|_| "Failed to send approval".to_string()),
        None => Err(format!("No pending tool call with id '{}'", call_id)),
    }
}

#[tauri::command]
pub async fn reject_tool_call(
    pending_state: State<'_, PendingApprovals>,
    call_id: String,
) -> Result<(), String> {
    let sender = {
        let mut p = pending_state.pending.lock().await;
        p.remove(&call_id)
    };
    match sender {
        Some(sender) => sender
            .send(false)
            .map_err(|_| "Failed to send rejection".to_string()),
        None => Err(format!("No pending tool call with id '{}'", call_id)),
    }
}

// ── LLM Providers ──

#[tauri::command]
pub fn list_providers(state: State<'_, AppState>) -> Vec<ProviderInfo> {
    state
        .registry
        .read()
        .map(|reg| reg.list())
        .unwrap_or_default()
}

#[tauri::command]
pub async fn test_connection(
    provider_type: String,
    api_key: String,
    model: String,
    endpoint: Option<String>,
) -> Result<String, String> {
    if api_key.is_empty() {
        return Err("API key is required".to_string());
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|_| "Connection failed: unable to create HTTP client".to_string())?;

    match provider_type.to_lowercase().as_str() {
        "openai" => {
            let base = endpoint.unwrap_or_else(|| "https://api.openai.com/v1".to_string());
            let url = if base.ends_with("/chat/completions") || base.ends_with("/responses") {
                base
            } else {
                format!("{}/chat/completions", base.trim_end_matches('/'))
            };

            let body = serde_json::json!({
                "model": model,
                "messages": [{"role": "user", "content": "Say ok"}],
                "max_tokens": 50,
                "stream": false,
            });

            let resp = client
                .post(&url)
                .header("Authorization", format!("Bearer {}", api_key))
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await
                .map_err(|e| {
                    let msg = e.to_string();
                    if msg.contains("dns")
                        || msg.contains("resolve")
                        || msg.contains("connect")
                        || msg.contains("refused")
                        || msg.contains("timed out")
                    {
                        "Connection failed: unable to reach the server".to_string()
                    } else {
                        format!("Connection failed: {}", msg)
                    }
                })?;

            if !resp.status().is_success() {
                let status = resp.status();
                let body_text = resp.text().await.unwrap_or_default();
                let detail = if body_text.is_empty() {
                    String::new()
                } else {
                    let trimmed = body_text.trim();
                    let snippet = if trimmed.len() > 300 {
                        &trimmed[..300]
                    } else {
                        trimmed
                    };
                    format!(": {}", snippet)
                };
                return match status.as_u16() {
                    401 => Err(format!(
                        "Connection failed: invalid API key or authentication error{}",
                        detail
                    )),
                    429 => Err(format!(
                        "Connection failed: rate limited by the server{}",
                        detail
                    )),
                    _ => Err(format!("Connection failed: HTTP {}{}", status, detail)),
                };
            }

            let json: serde_json::Value = resp
                .json()
                .await
                .map_err(|_| "Connection failed: invalid response from server".to_string())?;

            let content = json["choices"][0]["message"]["content"]
                .as_str()
                .unwrap_or("");
            let finish = json["choices"][0]["finish_reason"].as_str().unwrap_or("");

            if finish == "stop" || finish == "length" {
                Ok("Connection successful".to_string())
            } else {
                Ok(format!("Connected (response: {})", content))
            }
        }
        "anthropic" => {
            let base = endpoint.unwrap_or_else(|| "https://api.anthropic.com/v1".to_string());
            let url = if base.ends_with("/messages") {
                base
            } else {
                format!("{}/messages", base.trim_end_matches('/'))
            };

            let body = serde_json::json!({
                "model": model,
                "max_tokens": 50,
                "messages": [{"role": "user", "content": "Say ok"}],
            });

            let resp = client
                .post(&url)
                .header("x-api-key", &api_key)
                .header("anthropic-version", "2023-06-01")
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await
                .map_err(|_| "Connection failed: unable to reach the server".to_string())?;

            if !resp.status().is_success() {
                let status = resp.status();
                return match status.as_u16() {
                    401 => {
                        Err("Connection failed: invalid API key or authentication error"
                            .to_string())
                    }
                    429 => Err("Connection failed: rate limited by the server".to_string()),
                    _ => Err(format!("Connection failed: HTTP {}", status)),
                };
            }

            Ok("Connection successful".to_string())
        }
        _ => Err(format!("Unknown provider type: {}", provider_type)),
    }
}

#[tauri::command]
pub async fn fetch_models(
    provider_type: String,
    endpoint: String,
    api_key: Option<String>,
) -> Result<Vec<String>, String> {
    fn parse_model_names(json: &serde_json::Value) -> Vec<String> {
        if let Some(arr) = json["data"].as_array() {
            let names: Vec<String> = arr
                .iter()
                .filter_map(|m| m["id"].as_str().map(|s| s.to_string()))
                .collect();
            if !names.is_empty() {
                return names;
            }
        }

        if let Some(arr) = json["models"].as_array() {
            let names: Vec<String> = arr
                .iter()
                .filter_map(|m| {
                    m["name"]
                        .as_str()
                        .or_else(|| m["model"].as_str())
                        .map(|s| s.to_string())
                })
                .collect();
            if !names.is_empty() {
                return names;
            }
        }

        Vec::new()
    }

    async fn fetch_openai_models(
        client: &reqwest::Client,
        models_url: &str,
        api_key: Option<&String>,
    ) -> Result<Vec<String>, String> {
        let url = format!("{}/models", models_url);
        let mut req = client.get(&url);
        if let Some(key) = api_key {
            if !key.is_empty() {
                req = req.header("Authorization", format!("Bearer {}", key));
            }
        }
        let resp = req.send().await.map_err(|e| format!("HTTP error: {}", e))?;
        let status = resp.status();
        let text = resp.text().await.map_err(|e| e.to_string())?;
        if !status.is_success() {
            return Err(format!("Server returned {}: {}", status, text));
        }
        let json: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| format!("Invalid JSON: {}", e))?;
        Ok(parse_model_names(&json))
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    let models_url = {
        let trimmed = endpoint.trim_end_matches('/');
        if trimmed.ends_with("/chat/completions")
            || trimmed.ends_with("/responses")
            || trimmed.ends_with("/messages")
        {
            trimmed
                .rsplit_once('/')
                .map(|(base, _)| base)
                .unwrap_or(trimmed)
                .to_string()
        } else {
            trimmed.to_string()
        }
    };

    match provider_type.to_lowercase().as_str() {
        "openai" | "custom" => {
            let mut names = fetch_openai_models(&client, &models_url, api_key.as_ref()).await?;

            // Ollama compatibility: some versions expose model list via /api/tags only.
            if names.is_empty()
                && (models_url.contains("localhost:11434")
                    || models_url.contains("127.0.0.1:11434"))
            {
                let base = models_url.trim_end_matches("/v1").trim_end_matches('/');
                let tags_url = format!("{}/api/tags", base);
                let resp = client
                    .get(&tags_url)
                    .send()
                    .await
                    .map_err(|e| format!("HTTP error: {}", e))?;
                let status = resp.status();
                let text = resp.text().await.map_err(|e| e.to_string())?;
                if status.is_success() {
                    let json: serde_json::Value =
                        serde_json::from_str(&text).map_err(|e| format!("Invalid JSON: {}", e))?;
                    names = parse_model_names(&json);
                }
            }

            if names.is_empty() {
                return Err("No models found in response".to_string());
            }
            Ok(names)
        }
        "anthropic" => {
            let url = format!("{}/models", models_url);
            let mut req = client.get(&url);
            if let Some(key) = &api_key {
                req = req.header("x-api-key", key);
                req = req.header("anthropic-version", "2023-06-01");
            }
            let resp = req.send().await.map_err(|e| format!("HTTP error: {}", e))?;
            let status = resp.status();
            let text = resp.text().await.map_err(|e| e.to_string())?;
            if !status.is_success() {
                return Err(format!("Server returned {}: {}", status, text));
            }
            let json: serde_json::Value =
                serde_json::from_str(&text).map_err(|e| format!("Invalid JSON: {}", e))?;
            let models = json["data"]
                .as_array()
                .ok_or_else(|| "No 'data' array in response".to_string())?;
            let names: Vec<String> = models
                .iter()
                .filter_map(|m| m["id"].as_str().map(|s| s.to_string()))
                .collect();
            if names.is_empty() {
                return Err("No models found in response".to_string());
            }
            Ok(names)
        }
        _ => Err(format!("Unknown provider type: {}", provider_type)),
    }
}

// ── File Operations ──

#[tauri::command]
pub fn read_file(path: String) -> Result<String, String> {
    ops::read_file(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn write_file(path: String, content: String) -> Result<(), String> {
    ops::write_file(&path, &content).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_dir(path: String) -> Result<(), String> {
    ops::create_dir(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_item(path: String) -> Result<(), String> {
    ops::delete_item(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn rename_item(from: String, to: String) -> Result<(), String> {
    ops::rename_item(&from, &to).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_directory(path: String) -> Result<Vec<FileEntry>, String> {
    ops::list_directory(&path).map_err(|e| e.to_string())
}

// ── Search ──

#[tauri::command]
pub fn search_files(
    query: String,
    path: String,
    regex: bool,
    case_sensitive: bool,
    whole_word: bool,
    max_results: Option<usize>,
    glob: Option<String>,
    context_lines: Option<u64>,
) -> Result<Vec<SearchMatch>, String> {
    let options = SearchOptions {
        query,
        path,
        regex,
        case_sensitive,
        whole_word,
        max_results,
        glob,
        context_lines,
    };
    grep::search(&options).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn replace_in_files(
    query: String,
    replacement: String,
    path: String,
    regex: bool,
    case_sensitive: bool,
    glob: Option<String>,
) -> Result<usize, String> {
    let options = GrepReplaceOptions {
        query,
        replacement,
        path,
        regex,
        case_sensitive,
        glob,
    };
    grep::grep_replace(&options).map_err(|e| e.to_string())
}

// ── Git ──

#[tauri::command]
pub fn git_status(path: Option<String>) -> Result<Vec<crate::fs::git::GitStatus>, String> {
    let p = path.as_deref().unwrap_or(".");
    crate::fs::git::status(p).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn git_diff(path: String, staged: bool) -> Result<Vec<crate::fs::git::GitDiff>, String> {
    crate::fs::git::diff(&path, staged).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn git_log(path: String, max_count: i32) -> Result<Vec<crate::fs::git::GitLogEntry>, String> {
    crate::fs::git::log(&path, max_count).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn git_branches(path: String) -> Result<Vec<crate::fs::git::GitBranch>, String> {
    crate::fs::git::branches(&path).map_err(|e| e.to_string())
}

// ── Shell ──

#[tauri::command]
pub fn execute_command(
    command: String,
    args: Vec<String>,
    workdir: Option<String>,
) -> Result<CommandResult, String> {
    exec::execute(&command, &args, workdir.as_deref()).map_err(|e| e.to_string())
}

// ── File Watcher ──

#[tauri::command]
pub fn watch_directory(
    app: AppHandle,
    watcher_state: State<'_, WatcherState>,
    path: String,
) -> Result<(), String> {
    let mut watcher = watcher_state
        .watcher
        .lock()
        .map_err(|e| format!("Lock error: {}", e))?;
    watcher.watch(&path)?;

    let app_clone = app.clone();
    drop(watcher);

    let _ = app_clone.emit(
        "file-event",
        FileEvent {
            kind: "watch-started".into(),
            paths: vec![path],
        },
    );

    Ok(())
}

// ── Terminal ──

#[tauri::command]
pub async fn spawn_terminal(
    app: AppHandle,
    term_state: State<'_, TerminalState>,
    shell: Option<String>,
) -> Result<String, String> {
    term_state.manager.spawn(app, shell, None).await
}

#[tauri::command]
pub async fn write_stdin(
    term_state: State<'_, TerminalState>,
    terminal_id: String,
    data: String,
) -> Result<(), String> {
    term_state.manager.write(&terminal_id, &data).await
}

#[tauri::command]
pub async fn resize_pty(
    term_state: State<'_, TerminalState>,
    terminal_id: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    term_state.manager.resize(&terminal_id, cols, rows).await
}

#[tauri::command]
pub async fn kill_terminal(
    term_state: State<'_, TerminalState>,
    terminal_id: String,
) -> Result<(), String> {
    term_state.manager.kill(&terminal_id).await
}

#[tauri::command]
pub async fn list_terminals(term_state: State<'_, TerminalState>) -> Result<Vec<String>, String> {
    Ok(term_state.manager.list().await)
}

// ── App Setup ──

pub fn setup_app(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let app_handle = app.handle().clone();

    let config_dir = app
        .path()
        .app_config_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."));
    config::set_config_dir(config_dir.clone());

    let config = config::load_config().unwrap_or_default();

    let db_path = config_dir.join("squirecli.db");
    let db = crate::state::db::Database::open(&db_path)
        .map_err(|e| format!("Failed to open database: {}", e))?;

    let registry = ProviderRegistry::from_config(&config);

    let (file_watcher, mut watcher_rx) = FileWatcher::new();

    tauri::async_runtime::spawn(async move {
        while let Ok(event) = watcher_rx.recv().await {
            let _ = app_handle.emit("file-event", event);
        }
    });

    app.manage(AppState {
        config: RwLock::new(config),
        store: Arc::new(db),
        registry: RwLock::new(registry),
    });

    app.manage(WatcherState {
        watcher: Mutex::new(file_watcher),
    });

    app.manage(TerminalState {
        manager: PtyManager::new(),
    });

    app.manage(PendingApprovals::new());

    if cfg!(debug_assertions) {
        app.handle().plugin(
            tauri_plugin_log::Builder::default()
                .level(log::LevelFilter::Info)
                .build(),
        )?;
    }

    Ok(())
}
