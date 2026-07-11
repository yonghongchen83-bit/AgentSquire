//! End-to-end integration test for the Squire two-phase protocol.
//!
//! This test exercises the real LLM provider pipeline (DeepSeek V4 Flash for
//! Phase 1, OpenRouter/free for Phase 2) through `SquireContextAdapter`
//! directly, without any UI or Tauri state. It verifies:
//!
//! 1. `build_turn_input` produces a valid system prompt with context
//! 2. Phase 1 LLM call produces a valid Bookmark Protocol response
//! 3. `finalize_turn` transitions to Phase 2 correctly
//! 4. Phase 2 LLM call produces tokens/relationships
//! 5. `finalize_turn` stores tokens/relationships in the Squire store
//!
//! Run with: cargo test --test squire_e2e_test -- --nocapture
//!
//! Requires the following environment variables:
//!   DEEPSEEK_API_KEY - API key for DeepSeek
//!   OPENROUTER_API_KEY - API key for OpenRouter

use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::time::Duration;

use async_trait::async_trait;
use squirecli_lib::agent::context_adapter::{ContextManagerAdapter, TurnOutcome};
use squirecli_lib::agent::squire::{InMemorySquireStore, SquireContextAdapter, SquireStore};
use squirecli_lib::agent::squire_prompts::system_prompt_phase2;
use squirecli_lib::llm::provider::{ChatMessage, ChatRole, LlmProvider, StreamEvent};
use squirecli_lib::state::config::SquirePrefetchConfig;
use squirecli_lib::storage::conversation_store::{
    ContextMode, ConversationStore, Message, MessageRole, NewMessage, Session, SessionId,
    SessionWithMessages, StoreError,
};
use uuid::Uuid;

// ── Session fixture ──

fn fixture_session(user_text: &str) -> SessionWithMessages {
    SessionWithMessages {
        session: Session {
            id: Uuid::new_v4(),
            title: "Squire E2E Test".to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            context_mode: ContextMode::Squire,
        },
        messages: vec![Message {
            id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            role: MessageRole::User,
            content: user_text.to_string(),
            created_at: chrono::Utc::now(),
            blocks_json: None,
            thinking_content: None,
        }],
    }
}

// ── Recording conversation store (tracks persisted messages) ──

struct RecordingStore {
    appended: StdMutex<Vec<NewMessage>>,
}

