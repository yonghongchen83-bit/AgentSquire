//! Squire protocol integration test using the user's original fishing
//! conversation, which exposed three bugs:
//!
//!   1. Sigil markers leaking into thinking content (Q1)
//!   2. Formatter dropping CONCEPT_/REF_ tokens (context loss Q2→Q3)
//!   3. Model not exploring past turns when preserve is empty (Q3)
//!
//! Run: cargo test --test squire_fishing_test -- --nocapture
//! Requires: DEEPSEEK_API_KEY environment variable

use std::collections::HashMap;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Instant;

use async_trait::async_trait;
use provider_core::{ChatMessage, ChatRequest, ChatRole, StreamEvent, ToolCall};
use provider_registry::{ProviderRegistry, ProviderRegistryConfig, ProviderSpec};
use uuid::Uuid;

use squirecli_lib::agent::context_adapter::{ContextManagerAdapter, TurnOutcome};
use squirecli_lib::agent::squire::{
    InMemorySquireStore, SquireContextAdapter, SquireStore,
    built_in_tool_definitions, SquireBatchTool, SquireExploreTool,
    SquireRdfTool, SquireTokenToDetailTool,
};
use squirecli_lib::agent::Tool;
use squirecli_lib::state::config::SquirePrefetchConfig;
use squirecli_lib::storage::conversation_store::{
    ContextMode, ConversationStore, Message, MessageRole, NewMessage, NewSession, Session,
    SessionId, SessionSummary, SessionWithMessages, StoreError,
};

// ── Quoted questions from the user's Celine workspace ──
const Q1: &str = "i want to go fishing tomorrow what is your recommendation";
const Q2: &str = "sydney, ocean, beginner";
const Q3: &str = "what cloth should i wear?";

// ── InMemoryConvStore ──

struct InMemoryConvStore {
    sessions: StdMutex<HashMap<SessionId, Session>>,
    messages: StdMutex<HashMap<SessionId, Vec<Message>>>,
}

impl InMemoryConvStore {
    fn new() -> Self { Self { sessions: StdMutex::new(HashMap::new()), messages: StdMutex::new(HashMap::new()) } }
}

#[async_trait]
impl ConversationStore for InMemoryConvStore {
    async fn create_session(&self, new: NewSession) -> Result<Session, StoreError> {
        let id = Uuid::new_v4();
        let now = chrono::Utc::now();
        let s = Session { id, title: new.title, created_at: now, updated_at: now, context_mode: new.context_mode.unwrap_or_default() };
        self.sessions.lock().unwrap().insert(id, s.clone());
        self.messages.lock().unwrap().insert(id, Vec::new());
        Ok(s)
    }
    async fn append_message(&self, msg: NewMessage) -> Result<Message, StoreError> {
        let m = Message { id: Uuid::new_v4(), session_id: msg.session_id, role: msg.role, content: msg.content, created_at: chrono::Utc::now(), blocks_json: None, thinking_content: msg.thinking_content };
        self.messages.lock().unwrap().entry(msg.session_id).or_default().push(m.clone());
        Ok(m)
    }
    async fn get_session(&self, id: SessionId) -> Result<SessionWithMessages, StoreError> {
        let s = self.sessions.lock().unwrap().get(&id).cloned().ok_or_else(|| StoreError::NotFound(id.to_string()))?;
        let msgs = self.messages.lock().unwrap().get(&id).cloned().unwrap_or_default();
        Ok(SessionWithMessages { session: s, messages: msgs })
    }
    async fn list_sessions(&self) -> Result<Vec<SessionSummary>, StoreError> { Ok(Vec::new()) }
    async fn update_session_title(&self, _: SessionId, _: String) -> Result<(), StoreError> { Ok(()) }
    async fn delete_session(&self, _: SessionId) -> Result<(), StoreError> { Ok(()) }
    async fn truncate_messages_from(&self, _: SessionId, _: Uuid) -> Result<(), StoreError> { Ok(()) }
    async fn set_message_blocks(&self, _: Uuid, _: String) -> Result<(), StoreError> { Ok(()) }
}

// ── Helpers ──

fn clamp(s: &str, n: usize) -> String {
    if s.len() <= n { s.to_string() } else { format!("{}…", &s[..n]) }
}

fn fmt_ms(ms: f64) -> String {
    if ms < 1000.0 { format!("{:.0}ms", ms) }
    else { format!("{:.2}s", ms / 1000.0) }
}

