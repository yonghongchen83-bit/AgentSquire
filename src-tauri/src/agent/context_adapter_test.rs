use super::*;
use crate::storage::conversation_store::{ContextMode, Message, Session, StoreError};
use std::sync::Mutex;
use uuid::Uuid;

fn fixture_session(messages: Vec<Message>) -> SessionWithMessages {
    SessionWithMessages {
        session: Session {
            id: Uuid::new_v4(),
            title: "Test".to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            context_mode: ContextMode::Legacy,
        },
        messages,
    }
}

fn fixture_message(role: MessageRole, content: &str, thinking: Option<&str>) -> Message {
    Message {
        id: Uuid::new_v4(),
        session_id: Uuid::new_v4(),
        role,
        content: content.to_string(),
        created_at: chrono::Utc::now(),
        blocks_json: None,
        thinking_content: thinking.map(|s| s.to_string()),
    }
}

#[tokio::test]
async fn build_turn_input_replays_full_history_unfiltered() {
    let session = fixture_session(vec![
        fixture_message(MessageRole::System, "sys prompt", None),
        fixture_message(MessageRole::User, "hello", None),
        fixture_message(MessageRole::Assistant, "hi there", Some("thinking...")),
    ]);
    let base_tools = vec![ToolDefinition {
        name: "read_file".to_string(),
        description: "reads a file".to_string(),
        input_schema: serde_json::json!({}),
    }];

    let mut adapter = LegacyContextAdapter;
    let turn_input = adapter
        .build_turn_input(&session, &base_tools)
        .await
        .unwrap();

    assert_eq!(turn_input.messages.len(), 3);
    assert!(matches!(turn_input.messages[0].role, ChatRole::System));
    assert!(matches!(turn_input.messages[1].role, ChatRole::User));
    assert!(matches!(turn_input.messages[2].role, ChatRole::Assistant));
    assert_eq!(turn_input.messages[2].content, "hi there");
    assert_eq!(
        turn_input.messages[2].reasoning_content.as_deref(),
        Some("thinking...")
    );
    assert_eq!(turn_input.tools.len(), 1);
    assert_eq!(turn_input.tools[0].name, "read_file");
}

#[tokio::test]
async fn handle_tool_loop_step_pushes_assistant_and_tool_messages() {
    let mut adapter = LegacyContextAdapter;
    let mut messages: Vec<ChatMessage> = Vec::new();
    let tool_call = ToolCall {
        id: "call-1".to_string(),
        name: "read_file".to_string(),
        arguments: serde_json::json!({"path": "a.txt"}),
    };
    let result = ToolResult {
        call_id: "call-1".to_string(),
        output: "file contents".to_string(),
        is_error: false,
    };

    adapter
        .handle_tool_loop_step(&tool_call, &result, &mut messages)
        .await
        .unwrap();

    assert_eq!(messages.len(), 1);
    assert!(matches!(messages[0].role, ChatRole::Tool));
    assert_eq!(messages[0].content, "file contents");
    assert_eq!(messages[0].tool_call_id.as_deref(), Some("call-1"));
    assert!(messages[0].tool_calls.is_none());
}

#[tokio::test]
async fn handle_tool_loop_step_carries_error_output_as_content() {
    let mut adapter = LegacyContextAdapter;
    let mut messages: Vec<ChatMessage> = Vec::new();
    let tool_call = ToolCall {
        id: "call-2".to_string(),
        name: "run_terminal".to_string(),
        arguments: serde_json::json!({}),
    };
    let result = ToolResult {
        call_id: "call-2".to_string(),
        output: "Tool call 'run_terminal' was rejected by user".to_string(),
        is_error: true,
    };

    adapter
        .handle_tool_loop_step(&tool_call, &result, &mut messages)
        .await
        .unwrap();

    assert_eq!(messages[0].content, "Tool call 'run_terminal' was rejected by user");
}

struct RecordingStore {
    appended: Mutex<Vec<NewMessage>>,
}

#[async_trait]
impl ConversationStore for RecordingStore {
    async fn create_session(
        &self,
        _session: crate::storage::conversation_store::NewSession,
    ) -> Result<Session, StoreError> {
        unimplemented!()
    }
    async fn append_message(&self, msg: NewMessage) -> Result<Message, StoreError> {
        let stored = Message {
            id: Uuid::new_v4(),
            session_id: msg.session_id,
            role: msg.role.clone(),
            content: msg.content.clone(),
            created_at: chrono::Utc::now(),
            blocks_json: None,
            thinking_content: msg.thinking_content.clone(),
        };
        self.appended.lock().unwrap().push(msg);
        Ok(stored)
    }
    async fn get_session(&self, _id: SessionId) -> Result<SessionWithMessages, StoreError> {
        unimplemented!()
    }
    async fn list_sessions(
        &self,
    ) -> Result<Vec<crate::storage::conversation_store::SessionSummary>, StoreError> {
        unimplemented!()
    }
    async fn update_session_title(
        &self,
        _id: SessionId,
        _title: String,
    ) -> Result<(), StoreError> {
        unimplemented!()
    }
    async fn delete_session(&self, _id: SessionId) -> Result<(), StoreError> {
        unimplemented!()
    }
    async fn truncate_messages_from(
        &self,
        _session_id: SessionId,
        _message_id: Uuid,
    ) -> Result<(), StoreError> {
        unimplemented!()
    }
    async fn set_message_blocks(
        &self,
        _message_id: Uuid,
        _blocks_json: String,
    ) -> Result<(), StoreError> {
        unimplemented!()
    }
}

#[tokio::test]
async fn finalize_turn_skips_persistence_when_content_empty() {
    let mut adapter = LegacyContextAdapter;
    let store = RecordingStore {
        appended: Mutex::new(Vec::new()),
    };
    let mut messages = Vec::new();

    adapter
        .finalize_turn(Uuid::new_v4(), String::new(), None, &mut messages, &store)
        .await
        .unwrap();

    assert!(store.appended.lock().unwrap().is_empty());
}

#[tokio::test]
async fn finalize_turn_persists_non_empty_content_with_thinking() {
    let mut adapter = LegacyContextAdapter;
    let store = RecordingStore {
        appended: Mutex::new(Vec::new()),
    };
    let sid = Uuid::new_v4();
    let mut messages = Vec::new();

    adapter
        .finalize_turn(
            sid,
            "final answer".to_string(),
            Some("chain of thought".to_string()),
            &mut messages,
            &store,
        )
        .await
        .unwrap();

    let appended = store.appended.lock().unwrap();
    assert_eq!(appended.len(), 1);
    assert_eq!(appended[0].session_id, sid);
    assert_eq!(appended[0].content, "final answer");
    assert_eq!(
        appended[0].thinking_content.as_deref(),
        Some("chain of thought")
    );
    assert!(matches!(appended[0].role, MessageRole::Assistant));
}
