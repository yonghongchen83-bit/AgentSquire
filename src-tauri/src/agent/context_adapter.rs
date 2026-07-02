use async_trait::async_trait;

use crate::llm::provider::{ChatMessage, ChatRole, ToolCall, ToolDefinition};
use crate::storage::conversation_store::{
    ConversationStore, MessageRole, NewMessage, SessionId, SessionWithMessages,
};

use super::ToolResult;

/// Output of `build_turn_input`: the message history and tool surface to
/// send to the LLM provider for this turn.
pub struct TurnInput {
    pub messages: Vec<ChatMessage>,
    pub tools: Vec<ToolDefinition>,
}

/// Pluggable per-session context strategy. Orchestration (provider calls,
/// streaming, tool approval/watchdog, MCP discovery) stays in
/// `commands::streaming_cmd`; adapters own only history assembly,
/// per-tool-call bookkeeping, and turn-close persistence.
#[async_trait]
pub trait ContextManagerAdapter: Send + Sync {
    /// Called once per user turn, before the first `provider.chat()` call.
    async fn build_turn_input(
        &mut self,
        session: &SessionWithMessages,
        base_tools: &[ToolDefinition],
    ) -> Result<TurnInput, String>;

    /// Called once per tool call after it has been executed (and approved,
    /// if destructive), before looping back into `provider.chat()`.
    async fn handle_tool_loop_step(
        &mut self,
        tool_call: &ToolCall,
        result: &ToolResult,
        reasoning: Option<String>,
        messages: &mut Vec<ChatMessage>,
    ) -> Result<(), String>;

    /// Called once when the turn reaches a terminal Stop/Length state.
    async fn finalize_turn(
        &mut self,
        session_id: SessionId,
        assistant_content: String,
        thinking: Option<String>,
        store: &dyn ConversationStore,
    ) -> Result<(), String>;
}

fn to_chat_role(role: &MessageRole) -> ChatRole {
    match role {
        MessageRole::User => ChatRole::User,
        MessageRole::Assistant => ChatRole::Assistant,
        MessageRole::System => ChatRole::System,
    }
}

/// Full conversation-history replay — behavior identical to the pre-adapter
/// inline implementation in `send_message_impl`.
pub struct LegacyContextAdapter;

#[async_trait]
impl ContextManagerAdapter for LegacyContextAdapter {
    async fn build_turn_input(
        &mut self,
        session: &SessionWithMessages,
        base_tools: &[ToolDefinition],
    ) -> Result<TurnInput, String> {
        let messages = session
            .messages
            .iter()
            .map(|m| ChatMessage {
                role: to_chat_role(&m.role),
                content: m.content.clone(),
                tool_call_id: None,
                tool_calls: None,
                reasoning_content: m.thinking_content.clone(),
            })
            .collect();

        Ok(TurnInput {
            messages,
            tools: base_tools.to_vec(),
        })
    }

    async fn handle_tool_loop_step(
        &mut self,
        tool_call: &ToolCall,
        result: &ToolResult,
        reasoning: Option<String>,
        messages: &mut Vec<ChatMessage>,
    ) -> Result<(), String> {
        messages.push(ChatMessage {
            role: ChatRole::Assistant,
            content: String::new(),
            tool_call_id: Some(tool_call.id.clone()),
            tool_calls: Some(vec![ToolCall {
                id: tool_call.id.clone(),
                name: tool_call.name.clone(),
                arguments: tool_call.arguments.clone(),
            }]),
            reasoning_content: reasoning,
        });

        messages.push(ChatMessage {
            role: ChatRole::Tool,
            content: result.output.clone(),
            tool_call_id: Some(tool_call.id.clone()),
            tool_calls: None,
            reasoning_content: None,
        });

        Ok(())
    }

    async fn finalize_turn(
        &mut self,
        session_id: SessionId,
        assistant_content: String,
        thinking: Option<String>,
        store: &dyn ConversationStore,
    ) -> Result<(), String> {
        if assistant_content.is_empty() {
            return Ok(());
        }

        store
            .append_message(NewMessage {
                session_id,
                role: MessageRole::Assistant,
                content: assistant_content,
                thinking_content: thinking,
            })
            .await
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
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
            .handle_tool_loop_step(
                &tool_call,
                &result,
                Some("reasoning text".to_string()),
                &mut messages,
            )
            .await
            .unwrap();

        assert_eq!(messages.len(), 2);
        assert!(matches!(messages[0].role, ChatRole::Assistant));
        assert_eq!(messages[0].tool_call_id.as_deref(), Some("call-1"));
        assert_eq!(
            messages[0].reasoning_content.as_deref(),
            Some("reasoning text")
        );
        let pushed_call = messages[0].tool_calls.as_ref().unwrap();
        assert_eq!(pushed_call[0].name, "read_file");
        assert_eq!(pushed_call[0].arguments, serde_json::json!({"path": "a.txt"}));

        assert!(matches!(messages[1].role, ChatRole::Tool));
        assert_eq!(messages[1].content, "file contents");
        assert_eq!(messages[1].tool_call_id.as_deref(), Some("call-1"));
        assert!(messages[1].tool_calls.is_none());
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
            .handle_tool_loop_step(&tool_call, &result, None, &mut messages)
            .await
            .unwrap();

        assert_eq!(messages[1].content, "Tool call 'run_terminal' was rejected by user");
        assert!(messages[0].reasoning_content.is_none());
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

        adapter
            .finalize_turn(Uuid::new_v4(), String::new(), None, &store)
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

        adapter
            .finalize_turn(
                sid,
                "final answer".to_string(),
                Some("chain of thought".to_string()),
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
}
