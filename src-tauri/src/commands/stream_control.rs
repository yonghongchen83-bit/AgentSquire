use super::AppState;
use crate::agent::{ApprovalSender, PendingApprovals};
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

#[cfg(test)]
mod tests {
    use super::resolve_tool_call_decision_impl;
    use crate::agent::PendingApprovals;

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
}
