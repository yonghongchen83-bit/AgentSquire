//! Manual, network-hitting end-to-end harness for sa-5 (ask_user response-field
//! loop). Not a unit test (needs a real configured LLM provider + network
//! access) and not wired into `cargo test` — run explicitly:
//!
//!   cargo run --example ask_user_e2e
//!
//! Drives the exact same adapter/orchestration code paths `streaming_cmd.rs`
//! uses (SquireContextAdapter::build_turn_input -> provider.chat() -> tool
//! loop -> finalize_turn), outside of Tauri/IPC, so it can run headlessly in
//! a sandboxed CLI environment with no GUI. Simulates the frontend's role in
//! the pause/resume loop: when finalize_turn returns TurnOutcome::AskUser,
//! this harness itself supplies a canned answer (standing in for a human
//! typing into the new chat-panel.tsx inline UI) and confirms the turn
//! resumes and eventually closes.
//!
//! See `.AiControl/root/Squire/ask-user-loop/state.md` for the real run's
//! transcript/outcome from this session.

use squirecli_lib::agent::context_adapter::{ContextManagerAdapter, TurnOutcome};
use squirecli_lib::agent::squire::{InMemorySquireStore, SquireContextAdapter};
use squirecli_lib::llm::openai::OpenAIProvider;
use squirecli_lib::llm::provider::{ChatMessage, ChatRequest, ChatRole, FinishReason, LlmProvider, StreamEvent};
use squirecli_lib::storage::conversation_store::{
    ContextMode, ConversationStore, Message, MessageRole, NewMessage, NewSession, Session,
    SessionId, SessionSummary, SessionWithMessages, StoreError,
};
use std::sync::{Arc, Mutex};

struct RecordingStore {
    appended: Mutex<Vec<NewMessage>>,
}

