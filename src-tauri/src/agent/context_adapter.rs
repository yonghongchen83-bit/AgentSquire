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

/// Result of `finalize_turn`: whether orchestration should treat the turn as
/// closed, or loop back into `provider.chat()` because the adapter rejected
/// the model's output and wants to give it another attempt (see Squire Q6).
pub enum TurnOutcome {
    /// Turn closed normally; nothing more to send to the provider.
    Done,
    /// Adapter appended a continuation message (e.g. a rejection payload) to
    /// `messages` and wants orchestration to call `provider.chat()` again.
    Retry,
    /// Adapter exhausted its retry budget. `reason` and `failed_content` are
    /// surfaced to the user as a compliance-failure error.
    Failed {
        reason: String,
        failed_content: String,
    },
    /// Adapter's response asked a clarifying question instead of closing the
    /// turn (Squire spec §8.2/§9.3's response-field AskUser loop). Not an
    /// error — a valid, expected turn state. Orchestration is responsible
    /// for pausing the turn, surfacing `question` to the user, collecting an
    /// answer, appending both to `messages`, and resuming (see
    /// `commands::streaming_cmd`'s `TurnOutcome::AskUser` handling and
    /// `.AiControl/root/Squire/ask-user-loop/decisions.md`).
    AskUser { question: String },
    /// Phase 1 response received — orchestrator should make a Phase 2 LLM
    /// call with the Phase 2 system prompt, feeding it the original user
    /// request and Phase 1 response text so the model can generate
    /// referential tokens, concept tokens, and relationships in a separate
    /// no-tools pass (two-phase Squire protocol).
    Phase2 {
        /// The Phase 1 response text containing bookmarks and spans.
        phase1_content: String,
        /// The original user request text with chunk bookmark markers.
        user_request: String,
    },
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
    /// The tool_call assistant message itself is pushed by the
    /// `FinishReason::ToolCalls` handler in streaming_cmd.rs (which includes
    /// content + reasoning_content + all tool_calls in ONE message), so
    /// this adapter method only needs to push the tool result(s).
    async fn handle_tool_loop_step(
        &mut self,
        tool_call: &ToolCall,
        result: &ToolResult,
        messages: &mut Vec<ChatMessage>,
    ) -> Result<(), String>;

    /// Called once when the turn reaches a terminal Stop/Length state.
    /// `messages` is passed mutably so an adapter can append a continuation
    /// message when returning `TurnOutcome::Retry`.
    async fn finalize_turn(
        &mut self,
        session_id: SessionId,
        assistant_content: String,
        thinking: Option<String>,
        messages: &mut Vec<ChatMessage>,
        store: &dyn ConversationStore,
    ) -> Result<TurnOutcome, String>;

    /// Optional hook for Phase 2 initialization. Called by the orchestrator
    /// when `TurnOutcome::Phase2` is returned. Default is a no-op.
    /// `SquireContextAdapter` overrides this to switch to Phase 2 mode
    /// with the given user request text.
    fn set_phase2(&mut self, _user_request: String) {}
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
        messages: &mut Vec<ChatMessage>,
    ) -> Result<(), String> {
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
        _messages: &mut Vec<ChatMessage>,
        store: &dyn ConversationStore,
    ) -> Result<TurnOutcome, String> {
        if assistant_content.is_empty() {
            return Ok(TurnOutcome::Done);
        }

        store
            .append_message(NewMessage {
                session_id,
                role: MessageRole::Assistant,
                content: assistant_content,
                thinking_content: thinking,
            })
            .await
            .map(|_| TurnOutcome::Done)
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
#[path = "context_adapter_test.rs"]
mod tests;