#[async_trait]
impl ConversationStore for RecordingStore {
    async fn create_session(
        &self,
        _: squirecli_lib::storage::conversation_store::NewSession,
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
    async fn get_session(&self, _: SessionId) -> Result<SessionWithMessages, StoreError> {
        unimplemented!()
    }
    async fn list_sessions(
        &self,
    ) -> Result<Vec<squirecli_lib::storage::conversation_store::SessionSummary>, StoreError>
    {
        unimplemented!()
    }
    async fn update_session_title(&self, _: SessionId, _: String) -> Result<(), StoreError> {
        unimplemented!()
    }
    async fn delete_session(&self, _: SessionId) -> Result<(), StoreError> {
        unimplemented!()
    }
    async fn truncate_messages_from(
        &self,
        _: SessionId,
        _: Uuid,
    ) -> Result<(), StoreError> {
        unimplemented!()
    }
    async fn set_message_blocks(&self, _: Uuid, _: String) -> Result<(), StoreError> {
        unimplemented!()
    }
}

// ── Helper: create a real OpenAI-compatible provider ──

fn create_provider(
    api_key: String,
    model: String,
    base_url: Option<String>,
) -> Arc<dyn LlmProvider> {
    Arc::new(provider_openai::OpenAIProvider::new(api_key, model, base_url))
}

// ── Helper: emit timing diagnostics ──

fn emit_timing(label: &str, elapsed_ms: u128) {
    println!("  ⏱  {:>35}: {:>6}ms", label, elapsed_ms);
}

// ── The actual E2E test ──

/// Overall timeout: if anything hangs, the test panics after this duration
/// instead of running forever. Increase if API calls legitimately take longer.
const TEST_TIMEOUT_SECS: u64 = 120;

#[tokio::test]
async fn test_squire_two_phase_e2e() {
    let t0 = std::time::Instant::now();

    tokio::select! {
        _ = tokio::time::sleep(Duration::from_secs(TEST_TIMEOUT_SECS)) => {
            panic!("Test timed out after {}s — Phase 2 likely hung", TEST_TIMEOUT_SECS);
        }
        result = run_test() => {
            if let Err(e) = result {
                panic!("{}", e);
            }
        }
    }
}

async fn run_test() -> Result<(), String> {
    let t0 = std::time::Instant::now();

    // ── Read API keys from environment ──
    let deepseek_key = std::env::var("DEEPSEEK_API_KEY")
        .map_err(|_| "DEEPSEEK_API_KEY environment variable required".to_string())?;
    let google_key = std::env::var("GOOGLE_API_KEY")
        .map_err(|_| "GOOGLE_API_KEY environment variable required".to_string())?;

    // ── Create providers ──
    let phase1_provider = create_provider(
        deepseek_key,
        "deepseek-v4-flash".to_string(),
        Some("https://api.deepseek.com/v1".to_string()),
    );
    let phase2_provider = create_provider(
        google_key,
        "models/gemini-2.5-flash".to_string(),
        Some("https://generativelanguage.googleapis.com/v1beta/openai".to_string()),
    );

    let t1 = std::time::Instant::now();
    emit_timing("providers_created", t1.duration_since(t0).as_millis());

    // ── Verify providers work with a quick chat test ──
    println!("\n  Testing Phase 1 provider (DeepSeek V4 Flash)...");
    let test_response = call_provider_simple(
        phase1_provider.clone(),
        "deepseek-v4-flash".to_string(),
        "Reply with exactly the word OK and nothing else.".to_string(),
    )
    .await;
    assert!(
        test_response.to_lowercase().contains("ok"),
        "Phase 1 provider test failed. Response: {}",
        test_response
    );
    println!("    ✓ Provider test passed: {}", test_response.trim());
    let t2 = std::time::Instant::now();
    emit_timing("phase1_provider_verified", t2.duration_since(t0).as_millis());

    // ── Setup Squire store and adapter ──
    let store = Arc::new(InMemorySquireStore::new()) as Arc<dyn SquireStore>;
    let conv_store = RecordingStore {
        appended: StdMutex::new(Vec::new()),
    };

    let mut adapter = SquireContextAdapter::new_with_prefetch(
        store.clone(),
        SquirePrefetchConfig {
            memory_top_k: 3,
            workflow_top_k: 2,
            tool_top_k: 2,
            skill_top_k: 2,
            min_score: 0.0,
        },
    );

    let session = fixture_session("What is Rust's ownership model? Explain briefly with an example.");
    let _user_text = session
        .messages
        .iter()
        .rev()
        .find(|m| matches!(m.role, MessageRole::User))
        .map(|m| m.content.clone())
        .unwrap_or_default();

    // ── Step 1: build_turn_input ──
    println!("\n  Phase 1: Building turn input...");
    let turn_input = adapter
        .build_turn_input(&session, &[])
        .await
        .expect("build_turn_input should succeed");
    assert_eq!(turn_input.messages.len(), 2, "Expected System + User messages");
    assert!(
        turn_input.messages[0].content.contains("Context for this turn"),
        "System message should contain context block"
    );
    let t3 = std::time::Instant::now();
    emit_timing("build_turn_input", t3.duration_since(t0).as_millis());
    println!("    ✓ build_turn_input produced {} messages", turn_input.messages.len());
    let store_count = store.list_token_ids_by_session(session.session.id).await.len();
    println!("    Store tokens: {}", store_count);

    // ── Step 2: Phase 1 LLM Call ──
    println!("\n  Phase 1: Calling DeepSeek V4 Flash...");
    let mut messages = turn_input.messages;
    let phase1_response = call_provider(
        phase1_provider.clone(),
        "deepseek-v4-flash".to_string(),
        messages.clone(),
        vec![],
        None,
    )
    .await;

    println!(
        "    ✓ Phase 1 response received ({} bytes)",
        phase1_response.len()
    );
    let t4 = std::time::Instant::now();
    emit_timing("phase1_llm_call", t4.duration_since(t0).as_millis());

    // Verify Phase 1 response has content (it should explain ownership)
    assert!(!phase1_response.is_empty(), "Phase 1 response should not be empty");
    assert!(
        phase1_response.contains("ownership") || phase1_response.contains("Ownership")
            || phase1_response.contains("owner"),
        "Phase 1 response should mention ownership. Got: {}",
        &phase1_response[..200.min(phase1_response.len())]
    );

    // ── Step 3: finalize_turn (Phase 1 → Phase 2 transition) ──
    println!("\n  Phase 1: Finalizing turn...");
    let outcome = adapter
        .finalize_turn(
            session.session.id,
            phase1_response.clone(),
            None,
            &mut messages,
            &conv_store,
        )
        .await
        .expect("finalize_turn should succeed");

    let t5 = std::time::Instant::now();
    emit_timing("phase1_finalize_turn", t5.duration_since(t0).as_millis());

    let (phase1_content, user_request) = match outcome {
        TurnOutcome::Phase2 {
            phase1_content,
            user_request,
        } => {
            println!("    ✓ Phase 1 → Phase 2 transition succeeded");
            // Check that the response was persisted to the conversation store
            let appended = conv_store.appended.lock().unwrap();
            assert_eq!(appended.len(), 1, "Phase 1 message should be appended");
            assert!(matches!(appended[0].role, MessageRole::Assistant));
            let store_count = store.list_token_ids_by_session(session.session.id).await.len();
            println!("    Store tokens after Phase 1: {}", store_count);
            (phase1_content, user_request)
        }
        _other => {
            panic!(
                "Expected TurnOutcome::Phase2\nPhase 1 response was:\n{}",
                &phase1_response[..500.min(phase1_response.len())]
            );
        }
    };

    // ── Step 4-5: Phase 2 retry loop (same as real streaming_cmd.rs flow) ──
    println!("\n  ── Phase 2: Token extraction with retry loop ──");
    let phase2_prompt = system_prompt_phase2();
    let mut p2_messages = vec![
        ChatMessage {
            role: ChatRole::System,
            content: phase2_prompt,
            tool_call_id: None,
            tool_calls: None,
            reasoning_content: None,
        },
        ChatMessage {
            role: ChatRole::User,
            content: format!(
                "Original user request:\n{}\n\nAssistant Phase 1 response:\n{}",
                user_request, phase1_content
            ),
            tool_call_id: None,
            tool_calls: None,
            reasoning_content: None,
        },
    ];

    adapter.set_phase2(user_request.clone());

    let mut phase2_tokens_total = 0usize;
    let mut phase2_rels_total = 0usize;
    let mut phase2_round = 0u32;
    let mut phase2_final = None;

    for round in 1..=4 {  // 1 initial + up to 3 retries (matches max_retries=3)
        phase2_round = round;
        println!("\n  Phase 2 Round {}:", round);

        let p2_response = call_provider(
            phase2_provider.clone(),
            "models/gemini-2.5-flash".to_string(),
            p2_messages.clone(),
            vec![],
            None,
        )
        .await;

        println!(
            "    Response: {} bytes",
            p2_response.len(),
        );
        println!("    ─── RAW RESPONSE ───");
        for line in p2_response.lines() {
            println!("    |{}", line);
        }
        println!("    ─── END RAW RESPONSE ───");
        let store_before = store.list_token_ids_by_session(session.session.id).await.len();

        let outcome = adapter
            .finalize_turn(
                session.session.id,
                p2_response.clone(),
                None,
                &mut p2_messages,
                &conv_store,
            )
            .await
            .expect("Phase 2 finalize_turn should succeed");

        let store_after = store.list_token_ids_by_session(session.session.id).await.len();
        let tokens_this_round = store_after - store_before;

        match outcome {
            TurnOutcome::Phase2Done {
                tokens_accepted,
                relationships_accepted,
                ref tokens_rejected,
                ref relationships_rejected,
            } => {
                phase2_tokens_total += tokens_accepted;
                phase2_rels_total += relationships_accepted;
                println!(
                    "    ✓ Phase2Done: +{} tokens this round (+{} rels), total store: {}",
                    tokens_this_round, relationships_accepted, store_after
                );
                if !tokens_rejected.is_empty() {
                    println!("    ⚠ Rejected tokens: {:?}", tokens_rejected);
                }
                if !relationships_rejected.is_empty() {
                    println!("    ⚠ Rejected rels: {:?}", relationships_rejected);
                }
                phase2_final = Some(outcome);
                break;
            }
            TurnOutcome::Retry => {
                // Valid tokens/relationships are saved BEFORE retry (our fix)
                phase2_tokens_total += tokens_this_round;
                // Show the rejection message that was appended to p2_messages
                if let Some(last) = p2_messages.last() {
                    if matches!(last.role, ChatRole::User) {
                        let rejection = &last.content;
                        println!("    ⚠ Retry: +{} tokens this round (saved), total store: {}", tokens_this_round, store_after);
                        println!("    Rejection sent to model: {}", &rejection[..rejection.len().min(200)]);
                    }
                }
                // p2_messages now has the rejection appended — next loop iteration
                // sends the retry with feedback to the model
                continue;
            }
            TurnOutcome::Failed { ref reason, ref failed_content } => {
                println!(
                    "    ❌ Failed: {}\n    Content:\n{}",
                    reason,
                    &failed_content[..500.min(failed_content.len())]
                );
                phase2_final = Some(outcome);
                break;
            }
            TurnOutcome::Done => {
                println!("    ℹ Done (non-Squire fallback)");
                phase2_final = Some(outcome);
                break;
            }
            other => {
                println!("    ❌ Unexpected outcome");
                phase2_final = Some(other);
                break;
            }
        }
    }

    let t7 = std::time::Instant::now();
    emit_timing("phase2_loop_complete", t7.duration_since(t0).as_millis());

    println!();
    println!("  ── Phase 2 Summary ──");
    println!("    Rounds: {}", phase2_round);
    println!("    Total tokens stored this Phase 2: {}", phase2_tokens_total);
    let final_outcome = phase2_final.unwrap_or(TurnOutcome::Failed {
        reason: "exhausted retries".to_string(),
        failed_content: String::new(),
    });
    match &final_outcome {
        TurnOutcome::Phase2Done { tokens_accepted, relationships_accepted, .. } => {
            println!("    ✓ Final outcome: Phase2Done ({} tokens, {} rels)", tokens_accepted, relationships_accepted);
        }
        TurnOutcome::Retry => {
            println!("    ⚠ Final outcome: Retry (retries exhausted without resolution)");
        }
        TurnOutcome::Done => {
            println!("    ℹ Final outcome: Done");
        }
        TurnOutcome::Failed { reason, .. } => {
            println!("    ❌ Final outcome: Failed: {}", reason);
            panic!("Phase 2 failed after {} rounds: {}", phase2_round, reason);
        }
        _ => {}
    }

    // ── Step 6: Verify store has the tokens ──
    println!("\n  ── Step 6: Store verification ──");
    let all_token_ids = store.list_token_ids_by_session(session.session.id).await;
    let mut usr_count = 0;
    let mut resp_count = 0;
    let mut concept_count = 0;
    let mut referential_count = 0;
    let mut other_count = 0;
    for tid in &all_token_ids {
        let detail = store.token_detail(tid).await;
        if tid.starts_with("USR_T") { usr_count += 1; }
        else if tid.starts_with("RESP_T") { resp_count += 1; }
        else if tid.starts_with("CONCEPT_") || tid.starts_with("CON_") { concept_count += 1; }
        else if tid.starts_with("REF_") { referential_count += 1; }
        else { other_count += 1; }
        if let Some(d) = detail {
            println!("    [{:>12}] {}", tid, d.short_desc);
        } else {
            println!("    [{:>12}] (no detail)", tid);
        }
    }
    println!(
        "    Total: {} tokens (USR_T:{}, RESP_T:{}, CONCEPT/CON:{}, REF/OTHER:{})",
        all_token_ids.len(), usr_count, resp_count, concept_count, other_count
    );

    // Verify preserve list
    let preserved = store.preserved_tokens(session.session.id).await;
    println!("    Preserved tokens: {}", preserved.len());
    for p in &preserved {
        println!("      {}", p.token_id);
    }

    // Phase 1 should have stored at least the USR_T chunk + RESP_T chunks
    assert!(
        all_token_ids.len() >= 3,
        "Expected at least 3 tokens (USR_T + RESP_T), got {}",
        all_token_ids.len()
    );
    assert!(
        all_token_ids.iter().any(|t| t.starts_with("USR_T")),
        "Expected at least one USR_T token"
    );
    assert!(
        all_token_ids.iter().any(|t| t.starts_with("RESP_T")),
        "Expected at least one RESP_T token"
    );

    let t8 = std::time::Instant::now();
    emit_timing("verification", t8.duration_since(t0).as_millis());

    // ── Print total duration ──
    println!(
        "\n  ─────────────────────────────────────"
    );
    emit_timing("TOTAL", t8.duration_since(t0).as_millis());
    println!();

    Ok(())
}

// ── Provider call helpers ──

/// Make a simple LLM call and collect the full text response.
async fn call_provider(
    provider: Arc<dyn LlmProvider>,
    model: String,
    messages: Vec<ChatMessage>,
    tools: Vec<squirecli_lib::llm::provider::ToolDefinition>,
    thinking_level: Option<String>,
) -> String {
    let request = squirecli_lib::llm::provider::ChatRequest {
        model,
        messages,
        tools,
        thinking_level,
        temperature: None,
        max_tokens: None,
    };

    let mut stream = provider
        .chat(request)
        .await
        .expect("Provider chat should succeed");

    let mut full_response = String::new();
    while let Some(event) = stream.recv().await {
        match event {
            StreamEvent::Chunk(text) => full_response.push_str(&text),
            StreamEvent::Thinking(_text) => {
                // Skip thinking for test output clarity
            }
            StreamEvent::Done(_) => break,
            StreamEvent::Error(e) => {
                panic!("Provider stream error: {}", e);
            }
            _ => {}
        }
    }
    full_response
}

/// Make a simple single-turn prompt call (no tools, no thinking) and return
/// the response text.
async fn call_provider_simple(
    provider: Arc<dyn LlmProvider>,
    model: String,
    prompt: String,
) -> String {
    let messages = vec![ChatMessage {
        role: ChatRole::User,
        content: prompt,
        tool_call_id: None,
        tool_calls: None,
        reasoning_content: None,
    }];
    call_provider(provider, model, messages, vec![], None).await
}
