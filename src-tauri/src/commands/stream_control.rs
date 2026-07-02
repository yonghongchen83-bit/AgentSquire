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

#[cfg(test)]
mod tests {
    use super::{resolve_ask_user_answer_impl, resolve_tool_call_decision_impl};
    use crate::agent::{PendingApprovals, PendingAskUserQuestions};

    #[tokio::test]
    async fn approve_decision_sends_true() {
        let pending = PendingApprovals::new();
        let (tx, rx) = tokio::sync::oneshot::channel();
        {
            let mut map = pending.pending.lock().await;
            map.insert("call-1".to_string(), tx);
        }

        let result = resolve_tool_call_decision_impl(&pending.pending, "call-1".to_string(), true)
            .await;

        assert!(result.is_ok());
        assert_eq!(rx.await.expect("receiver should get decision"), true);
    }

    #[tokio::test]
    async fn resolve_decision_errors_for_unknown_call() {
        let pending = PendingApprovals::new();
        let result =
            resolve_tool_call_decision_impl(&pending.pending, "missing".to_string(), false).await;

        assert!(result.is_err());
        assert!(
            result
                .expect_err("expected missing pending call error")
                .contains("No pending tool call with id 'missing'")
        );
    }

    // ── sa-5: PendingAskUserQuestions resolve ──

    #[tokio::test]
    async fn resolve_ask_user_answer_sends_answer_to_waiting_receiver() {
        let pending = PendingAskUserQuestions::new();
        let (tx, rx) = tokio::sync::oneshot::channel();
        {
            let mut map = pending.pending.lock().await;
            map.insert("question-1".to_string(), tx);
        }

        let result = resolve_ask_user_answer_impl(
            &pending.pending,
            "question-1".to_string(),
            "Sydney".to_string(),
        )
        .await;

        assert!(result.is_ok());
        assert_eq!(rx.await.expect("receiver should get answer"), "Sydney");
    }

    #[tokio::test]
    async fn resolve_ask_user_answer_errors_for_unknown_question_id() {
        let pending = PendingAskUserQuestions::new();
        let result = resolve_ask_user_answer_impl(
            &pending.pending,
            "missing".to_string(),
            "answer".to_string(),
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .expect_err("expected missing pending question error")
                .contains("No pending question with id 'missing'")
        );
    }

    #[tokio::test]
    async fn resolve_ask_user_answer_removes_entry_so_it_cannot_be_answered_twice() {
        let pending = PendingAskUserQuestions::new();
        let (tx, _rx) = tokio::sync::oneshot::channel();
        {
            let mut map = pending.pending.lock().await;
            map.insert("question-1".to_string(), tx);
        }

        let first = resolve_ask_user_answer_impl(
            &pending.pending,
            "question-1".to_string(),
            "first answer".to_string(),
        )
        .await;
        assert!(first.is_ok());

        let second = resolve_ask_user_answer_impl(
            &pending.pending,
            "question-1".to_string(),
            "second answer".to_string(),
        )
        .await;
        assert!(second.is_err());
    }

    #[tokio::test]
    async fn resolve_ask_user_answer_errors_when_receiver_already_dropped() {
        // Simulates the abandonment case: the turn task was aborted (e.g.
        // via abort_stream or a new message on the same session), which
        // drops the paired oneshot::Receiver. A late answer submission
        // should fail cleanly, not panic.
        let pending = PendingAskUserQuestions::new();
        let (tx, rx) = tokio::sync::oneshot::channel();
        {
            let mut map = pending.pending.lock().await;
            map.insert("question-1".to_string(), tx);
        }
        drop(rx);

        let result = resolve_ask_user_answer_impl(
            &pending.pending,
            "question-1".to_string(),
            "too late".to_string(),
        )
        .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("no longer waiting"));
    }
}
