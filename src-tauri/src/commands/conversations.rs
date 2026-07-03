use super::AppState;
use crate::storage::conversation_store::{
    ContextMode, NewSession, Session, SessionId, SessionSummary, SessionWithMessages,
};
use tauri::State;
use uuid::Uuid;

pub async fn list_conversations_impl(
    state: State<'_, AppState>,
) -> Result<Vec<SessionSummary>, String> {
    state.store.list_sessions().await.map_err(|e| e.to_string())
}

pub async fn get_conversation_impl(
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

pub async fn create_conversation_impl(
    state: State<'_, AppState>,
    title: String,
    context_mode: Option<String>,
) -> Result<Session, String> {
    let context_mode = match context_mode {
        Some(s) => Some(
            ContextMode::from_str(&s).ok_or_else(|| format!("Invalid context mode: {}", s))?,
        ),
        None => None,
    };
    state
        .store
        .create_session(NewSession { title, context_mode })
        .await
        .map_err(|e| e.to_string())
}

pub async fn delete_conversation_impl(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let session_id = SessionId::parse_str(&id).map_err(|e| format!("Invalid session ID: {}", e))?;
    state
        .store
        .delete_session(session_id)
        .await
        .map_err(|e| e.to_string())
}

pub fn sanitize_conversation_title(title: String) -> Result<String, String> {
    let trimmed = title.trim();
    if trimmed.is_empty() {
        return Err("Conversation title cannot be empty".to_string());
    }
    let sanitized: String = trimmed.chars().take(120).collect();
    Ok(sanitized)
}

pub async fn rename_conversation_impl(
    state: State<'_, AppState>,
    id: String,
    title: String,
) -> Result<(), String> {
    let session_id = SessionId::parse_str(&id).map_err(|e| format!("Invalid session ID: {}", e))?;
    let sanitized = sanitize_conversation_title(title)?;
    state
        .store
        .update_session_title(session_id, sanitized)
        .await
        .map_err(|e| e.to_string())
}

pub async fn truncate_messages_from_impl(
    state: State<'_, AppState>,
    session_id: String,
    message_id: String,
) -> Result<(), String> {
    let sid = SessionId::parse_str(&session_id).map_err(|e| format!("Invalid session ID: {}", e))?;
    let mid = Uuid::parse_str(&message_id).map_err(|e| format!("Invalid message ID: {}", e))?;
    state
        .store
        .truncate_messages_from(sid, mid)
        .await
        .map_err(|e| e.to_string())
}

pub async fn set_message_blocks_impl(
    state: State<'_, AppState>,
    message_id: String,
    blocks_json: String,
) -> Result<(), String> {
    let mid = Uuid::parse_str(&message_id).map_err(|e| format!("Invalid message ID: {}", e))?;
    state
        .store
        .set_message_blocks(mid, blocks_json)
        .await
        .map_err(|e| e.to_string())
}
