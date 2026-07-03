use super::AppState;
use crate::agent::{AskUserAnswerSender, ApprovalSender, PendingApprovals, PendingAskUserQuestions};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::Mutex;

fn emit_stream_status(app: &AppHandle, status: &str) {
    let _ = app.emit("stream-status", status.to_string());
}

pub async fn abort_stream_impl(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: String,
) -> Result<(), String> {
    let handle = {
        let mut tasks = state.stream_tasks.lock().await;
        tasks.remove(&session_id)
    };

    match handle {
        Some(handle) => {
            handle.abort();
            emit_stream_status(&app, "Stopped by user");
            let _ = app.emit("stream-error", "Generation aborted by user");
            Ok(())
        }
        None => Err(format!("No active stream for session '{}'", session_id)),
    }
}

pub async fn resolve_tool_call_decision_impl(
    pending: &Arc<Mutex<HashMap<String, ApprovalSender>>>,
    call_id: String,
    decision: bool,
) -> Result<(), String> {
    let sender = {
        let mut p = pending.lock().await;
        p.remove(&call_id)
    };

    match sender {
        Some(sender) => sender.send(decision).map_err(|_| {
            if decision {
                "Failed to send approval".to_string()
            } else {
                "Failed to send rejection".to_string()
            }
        }),
        None => Err(format!("No pending tool call with id '{}'", call_id)),
    }
}

pub async fn approve_tool_call_impl(
    pending_state: State<'_, PendingApprovals>,
    call_id: String,
) -> Result<(), String> {
    resolve_tool_call_decision_impl(&pending_state.pending, call_id, true).await
}

pub async fn reject_tool_call_impl(
    pending_state: State<'_, PendingApprovals>,
    call_id: String,
) -> Result<(), String> {
    resolve_tool_call_decision_impl(&pending_state.pending, call_id, false).await
}

// ── Pending AskUser Questions (sa-5) ──
//
// Structurally identical to `resolve_tool_call_decision_impl` above — see
// `.AiControl/root/Squire/ask-user-loop/decisions.md`.

pub async fn resolve_ask_user_answer_impl(
    pending: &Arc<Mutex<HashMap<String, AskUserAnswerSender>>>,
    question_id: String,
    answer: String,
) -> Result<(), String> {
    let sender = {
        let mut p = pending.lock().await;
        p.remove(&question_id)
    };

    match sender {
        Some(sender) => sender
            .send(answer)
            .map_err(|_| "Failed to send answer: turn is no longer waiting for it".to_string()),
        None => Err(format!("No pending question with id '{}'", question_id)),
    }
}

pub async fn answer_ask_user_question_impl(
    pending_state: State<'_, PendingAskUserQuestions>,
    question_id: String,
    answer: String,
) -> Result<(), String> {
    resolve_ask_user_answer_impl(&pending_state.pending, question_id, answer).await
}