#[async_trait::async_trait]
impl ConversationStore for RecordingStore {
    async fn create_session(&self, _session: NewSession) -> Result<Session, StoreError> {
        unimplemented!()
    }
    async fn append_message(&self, msg: NewMessage) -> Result<Message, StoreError> {
        println!(
            "[persisted message] role={:?} content={:?}",
            msg.role, msg.content
        );
        let stored = Message {
            id: uuid::Uuid::new_v4(),
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
    async fn list_sessions(&self) -> Result<Vec<SessionSummary>, StoreError> {
        unimplemented!()
    }
    async fn update_session_title(&self, _id: SessionId, _title: String) -> Result<(), StoreError> {
        unimplemented!()
    }
    async fn delete_session(&self, _id: SessionId) -> Result<(), StoreError> {
        unimplemented!()
    }
    async fn truncate_messages_from(&self, _session_id: SessionId, _message_id: uuid::Uuid) -> Result<(), StoreError> {
        unimplemented!()
    }
    async fn set_message_blocks(&self, _message_id: uuid::Uuid, _blocks_json: String) -> Result<(), StoreError> {
        unimplemented!()
    }
}

fn fixture_session(user_text: &str) -> SessionWithMessages {
    SessionWithMessages {
        session: Session {
            id: uuid::Uuid::new_v4(),
            title: "sa-5 e2e".to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            context_mode: ContextMode::Squire,
        },
        messages: vec![Message {
            id: uuid::Uuid::new_v4(),
            session_id: uuid::Uuid::new_v4(),
            role: MessageRole::User,
            content: user_text.to_string(),
            created_at: chrono::Utc::now(),
            blocks_json: None,
            thinking_content: None,
        }],
    }
}

/// Canned "human" answers this harness supplies when the model asks a
/// question, standing in for what a person would type into the new
/// chat-panel.tsx inline ask_user UI. Cycles through in order if the model
/// asks more than once.
fn canned_answer(question_number: usize) -> &'static str {
    match question_number {
        0 => "Sydney, Australia.",
        _ => "Yes, that's correct, thank you.",
    }
}

#[tokio::main]
async fn main() {
    let api_key = std::env::var("SQUIRE_E2E_API_KEY")
        .expect("set SQUIRE_E2E_API_KEY to a valid OpenAI-compatible API key");
    let base_url = std::env::var("SQUIRE_E2E_BASE_URL")
        .unwrap_or_else(|_| "https://opencode.ai/zen/v1".to_string());
    let model = std::env::var("SQUIRE_E2E_MODEL")
        .unwrap_or_else(|_| "deepseek-v4-flash-free".to_string());

    let provider = OpenAIProvider::new(api_key, model.clone(), Some(base_url));
    let store = Arc::new(InMemorySquireStore::new());
    let mut adapter = SquireContextAdapter::new(store.clone());
    let conv_store = RecordingStore {
        appended: Mutex::new(Vec::new()),
    };

    let session = fixture_session(
        "Before you answer anything else, you must ask me one clarifying question about \
         which city I'm asking about, using the ask_user field, before you write any content. \
         Do not answer until you have asked and I have replied.",
    );
    let sid = session.session.id;

    let turn_input = adapter
        .build_turn_input(&session, &[])
        .await
        .expect("build_turn_input should succeed");
    let mut messages: Vec<ChatMessage> = turn_input.messages;
    let tools = turn_input.tools;

    let mut questions_asked = 0usize;
    let mut saw_ask_user_outcome = false;
    let mut turn_closed = false;

    for round in 0..6 {
        println!("\n===== round {round} — sending {} messages =====", messages.len());
        let request = ChatRequest {
            model: model.clone(),
            messages: messages.clone(),
            tools: tools.clone(),
            thinking_level: None,
            temperature: None,
            max_tokens: None,
        };

        let mut stream = provider.chat(request).await.expect("provider.chat should succeed");
        let mut full_response = String::new();
        let mut finish_reason = None;

        while let Some(event) = stream.recv().await {
            match event {
                StreamEvent::Chunk(text) => {
                    full_response.push_str(&text);
                }
                StreamEvent::Thinking(_) => {}
                StreamEvent::ToolCall(tc) => {
                    println!("[unexpected tool call in Squire mode] {:?}", tc);
                }
                StreamEvent::Log(msg) => println!("[provider log] {msg}"),
                StreamEvent::Done(reason) => {
                    finish_reason = Some(reason);
                    break;
                }
                StreamEvent::Error(err) => {
                    panic!("provider stream error: {err}");
                }
            }
        }

        println!("[raw model response]\n{full_response}");
        println!("[finish reason] {:?}", finish_reason);

        match finish_reason {
            Some(FinishReason::Stop) | Some(FinishReason::Length) => {
                let raw_assistant_content = full_response.clone();
                match adapter
                    .finalize_turn(sid, full_response, None, &mut messages, &conv_store)
                    .await
                    .expect("finalize_turn should not Err for sa-5's ask_user path")
                {
                    TurnOutcome::Done => {
                        println!("\n>>> Turn closed normally (TurnOutcome::Done).");
                        turn_closed = true;
                        break;
                    }
                    TurnOutcome::Retry => {
                        println!("\n>>> Compliance rejection, retrying (TurnOutcome::Retry).");
                        continue;
                    }
                    TurnOutcome::Failed { reason, failed_content } => {
                        println!("\n>>> Turn FAILED: {reason}\nfailed_content={failed_content}");
                        break;
                    }
                    TurnOutcome::AskUser { question } => {
                        saw_ask_user_outcome = true;
                        println!(
                            "\n>>> TurnOutcome::AskUser received. Question surfaced to (simulated) user:\n    \"{question}\""
                        );
                        let answer = canned_answer(questions_asked);
                        questions_asked += 1;
                        println!(">>> Simulated user answers: \"{answer}\"");

                        // Mirrors streaming_cmd.rs's resume handling exactly.
                        messages.push(ChatMessage {
                            role: ChatRole::Assistant,
                            content: raw_assistant_content,
                            tool_call_id: None,
                            tool_calls: None,
                            reasoning_content: None,
                        });
                        messages.push(ChatMessage {
                            role: ChatRole::User,
                            content: serde_json::json!({ "user_answer": answer }).to_string(),
                            tool_call_id: None,
                            tool_calls: None,
                            reasoning_content: None,
                        });
                        continue;
                    }
                    TurnOutcome::Phase2 { .. } => {
                        println!("\n>>> TurnOutcome::Phase2 (two-phase protocol) — not exercised in ask_user e2e.");
                        break;
                    }
                    TurnOutcome::Phase2Done { .. } => {
                        println!("\n>>> TurnOutcome::Phase2Done — not exercised in ask_user e2e.");
                        break;
                    }
                }
            }
            other => {
                println!("[unhandled finish reason in this harness] {:?}", other);
                break;
            }
        }
    }

    println!("\n===== summary =====");
    println!("saw_ask_user_outcome = {saw_ask_user_outcome}");
    println!("questions_asked = {questions_asked}");
    println!("turn_closed = {turn_closed}");
    println!("persisted messages = {}", conv_store.appended.lock().unwrap().len());

    if !saw_ask_user_outcome {
        eprintln!(
            "\nWARNING: the model never populated ask_user in this run — either the prompt \
             needs adjusting or the model didn't follow the instruction. This harness cannot \
             force ask_user to fire; it can only confirm the code path handles it correctly \
             when it does."
        );
    }
}
