use super::utils::derive_session_title_from_message;
use super::AppState;
use crate::agent::{self, PendingApprovals, PendingAskUserQuestions};
use crate::engine::{Engine, RuntimeContext, SquireEngine, TauriEventEmitter};
use crate::storage::conversation_store::{ContextMode, NewMessage, SessionId};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, State};

fn emit_stream_status(app: &AppHandle, status: &str) {
    let _ = app.emit("stream-status", status.to_string());
}

/// Split text into chunks of at most `max_len` bytes, splitting on
/// newlines when possible so the frontend receives reasonably-sized
/// stream-chunk events.
fn split_into_chunks(text: &str, max_len: usize) -> Vec<String> {
    if text.len() <= max_len {
        return vec![text.to_string()];
    }
    let mut chunks = Vec::new();
    let mut start = 0;
    let bytes = text.as_bytes();
    while start < bytes.len() {
        let end = (start + max_len).min(bytes.len());
        // Try to break at a newline before end for cleaner splits
        let split_at = if end < bytes.len() {
            // Look backwards for a newline
            let mut newline_pos = end;
            while newline_pos > start && bytes[newline_pos] != b'\n' {
                newline_pos -= 1;
            }
            if newline_pos > start { newline_pos + 1 } else { end }
        } else {
            end
        };
        chunks.push(text[start..split_at].to_string());
        start = split_at;
    }
    chunks
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
    _pending_state: State<'_, PendingApprovals>,
    pending_ask_user_state: State<'_, PendingAskUserQuestions>,
    session_id: String,
    content: String,
    provider_name: Option<String>,
    model: Option<String>,
    _thinking_level: Option<String>,
    phase2_provider: Option<String>,
    phase2_model: Option<String>,
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

    // Resolve Phase 1 provider, model, and provider name
    let (_provider_arc, selected_model, selected_provider_name) = {
        let reg = state.registry.read().map_err(|e| e.to_string())?;
        let name = provider_name
            .clone()
            .or_else(|| reg.default_name().map(|s| s.to_string()))
            .ok_or_else(|| "No default LLM provider configured".to_string())?;
        let entry = reg
            .get(&name)
            .ok_or_else(|| format!("Provider '{}' not found", name))?;
        let sm = model.clone().unwrap_or_else(|| entry.default_model.clone());
        // Destructure to get individual bindings so provider_arc can be `mut`
        // inside the spawn for Phase 2 provider switching.
        (entry.provider.clone(), sm, name)
    };
    let app_clone = app.clone();
    let stream_tasks = state.stream_tasks.clone();
    let session_key = sid.to_string();

    if let Some(existing) = stream_tasks.lock().await.remove(&session_key) {
        existing.abort();
    }

    // Build a ModelInstance from the resolved provider + model.
    let model_instance = state
        .registry
        .read()
        .map_err(|e| e.to_string())?
        .resolve_model_instance(&selected_provider_name, &selected_model, None)
        .map_err(|e| e.to_string())?;

    // Build Phase 2 ModelInstance if a different provider/model was selected.
    let phase2_model_instance = {
        let p2_prov = phase2_provider.filter(|s| !s.is_empty());
        let p2_mod = phase2_model.filter(|s| !s.is_empty());
        match (p2_prov, p2_mod) {
            (Some(ref prov), Some(ref mod_)) => {
                match state
                    .registry
                    .read()
                    .map_err(|e| e.to_string())?
                    .resolve_model_instance(prov, mod_, None)
                {
                    Ok(inst) => Some(inst),
                    Err(_) => {
                        let _ = app.emit(
                            "output:append",
                            serde_json::json!({
                                "source": "chat",
                                "line": format!(
                                    "WARNING: Phase 2 provider '{}' not found in registry. Falling back to Phase 1 provider.",
                                    prov
                                ),
                                "timestamp": chrono::Utc::now().to_rfc3339(),
                            }),
                        );
                        None
                    }
                }
            }
            _ => None,
        }
    };

    // Build RuntimeContext from AppState.
    let config: crate::engine::RuntimeConfig = state
        .config
        .read()
        .map_err(|e| e.to_string())?
        .clone()
        .into();

    let event_emitter: Arc<dyn crate::engine::EventEmitter> =
        Arc::new(TauriEventEmitter::new(app.clone()));

    let ctx = RuntimeContext {
        provider_registry: Arc::new(
            state.registry.read().map_err(|e| e.to_string())?.clone(),
        ),
        store: state.store.read().map_err(|e| e.to_string())?.clone(),
        squire_store: state.squire_store.read().map_err(|e| e.to_string())?.clone(),
        project_path: state
            .project_path
            .read()
            .map(|p| p.clone())
            .unwrap_or_default(),
        config,
        mcp_tools_cache: state.mcp_tools_cache.clone(),
        tool_registry_hash: state.tool_registry_hash.clone(),
        tool_endpoints: HashMap::new(),
        event_emitter,
        cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        model_instance,
        phase2_model_instance,
        app_handle: Some(app.clone()),
    };

    let stream_tasks_cleanup = stream_tasks.clone();
    let session_key_cleanup = session_key.clone();
    let pending_ask_user = pending_ask_user_state.pending.clone();

    let engine = Box::new(SquireEngine);
    let handle = tokio::spawn(async move {
        let result = engine.run(ctx, sid).await;
        match result {
            Ok(()) => {}
            Err(ref msg) if msg.starts_with("__ASK_USER__") => {
                let parts: Vec<&str> = msg.splitn(3, ':').collect();
                if parts.len() == 3 {
                    let question_id = parts[1].to_string();
                    let question = parts[2].to_string();
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
                        Some(_answer) => {
                            emit_stream_status(&app_clone, "Answer received");
                            let _ = app_clone.emit("stream-done", "");
                        }
                        None => {
                            emit_stream_status(&app_clone, "Stopped waiting for answer");
                        }
                    }
                }
            }
            Err(e) => {
                emit_stream_status(&app_clone, "Engine error");
                let _ = app_clone.emit("stream-error", e);
            }
        }
        let mut tasks = stream_tasks_cleanup.lock().await;
        tasks.remove(&session_key_cleanup);
    });

    stream_tasks.lock().await.insert(session_key, handle);

    Ok(())
}

#[cfg(test)]
#[path = "streaming_cmd_test.rs"]
mod tests;