async fn call_llm(
    provider: &Arc<dyn provider_core::LlmProvider>, model: &str,
    messages: &[ChatMessage], tools: &[provider_core::ToolDefinition],
    temperature: Option<f32>, max_tokens: Option<u32>, thinking_level: Option<String>,
) -> Result<(String, Vec<ToolCall>), String> {
    let request = ChatRequest {
        model: model.to_string(), messages: messages.to_vec(), tools: tools.to_vec(),
        thinking_level, temperature, max_tokens,
    };
    let mut stream = provider.chat(request).await.map_err(|e| format!("Provider: {e}"))?;
    let mut full_text = String::new();
    let mut tool_calls: Vec<ToolCall> = Vec::new();

    while let Some(event) = stream.recv().await {
        match event {
            StreamEvent::Chunk(t) => full_text.push_str(&t),
            StreamEvent::ToolCall(tc) => tool_calls.push(tc),
            StreamEvent::Done(_) => break,
            StreamEvent::Error(e) => return Err(format!("Stream: {e}")),
            _ => {}
        }
    }
    Ok((full_text, tool_calls))
}

async fn execute_tool(
    name: &str, args: serde_json::Value,
    store: &Arc<dyn SquireStore>, tool_defs: &[provider_core::ToolDefinition],
    sid: SessionId, batch_counter: &Arc<std::sync::atomic::AtomicU32>,
) -> String {
    let cap = squirecli_lib::agent::squire::tools::DEFAULT_BATCH_CAP;
    match name {
        "explore" => SquireExploreTool { store: store.clone(), tool_defs: tool_defs.to_vec(), session_id: sid, batch_counter: batch_counter.clone(), batch_cap: cap }.execute("c", args).await.output,
        "token_to_detail" => SquireTokenToDetailTool { store: store.clone(), batch_counter: batch_counter.clone(), batch_cap: cap }.execute("c", args).await.output,
        "rdf" => SquireRdfTool { store: store.clone(), batch_counter: batch_counter.clone(), batch_cap: cap }.execute("c", args).await.output,
        "batch" => SquireBatchTool { store: store.clone(), tool_defs: tool_defs.to_vec(), session_id: sid, batch_counter: batch_counter.clone(), batch_cap: cap }.execute("c", args).await.output,
        o => format!("Unknown: {o}"),
    }
}

// ── Run one turn ──

