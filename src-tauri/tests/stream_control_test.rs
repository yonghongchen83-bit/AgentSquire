use squirecli_lib::agent::{PendingApprovals, PendingAskUserQuestions};
use squirecli_lib::commands::stream_control::{
    resolve_ask_user_answer_impl, resolve_tool_call_decision_impl,
};

#[tokio::test]
async fn approve_decision_sends_true() {
    let pending = PendingApprovals::new();
    let (tx, rx) = tokio::sync::oneshot::channel();
    {
        let mut map = pending.pending.lock().await;
        map.insert("call-1".to_string(), tx);
    }

    let result =
        resolve_tool_call_decision_impl(&pending.pending, "call-1".to_string(), true).await;

    assert!(result.is_ok());
    assert!(rx.await.expect("receiver should get decision"));
}

#[tokio::test]
async fn resolve_decision_errors_for_unknown_call() {
    let pending = PendingApprovals::new();
    let result =
        resolve_tool_call_decision_impl(&pending.pending, "missing".to_string(), false).await;

    assert!(result.is_err());
    assert!(result
        .expect_err("expected missing pending call error")
        .contains("No pending tool call with id 'missing'"));
}

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
    assert!(result
        .expect_err("expected missing pending question error")
        .contains("No pending question with id 'missing'"));
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