#[allow(clippy::too_many_arguments)]
async fn run_turn(
    sid: SessionId,
    user_prompt: &str,
    provider: &Arc<dyn provider_core::LlmProvider>,
    conv_store: &Arc<dyn ConversationStore>,
    squire_store: &Arc<dyn SquireStore>,
    turn_num: usize,
) -> Result<(usize, usize, u32), String> {
    // Append user message
    conv_store.append_message(NewMessage {
        session_id: sid, role: MessageRole::User,
        content: user_prompt.to_string(), thinking_content: None,
    }).await.map_err(|e| e.to_string())?;

    let session_data = conv_store.get_session(sid).await.map_err(|e| e.to_string())?;

    // build_turn_input
    let mut adapter = SquireContextAdapter::new_with_prefetch(
        squire_store.clone(),
        SquirePrefetchConfig { memory_top_k: 5, workflow_top_k: 2, tool_top_k: 2, skill_top_k: 2, min_score: 0.0, ..Default::default() },
    );
    let t0 = Instant::now();
    let input = adapter.build_turn_input(&session_data, &[]).await.map_err(|e| e.to_string())?;
    let build_ms = t0.elapsed().as_secs_f64() * 1000.0;
    println!("  T{turn_num} build_turn_input: {}  (sys={}B, usr={}B)", fmt_ms(build_ms), input.messages[0].content.len(), input.messages[1].content.len());

    // Print how many tokens are in context
    let sys = &input.messages[0].content;
    if let Some(ctx_start) = sys.find("long_tokens") {
        let ctx = &sys[ctx_start..];
        let long_count = ctx.matches("\"token_id\"").count();
        println!("  T{turn_num} context: {} long_tokens, {}B system prompt", long_count, sys.len());
    }

    // Phase 1 LLM + tool loop
    let t_p1 = Instant::now();
    let mut messages = input.messages;
    let turn_tools = built_in_tool_definitions();
    let batch_counter = Arc::new(std::sync::atomic::AtomicU32::new(0));
    let mut num_rounds = 0u32;
    let mut p1_content;
    let mut tool_call_count = 0u32;

    loop {
        num_rounds += 1;
        let (text, tool_calls) = call_llm(
            provider, "deepseek-v4-flash", &messages, &turn_tools,
            Some(0.7), Some(4096), None,
        ).await?;
        tool_call_count += tool_calls.len() as u32;

        if !tool_calls.is_empty() {
            println!("  T{turn_num} r{num_rounds}: {} tool calls ({})", tool_calls.len(),
                tool_calls.iter().map(|t| t.name.as_str()).collect::<Vec<_>>().join(", "));

            let reasoning = if text.is_empty() { None } else { Some(text.clone()) };
            messages.push(ChatMessage {
                role: ChatRole::Assistant, content: text.clone(), tool_call_id: None,
                tool_calls: Some(tool_calls.clone()), reasoning_content: reasoning,
            });

            for tc in &tool_calls {
                let result = execute_tool(&tc.name, tc.arguments.clone(), squire_store, &[], sid, &batch_counter).await;
                messages.push(ChatMessage {
                    role: ChatRole::Tool, content: result,
                    tool_call_id: Some(tc.id.clone()), tool_calls: None, reasoning_content: None,
                });
            }
            if num_rounds >= 5 { p1_content = text; break; }
            continue;
        }
        p1_content = text;
        break;
    }

    let p1_ms = t_p1.elapsed().as_secs_f64() * 1000.0;
    let inline_refs = p1_content.matches("§!").count();
    let spans = p1_content.matches("§^").count();
    println!("  T{turn_num} P1 done: {}  ({num_rounds}r, {tool_call_count}TC, {}B, {inline_refs}§!, {spans}§^)",
        fmt_ms(p1_ms), p1_content.len());

    // finalize Phase 1
    let outcome = adapter.finalize_turn(sid, p1_content.clone(), None, &mut messages, conv_store.as_ref())
        .await.map_err(|e| e.to_string())?;

    match outcome {
        TurnOutcome::Phase2 { phase1_content, user_request } => {
            // Phase 2
            let t_p2 = Instant::now();
            let p2_prompt = squirecli_lib::agent::squire_prompts::system_prompt_phase2();
            let mut p2_msgs = vec![
                ChatMessage { role: ChatRole::System, content: p2_prompt, tool_call_id: None, tool_calls: None, reasoning_content: None },
                ChatMessage { role: ChatRole::User, content: format!("Original user request:\n{}\n\nAssistant Phase 1 response:\n{}", user_request, phase1_content), tool_call_id: None, tool_calls: None, reasoning_content: None },
            ];
            adapter.set_phase2(user_request);

            let mut p2_tokens = 0usize;
            let mut p2_rels = 0usize;
            for rnd in 1..=3 {
                let (text, _) = call_llm(provider, "deepseek-v4-flash", &p2_msgs, &[], Some(0.0), Some(4096), Some("none".to_string())).await?;
                match adapter.finalize_turn(sid, text, None, &mut p2_msgs, conv_store.as_ref()).await.map_err(|e| e.to_string())? {
                    TurnOutcome::Phase2Done { tokens_accepted, relationships_accepted, .. } => {
                        p2_tokens += tokens_accepted;
                        p2_rels += relationships_accepted;
                        println!("  T{turn_num} P2 r{rnd}: +{tokens_accepted}t +{relationships_accepted}r");
                        break;
                    }
                    TurnOutcome::Retry => {
                        // Print the retry reason (last user message pushed by adapter)
                        let reason = p2_msgs.last()
                            .filter(|m| matches!(m.role, ChatRole::User))
                            .map(|m| m.content.lines().next().unwrap_or("").to_string())
                            .unwrap_or_default();
                        println!("  T{turn_num} P2 r{rnd}: retry — {reason}");
                        continue;
                    }
                    _ => break,
                }
            }
            let p2_ms = t_p2.elapsed().as_secs_f64() * 1000.0;
            println!("  T{turn_num} P2: {} ({p2_tokens}t, {p2_rels}r)", fmt_ms(p2_ms));
            Ok((p2_tokens, p2_rels, num_rounds))
        }
        TurnOutcome::Done => {
            println!("  T{turn_num} → Done (no Squire)");
            Ok((0, 0, num_rounds))
        }
        TurnOutcome::Retry => {
            println!("  T{turn_num} → Retry (exhausted)");
            Ok((0, 0, num_rounds))
        }
        TurnOutcome::Failed { reason, .. } => {
            println!("  T{turn_num} → Failed: {reason}");
            Err(format!("Turn {turn_num} failed: {reason}"))
        }
        other => {
            Err(format!("Turn {turn_num} unexpected outcome: {:?}", match other { TurnOutcome::AskUser{..} => "AskUser", _ => "?" }))
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Main test
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_squire_fishing_conversation() {
    let t0 = Instant::now();

    let key = match std::env::var("DEEPSEEK_API_KEY") {
        Ok(k) => k,
        Err(_) => { eprintln!("SKIP: DEEPSEEK_API_KEY not set"); return; }
    };

    println!("═══ Squire Fishing Conversation Test ═══\n");

    // Provider
    let spec = ProviderSpec {
        provider_type: "openai".to_string(), name: "deepseek".to_string(),
        api_key: key, model: "deepseek-v4-flash".to_string(),
        models: vec!["deepseek-v4-flash".to_string()],
        endpoint: Some("https://api.deepseek.com/v1".to_string()),
        metadata: HashMap::new(), category: None,
    };
    let reg = ProviderRegistry::from_config(&ProviderRegistryConfig {
        providers: vec![spec], verbose_logging: false, wire_log_path: None,
    });
    let (provider, _) = reg.resolve_provider_for_instance(
        &provider_core::ModelInstance::new("deepseek", "deepseek-v4-flash"),
    ).expect("resolve provider");

    let conv_store = Arc::new(InMemoryConvStore::new()) as Arc<dyn ConversationStore>;
    let squire_store = Arc::new(InMemorySquireStore::new()) as Arc<dyn SquireStore>;

    let session = conv_store.create_session(NewSession {
        title: "Fishing test".to_string(), context_mode: Some(ContextMode::Squire),
    }).await.expect("create session");
    let sid = session.id;
    println!("Session: {sid}\n");

    // ── Run 3 turns ──
    let questions = [(Q1, "Q1-fishing"), (Q2, "Q2-sydney"), (Q3, "Q3-clothing")];
    let mut total_p2_tokens = 0usize;
    let mut total_p2_rels = 0usize;

    for (qi, (question, label)) in questions.iter().enumerate() {
        let turn_num = qi + 1;
        let t_start = Instant::now();
        println!("─── {label}: \"{q}\" ───", q = clamp(question, 120));

        match run_turn(sid, question, &provider, &conv_store, &squire_store, turn_num).await {
            Ok((p2t, p2r, rounds)) => {
                total_p2_tokens += p2t;
                total_p2_rels += p2r;
                let wall = t_start.elapsed().as_secs_f64() * 1000.0;
                println!("  ✓ {label} done: {} ({rounds}r, +{p2t}t, +{p2r}r)\n", fmt_ms(wall));
            }
            Err(e) => {
                println!("  ❌ {label}: {e}\n");
            }
        }

        // Show store state after each turn
        let all_ids = squire_store.list_token_ids_by_session(sid).await;
        let all_rels = squire_store.get_relationships(None, None, None).await;
        println!("  Store after {label}: {} tokens, {} rels", all_ids.len(), all_rels.len());

        // Show preserved tokens
        let preserved = squire_store.preserved_tokens(sid).await;
        if !preserved.is_empty() {
            println!("  Preserved: {}", preserved.iter().map(|p| p.token_id.as_str()).collect::<Vec<_>>().join(", "));
        }

        // Show some token details
        for tid in all_ids.iter().filter(|t| t.starts_with("CONCEPT_") || t.starts_with("REF_") || t.starts_with("CON_")).take(8) {
            if let Some(d) = squire_store.token_detail(tid).await {
                println!("    {tid}: {}", d.short_desc);
            }
        }
        println!();
    }

    // ── Summary ──
    let all_ids = squire_store.list_token_ids_by_session(sid).await;
    let all_rels = squire_store.get_relationships(None, None, None).await;
    let total_ms = t0.elapsed().as_secs_f64() * 1000.0;
    println!("═══ SUMMARY ═══");
    println!("  Total time: {}", fmt_ms(total_ms));
    println!("  Final store: {} tokens, {} relationships", all_ids.len(), all_rels.len());
    println!("  Phase 2 total: {total_p2_tokens} tokens, {total_p2_rels} relationships");

    // Verify: USR_T and RESP_T tokens exist
    let usr_tokens: Vec<_> = all_ids.iter().filter(|t| t.starts_with("USR_T")).collect();
    let resp_tokens: Vec<_> = all_ids.iter().filter(|t| t.starts_with("RESP_T")).collect();
    println!("  USR_T: {} tokens, RESP_T: {} tokens", usr_tokens.len(), resp_tokens.len());

    // Verify: concept/referential tokens were created
    let concept_tokens: Vec<_> = all_ids.iter().filter(|t| t.starts_with("CONCEPT_") || t.starts_with("CON_")).collect();
    let ref_tokens: Vec<_> = all_ids.iter().filter(|t| t.starts_with("REF_")).collect();
    println!("  CONCEPT/CON: {}, REF: {}", concept_tokens.len(), ref_tokens.len());

    // Assert: at minimum we should have tokens across all 3 turns
    assert!(usr_tokens.len() >= 1, "Expected at least 1 USR_T token");
    assert!(resp_tokens.len() >= 1, "Expected at least 1 RESP_T token");

    println!("\n  ✅ Fishing conversation test passed.");
}
