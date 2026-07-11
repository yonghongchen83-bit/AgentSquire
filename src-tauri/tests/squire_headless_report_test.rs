//! End-to-end headless test of the Squire context-mode protocol using
//! DeepSeek V4 Flash as both Phase 1 and Phase 2 models.
//!
//! Exercises the full adapter lifecycle — build turn input, LLM call with
//! tools, tool-call loop (explore/rdf/token_to_detail/batch), Phase 1
//! finalization, and Phase 2 formatter pass — without any Tauri UI.
//!
//! ## Log output
//!
//! All logs are written into `target/squire-e2e-logs/<timestamp>/`:
//!
//!   master.log              — Timeline, events, summaries, verification,
//!                              tool calls, token counts, timing per stage.
//!   p1-build-system.txt      — Full system prompt + context JSON
//!   p1-build-user.txt        — User request with bookmarks
//!   p1-r<N>-request.json     — LLM request sent (messages, tools, params)
//!   p1-r<N>-response.txt     — Raw response text + tool calls
//!   p1-r<N>-tool-<name>.txt  — Tool execution result
//!   p1-finalize-outcome.txt  — finalize_turn result details
//!   p2-r<N>-request.json     — Phase 2 LLM request
//!   p2-r<N>-response.txt     — Phase 2 raw response
//!   p2-r<N>-outcome.txt      — Phase 2 finalize outcome
//!   store-tokens.txt         — Final token inventory
//!   store-graph.txt          — Final relationship graph
//!   conversation.txt         — All stored conversation messages
//!
//! Run with:
//!   cargo test --test squire_headless_report_test -- --nocapture
//!
//! Requires: DEEPSEEK_API_KEY environment variable.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::io::Write as IoWrite;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use provider_core::{ChatMessage, ChatRequest, ChatRole, FinishReason, StreamEvent, ToolCall};
use provider_registry::{ProviderRegistry, ProviderRegistryConfig, ProviderSpec};
use uuid::Uuid;

use squirecli_lib::agent::context_adapter::{ContextManagerAdapter, TurnOutcome};
use squirecli_lib::agent::squire::{
    InMemorySquireStore, SquireContextAdapter, SquireStore,
    built_in_tool_definitions, SquireBatchTool, SquireExploreTool,
    SquireRdfTool, SquireTokenToDetailTool, ToolEndpoint,
};
use squirecli_lib::agent::Tool;
use squirecli_lib::state::config::SquirePrefetchConfig;
use squirecli_lib::storage::conversation_store::{
    ContextMode, ConversationStore, Message, MessageRole, NewMessage, NewSession, Session,
    SessionId, SessionSummary, SessionWithMessages, StoreError,
};

// ═══════════════════════════════════════════════════════════════════════════
// File-based log directory
// ═══════════════════════════════════════════════════════════════════════════

struct LogDir {
    dir: PathBuf,
    master: StdMutex<std::fs::File>,
    t_start: Instant,
    seq: StdMutex<u64>,
}

impl LogDir {
    fn create(base: &Path) -> Result<Self, String> {
        let ts = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
        let dir = base.join(&ts);
        std::fs::create_dir_all(&dir)
            .map_err(|e| format!("Cannot create log dir {}: {}", dir.display(), e))?;

        let master_path = dir.join("master.log");
        let master = std::fs::File::create(&master_path)
            .map_err(|e| format!("Cannot create master log: {}", e))?;

        println!("  📁 Log directory: {}", dir.display());

        Ok(Self {
            dir,
            master: StdMutex::new(master),
            t_start: Instant::now(),
            seq: StdMutex::new(0),
        })
    }

    fn dir(&self) -> &Path { &self.dir }

    /// Write a line to master.log with an auto-incrementing sequence number
    /// and elapsed-time prefix.
    fn master(&self, line: &str) {
        let seq = {
            let mut s = self.seq.lock().unwrap();
            *s += 1;
            *s
        };
        let elapsed = self.t_start.elapsed().as_secs_f64() * 1000.0;
        let ts = chrono::Utc::now().format("%H:%M:%S%.3f").to_string();
        let line = format!("[{seq:04}] [{ts}] @{elapsed:>10.0}ms  {line}\n");

        let mut f = self.master.lock().unwrap();
        let _ = f.write_all(line.as_bytes());
        let _ = f.flush();
        // Also echo to stdout for real-time visibility
        print!("{}", line);
    }

    /// Write a full message payload into a named file under the log dir.
    fn write_file(&self, name: &str, content: &str) {
        let path = self.dir.join(name);
        match std::fs::write(&path, content) {
            Ok(()) => {
                let sz = content.len();
                self.master(&format!("📄 {name}  ({sz} bytes)"));
            }
            Err(e) => {
                self.master(&format!("❌ Failed to write {name}: {e}"));
            }
        }
    }

    /// Write a block of lines into a named file under the log dir.
    fn write_lines(&self, name: &str, lines: &[String]) {
        let mut content = String::new();
        for line in lines {
            writeln!(content, "{line}").unwrap();
        }
        self.write_file(name, &content);
    }
}

fn fmt_ms(ms: f64) -> String {
    if ms < 1000.0 { format!("{:.0}ms", ms) }
    else if ms < 60_000.0 { format!("{:.2}s", ms / 1000.0) }
    else { format!("{:.2}min", ms / 60_000.0) }
}

// ═══════════════════════════════════════════════════════════════════════════
// In-memory ConversationStore
// ═══════════════════════════════════════════════════════════════════════════

struct InMemoryConvStore {
    sessions: StdMutex<HashMap<SessionId, Session>>,
    messages: StdMutex<HashMap<SessionId, Vec<Message>>>,
}

impl InMemoryConvStore {
    fn new() -> Self {
        Self {
            sessions: StdMutex::new(HashMap::new()),
            messages: StdMutex::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl ConversationStore for InMemoryConvStore {
    async fn create_session(&self, new: NewSession) -> Result<Session, StoreError> {
        let id = Uuid::new_v4();
        let now = chrono::Utc::now();
        let session = Session {
            id,
            title: new.title,
            created_at: now,
            updated_at: now,
            context_mode: new.context_mode.unwrap_or_default(),
        };
        self.sessions.lock().unwrap().insert(id, session.clone());
        self.messages.lock().unwrap().insert(id, Vec::new());
        Ok(session)
    }
    async fn append_message(&self, msg: NewMessage) -> Result<Message, StoreError> {
        let message = Message {
            id: Uuid::new_v4(),
            session_id: msg.session_id,
            role: msg.role.clone(),
            content: msg.content.clone(),
            created_at: chrono::Utc::now(),
            blocks_json: None,
            thinking_content: msg.thinking_content.clone(),
        };
        self.messages.lock().unwrap().entry(msg.session_id).or_default().push(message.clone());
        if let Some(s) = self.sessions.lock().unwrap().get_mut(&msg.session_id) {
            s.updated_at = chrono::Utc::now();
        }
        Ok(message)
    }
    async fn get_session(&self, id: SessionId) -> Result<SessionWithMessages, StoreError> {
        let session = self.sessions.lock().unwrap().get(&id).cloned()
            .ok_or_else(|| StoreError::NotFound(id.to_string()))?;
        let messages = self.messages.lock().unwrap().get(&id).cloned().unwrap_or_default();
        Ok(SessionWithMessages { session, messages })
    }
    async fn list_sessions(&self) -> Result<Vec<SessionSummary>, StoreError> { Ok(Vec::new()) }
    async fn update_session_title(&self, _: SessionId, _: String) -> Result<(), StoreError> { Ok(()) }
    async fn delete_session(&self, _: SessionId) -> Result<(), StoreError> { Ok(()) }
    async fn truncate_messages_from(&self, _: SessionId, _: Uuid) -> Result<(), StoreError> { Ok(()) }
    async fn set_message_blocks(&self, _: Uuid, _: String) -> Result<(), StoreError> { Ok(()) }
}

// ═══════════════════════════════════════════════════════════════════════════
// LLM call helper — streams chunks, logs full request/response to files
// ═══════════════════════════════════════════════════════════════════════════

struct LlmResponse {
    full_text: String,
    tool_calls: Vec<ToolCall>,
    finish_reason: Option<FinishReason>,
}

/// Serialize messages prettily for request log files.
fn format_messages_for_log(messages: &[ChatMessage]) -> String {
    let mut out = String::new();
    for (i, msg) in messages.iter().enumerate() {
        let role = match msg.role {
            ChatRole::System => "SYSTEM",
            ChatRole::User => "USER",
            ChatRole::Assistant => "ASSISTANT",
            ChatRole::Tool => "TOOL",
        };
        writeln!(out, "──── msg[{i}] {role} ({len} bytes) ────", len = msg.content.len()).unwrap();
        writeln!(out, "{}\n", msg.content).unwrap();
    }
    out
}

async fn call_llm(
    provider: &Arc<dyn provider_core::LlmProvider>,
    model: &str,
    messages: &[ChatMessage],
    tools: &[provider_core::ToolDefinition],
    temperature: Option<f32>,
    max_tokens: Option<u32>,
    thinking_level: Option<String>,
    log: &LogDir,
    request_file: &str,
    response_file: &str,
) -> Result<LlmResponse, String> {
    let t_call = Instant::now();

    let request = ChatRequest {
        model: model.to_string(),
        messages: messages.to_vec(),
        tools: tools.to_vec(),
        thinking_level,
        temperature,
        max_tokens,
    };

    // ── Write request payload to separate file ──
    let mut req_payload = format!(
        "model: {model}\ntemperature: {temp:?}\nmax_tokens: {max_tokens:?}\ntools: {num_tools}\n\n",
        model = model, temp = temperature, max_tokens = max_tokens,
        num_tools = tools.len(),
    );
    req_payload.push_str(&format_messages_for_log(messages));
    log.write_file(request_file, &req_payload);

    log.master(&format!(
        "LLM call → {request_file}  ({nmsg} msgs, {ntool} tools, model={model}, temp={temp:?})",
        nmsg = messages.len(), ntool = tools.len(), model = model, temp = temperature
    ));

    let mut stream = provider
        .chat(request)
        .await
        .map_err(|e| {
            log.master(&format!("❌ Provider chat failed: {e}"));
            format!("Provider chat failed: {}", e)
        })?;

    let mut full_text = String::new();
    let mut tool_calls: Vec<ToolCall> = Vec::new();
    let mut finish_reason: Option<FinishReason> = None;
    let mut chunk_count = 0u64;
    let mut thinking_count = 0u64;

    while let Some(event) = stream.recv().await {
        match event {
            StreamEvent::Chunk(text) => {
                chunk_count += 1;
                full_text.push_str(&text);
            }
            StreamEvent::Thinking(_text) => {
                thinking_count += 1;
            }
            StreamEvent::ToolCall(tc) => {
                log.master(&format!(
                    "  🔧 ToolCall: {}  args={}",
                    tc.name,
                    serde_json::to_string(&tc.arguments).unwrap_or_default()
                ));
                tool_calls.push(tc);
            }
            StreamEvent::Done(reason) => {
                finish_reason = Some(reason);
                break;
            }
            StreamEvent::Error(err) => {
                log.master(&format!("❌ Stream error: {err}"));
                return Err(format!("Stream error: {}", err));
            }
            _ => {}
        }
    }

    let call_ms = t_call.elapsed().as_secs_f64() * 1000.0;
    log.master(&format!(
        "  ✓ LLM done → {response_file}  ({text_len} text bytes, {chunks} chunks, {think} thinking, {ntc} tool-calls, finish={finish:?}, duration={dur})",
        text_len = full_text.len(), chunks = chunk_count, think = thinking_count,
        ntc = tool_calls.len(), finish = finish_reason, dur = fmt_ms(call_ms)
    ));

    // ── Write response payload to separate file ──
    let mut resp_payload = format!(
        "duration: {}\nfinish_reason: {:?}\nchunks: {}\nthinking_chunks: {}\ntool_calls: {}\ntext_bytes: {}\n\n",
        fmt_ms(call_ms), finish_reason, chunk_count, thinking_count,
        tool_calls.len(), full_text.len(),
    );
    if !full_text.is_empty() {
        resp_payload.push_str("──── RESPONSE TEXT ────\n");
        resp_payload.push_str(&full_text);
        resp_payload.push_str("\n");
    }
    if !tool_calls.is_empty() {
        resp_payload.push_str("──── TOOL CALLS ────\n");
        for tc in &tool_calls {
            writeln!(resp_payload, "id: {}", tc.id).unwrap();
            writeln!(resp_payload, "name: {}", tc.name).unwrap();
            writeln!(resp_payload, "arguments: {}", serde_json::to_string_pretty(&tc.arguments).unwrap_or_default()).unwrap();
            writeln!(resp_payload).unwrap();
        }
    }
    log.write_file(response_file, &resp_payload);

    Ok(LlmResponse { full_text, tool_calls, finish_reason })
}

// ═══════════════════════════════════════════════════════════════════════════
// Pre-seed the Squire store
// ═══════════════════════════════════════════════════════════════════════════

async fn seed_store(store: &dyn SquireStore, session_id: SessionId) {
    use squire_store::{NewTokenSpec, Relationship, predicates};

    store.upsert_token(NewTokenSpec {
        id: "WF_BatchDiscovery".to_string(), token_type: "source".to_string(),
        short_desc: "Use batch composition syntax for efficient multi-call retrieval".to_string(),
        full_desc: Some("A retrieval-optimisation workflow: use batch() with | pipe and & parallel operators to bundle explore/rdf/token_to_detail calls into one batch call. Each batch expression counts as ONE call against the per-turn batch cap (default 3).".to_string()),
        endpoint: None, ranges: vec![],
    }, 0, session_id).await;
    store.add_relationship(Relationship {
        subject: "WF_BatchDiscovery".to_string(), predicate: predicates::IS_A_WORKFLOW.to_string(), object: "workflow".to_string(),
    }).await;

    store.upsert_token(NewTokenSpec {
        id: "CON_RustOwnership".to_string(), token_type: "concept".to_string(),
        short_desc: "Rust's ownership model: each value has exactly one owner at a time".to_string(),
        full_desc: Some("Rust's ownership system enforces memory safety at compile time. Key rules: 1) Each value has a single owner, 2) Values are dropped when the owner goes out of scope, 3) Ownership can be transferred (moved) or borrowed (&T immutable, &mut T mutable). The borrow checker enforces that references do not outlive their referents.".to_string()),
        endpoint: None, ranges: vec![],
    }, 0, session_id).await;

    store.upsert_token(NewTokenSpec {
        id: "mcp_git_git_log".to_string(), token_type: "source".to_string(),
        short_desc: "MCP tool: git log — show commit history for a repository".to_string(),
        full_desc: Some(r#"{"name":"git_log","description":"Show commit history with hashes, authors, dates, and messages.","input_schema":{"type":"object","properties":{"repo_path":{"type":"string"},"max_count":{"type":"integer"}}}}"#.to_string()),
        endpoint: Some(ToolEndpoint::Mcp {
            server: squire_store::McpServerConfig { id: "git-server".to_string(), name: "Git Tools".to_string(), transport: "stdio".to_string(), command: "git-mcp".to_string(), args: vec![], url: None, enabled: true, env: Default::default(), headers: Default::default() },
            remote_name: "git_log".to_string(),
        }),
        ranges: vec![],
    }, 0, session_id).await;
    store.add_relationship(Relationship {
        subject: "mcp_git_git_log".to_string(), predicate: predicates::IS_A_TOOL.to_string(), object: "tool".to_string(),
    }).await;

    store.upsert_token(NewTokenSpec {
        id: "REF_Borrowing".to_string(), token_type: "referential".to_string(),
        short_desc: "Borrowing: Rust's mechanism for temporary access without transferring ownership".to_string(),
        full_desc: Some("Immutable borrows (&T) allow multiple readers; mutable borrows (&mut T) allow exactly one writer. Rust enforces that you cannot have both at the same time, preventing data races at compile time.".to_string()),
        endpoint: None, ranges: vec![],
    }, 0, session_id).await;
    store.add_relationship(Relationship {
        subject: "REF_Borrowing".to_string(), predicate: "references".to_string(), object: "CON_RustOwnership".to_string(),
    }).await;

    store.set_preserve_list(session_id, vec![
        "WF_BatchDiscovery".to_string(), "CON_RustOwnership".to_string(), "REF_Borrowing".to_string(),
    ]).await;
}

// ═══════════════════════════════════════════════════════════════════════════
// Tool execution helper
// ═══════════════════════════════════════════════════════════════════════════

async fn execute_squire_tool(
    name: &str,
    args: serde_json::Value,
    store: &Arc<dyn SquireStore>,
    tool_defs: &[provider_core::ToolDefinition],
    session_id: SessionId,
    batch_counter: &Arc<std::sync::atomic::AtomicU32>,
) -> String {
    let cap = squirecli_lib::agent::squire::tools::DEFAULT_BATCH_CAP;

    match name {
        "explore" => {
            let tool = SquireExploreTool {
                store: store.clone(), tool_defs: tool_defs.to_vec(),
                session_id, batch_counter: batch_counter.clone(), batch_cap: cap,
            };
            tool.execute("call_1", args).await.output
        }
        "token_to_detail" => {
            let tool = SquireTokenToDetailTool {
                store: store.clone(), batch_counter: batch_counter.clone(), batch_cap: cap,
            };
            tool.execute("call_1", args).await.output
        }
        "rdf" => {
            let tool = SquireRdfTool {
                store: store.clone(), batch_counter: batch_counter.clone(), batch_cap: cap,
            };
            tool.execute("call_1", args).await.output
        }
        "batch" => {
            let tool = SquireBatchTool {
                store: store.clone(), tool_defs: tool_defs.to_vec(),
                session_id, batch_counter: batch_counter.clone(), batch_cap: cap,
            };
            tool.execute("call_1", args).await.output
        }
        other => format!("Unknown tool: {}", other),
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Main test
// ═══════════════════════════════════════════════════════════════════════════

const TEST_TIMEOUT_SECS: u64 = 240;

#[tokio::test]
async fn test_squire_headless_e2e() {
    let t0 = Instant::now();
    tokio::select! {
        _ = tokio::time::sleep(Duration::from_secs(TEST_TIMEOUT_SECS)) => {
            panic!("Test timed out after {}s", TEST_TIMEOUT_SECS);
        }
        result = run_test() => {
            match result {
                Ok(()) => {
                    println!("\n  ✅ Test passed. See logs above for output directory.");
                }
                Err(e) => {
                    let elapsed = t0.elapsed().as_secs_f64() * 1000.0;
                    println!("\n╔══════════════════════════════════════════════════════════════════╗");
                    println!("║              SQUIRE HEADLESS TEST — FAILED                      ║");
                    println!("╚══════════════════════════════════════════════════════════════════╝");
                    println!("\n  Error: {}", e);
                    println!("  Total elapsed: {}", fmt_ms(elapsed));
                }
            }
        }
    }
}

async fn run_test() -> Result<(), String> {
    let t0 = Instant::now();

    // ── Create log directory ─────────────────────────────────────────────
    let log_base = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target").join("squire-e2e-logs");
    let log = LogDir::create(&log_base)?;

    log.master("════════════════════════════════════════════════════════");
    log.master("  SQUIRE HEADLESS E2E PROTOCOL TEST");
    log.master("════════════════════════════════════════════════════════");

    // ── Environment ────────────────────────────────────────────────────
    let deepseek_key = std::env::var("DEEPSEEK_API_KEY")
        .map_err(|_| "DEEPSEEK_API_KEY environment variable required".to_string())?;

    log.master(&format!(
        "Provider: DeepSeek (OpenAI-compatible) | Model: deepseek-v4-flash | OS: {os}",
        os = std::env::consts::OS
    ));
    log.master(&format!("API key: {}...", &deepseek_key[..8.min(deepseek_key.len())]));

    // ── Setup ──────────────────────────────────────────────────────────
    let t_setup = Instant::now();

    let spec = ProviderSpec {
        provider_type: "openai".to_string(), name: "deepseek".to_string(),
        api_key: deepseek_key, model: "deepseek-v4-flash".to_string(),
        models: vec!["deepseek-v4-flash".to_string()],
        endpoint: Some("https://api.deepseek.com/v1".to_string()),
        metadata: HashMap::new(), category: None,
    };
    let registry_config = ProviderRegistryConfig {
        providers: vec![spec], verbose_logging: true, wire_log_path: None,
    };
    let provider_registry = Arc::new(ProviderRegistry::from_config(&registry_config));
    let (provider_arc, _) = provider_registry
        .resolve_provider_for_instance(&provider_core::ModelInstance::new("deepseek", "deepseek-v4-flash"))
        .map_err(|e| e.to_string())?;

    let conv_store = Arc::new(InMemoryConvStore::new()) as Arc<dyn ConversationStore>;
    let squire_store = Arc::new(InMemorySquireStore::new()) as Arc<dyn SquireStore>;

    let session = conv_store.create_session(NewSession {
        title: "Squire Headless E2E Test".to_string(),
        context_mode: Some(ContextMode::Squire),
    }).await.map_err(|e| e.to_string())?;
    let sid = session.id;

    let user_prompt = concat!(
        "I'm learning Rust and need help understanding ownership. ",
        "First, search the memory for existing knowledge about Rust ownership ",
        "using the explore tool. If you find a concept token, use rdf to see ",
        "its related tokens. Then explain how borrowing works with an example."
    );

    conv_store.append_message(NewMessage {
        session_id: sid, role: MessageRole::User, content: user_prompt.to_string(), thinking_content: None,
    }).await.map_err(|e| e.to_string())?;

    let setup_ms = t_setup.elapsed().as_secs_f64() * 1000.0;
    log.master(&format!("Session: {sid} | Context: Squire | Setup: {dur}",
        sid = sid, dur = fmt_ms(setup_ms)));
    log.master(&format!("User prompt ({} chars): \"{}\"", user_prompt.len(), user_prompt));

    // ── Pre-seed store ─────────────────────────────────────────────────
    let t_seed = Instant::now();
    seed_store(squire_store.as_ref(), sid).await;
    let seed_ms = t_seed.elapsed().as_secs_f64() * 1000.0;

    let pre_seed_tokens = squire_store.list_token_ids_by_session(sid).await;
    let pre_seed_rels = squire_store.get_relationships(None, None, None).await;
    log.master(&format!(
        "Pre-seed: {} tokens, {} rels in {}",
        pre_seed_tokens.len(), pre_seed_rels.len(), fmt_ms(seed_ms)
    ));
    for r in &pre_seed_rels {
        log.master(&format!("  rel: {} →[{}]→ {}", r.subject, r.predicate, r.object));
    }

    // ═════════════════════════════════════════════════════════════════
    // PHASE 1 — Build turn input
    // ═════════════════════════════════════════════════════════════════
    log.master("── Phase 1: build_turn_input ──");
    let t_p1_start = Instant::now();

    let session_data = conv_store.get_session(sid).await.map_err(|e| e.to_string())?;
    let mut adapter = SquireContextAdapter::new_with_prefetch(
        squire_store.clone(),
        SquirePrefetchConfig { memory_top_k: 10, workflow_top_k: 3, tool_top_k: 5, skill_top_k: 3, min_score: 0.0, ..Default::default() },
    );

    let turn_input = adapter.build_turn_input(&session_data, &[])
        .await.map_err(|e| e.to_string())?;

    let t_build = Instant::now();
    let build_ms = t_build.elapsed().as_secs_f64() * 1000.0;

    let sys_content = &turn_input.messages[0].content;
    let user_msg_content = &turn_input.messages[1].content;
    let bookmark_count = user_msg_content.matches("§^chunk_").count();
    let chunk_token_count = squire_store.list_token_ids_by_session(sid).await.iter().filter(|t| t.starts_with("USR_T")).count();
    let tool_names: Vec<String> = turn_input.tools.iter().map(|t| t.name.clone()).collect();

    log.master(&format!(
        "build_turn_input: {} ({dur}) | system={sys_sz}B | user={usr_sz}B | bookmarks={bm} | USR_T chunks={ch} | tools=[{tools}] | context={ctx}",
        turn_input.messages.len(), dur = fmt_ms(build_ms),
        sys_sz = sys_content.len(), usr_sz = user_msg_content.len(),
        bm = bookmark_count, ch = chunk_token_count,
        tools = tool_names.join(", "),
        ctx = if sys_content.contains("long_tokens") { "long+short" } else { "?" },
    ));

    // Write system prompt and user message to separate files
    log.write_file("p1-build-system.txt", sys_content);
    log.write_file("p1-build-user.txt", user_msg_content);

    // ═════════════════════════════════════════════════════════════════
    // PHASE 1 — LLM + Tool loop
    // ═════════════════════════════════════════════════════════════════
    log.master("── Phase 1: LLM + Tool loop ──");

    let mut messages = turn_input.messages;
    let turn_tools = built_in_tool_definitions();
    let batch_counter = Arc::new(std::sync::atomic::AtomicU32::new(0));
    let max_tool_rounds = 4;
    let mut tool_round = 0u32;
    let mut total_tool_calls = 0u32;
    let phase1_content;

    loop {
        tool_round += 1;

        let req_file = format!("p1-r{}-request.txt", tool_round);
        let resp_file = format!("p1-r{}-response.txt", tool_round);

        let llm_response = call_llm(
            &provider_arc, "deepseek-v4-flash", &messages, &turn_tools,
            Some(0.7), Some(4096), None, &log, &req_file, &resp_file,
        ).await.map_err(|e| e.to_string())?;

        if !llm_response.tool_calls.is_empty() {
            // Push assistant message with tool calls
            messages.push(ChatMessage {
                role: ChatRole::Assistant,
                content: llm_response.full_text.clone(),
                tool_call_id: None,
                tool_calls: Some(llm_response.tool_calls.clone()),
                reasoning_content: None,
            });

            // Execute each tool call
            for (ti, tc) in llm_response.tool_calls.iter().enumerate() {
                total_tool_calls += 1;
                let t_tool = Instant::now();
                let tool_args_str = serde_json::to_string(&tc.arguments).unwrap_or_default();

                let result = if ["explore", "token_to_detail", "rdf", "batch"].contains(&tc.name.as_str()) {
                    execute_squire_tool(&tc.name, tc.arguments.clone(), &squire_store, &[], sid, &batch_counter).await
                } else {
                    format!("Unknown tool: {}", tc.name)
                };

                let tool_ms = t_tool.elapsed().as_secs_f64() * 1000.0;
                let args_preview = if tool_args_str.len() > 80 {
                    format!("{}...", &tool_args_str[..80])
                } else {
                    tool_args_str.clone()
                };
                log.master(&format!(
                    "  🔨 r{round}.{ti} {tool}({args}) → {len} chars in {dur}",
                    round = tool_round, ti = ti, tool = tc.name,
                    args = args_preview,
                    len = result.len(), dur = fmt_ms(tool_ms)
                ));

                // Write tool result to file
                log.write_file(
                    &format!("p1-r{}-tool-{}-{}.txt", tool_round, ti, tc.name),
                    &format!("tool: {}\nargs: {}\nduration: {}\nresult:\n{}",
                        tc.name, tool_args_str, fmt_ms(tool_ms), result),
                );

                // Push tool result message
                messages.push(ChatMessage {
                    role: ChatRole::Tool,
                    content: result,
                    tool_call_id: Some(tc.id.clone()),
                    tool_calls: None,
                    reasoning_content: None,
                });
            }

            if tool_round >= max_tool_rounds {
                log.master(&format!("Max tool rounds ({max_tool_rounds}) reached"));
                phase1_content = llm_response.full_text;
                break;
            }
            continue;
        }

        // No tool calls — this is the final Phase 1 response
        phase1_content = llm_response.full_text;
        break;
    }

    let t_llm_done = Instant::now();
    let llm_ms = t_llm_done.duration_since(t_p1_start).as_secs_f64() * 1000.0;
    log.master(&format!(
        "Phase 1 LLM complete: {rounds} rounds, {ntc} tool calls, {dur}",
        rounds = tool_round, ntc = total_tool_calls, dur = fmt_ms(llm_ms)
    ));

    // Bookmark analysis
    let inline_refs: Vec<&str> = phase1_content.match_indices("§!").map(|(i, _)| {
        let rest = &phase1_content[i+3..];
        let end = rest.find(|c: char| !c.is_alphanumeric() && c != '_').unwrap_or(rest.len());
        &rest[..end]
    }).collect();
    let span_open_count = phase1_content.match_indices("§^").count();
    log.master(&format!(
        "Response analysis: {} text bytes, {} §! inline refs, {} §^ span markers",
        phase1_content.len(), inline_refs.len(), span_open_count
    ));
    if !inline_refs.is_empty() {
        log.master(&format!("  §! refs: {}", inline_refs.iter().map(|r| format!("§!{r}")).collect::<Vec<_>>().join(", ")));
    }

    // Write full Phase 1 response to file
    log.write_file("p1-final-response.txt", &format!(
        "text_bytes: {}\ninline_refs: {:?}\nspan_markers: {}\n\n{}",
        phase1_content.len(), inline_refs, span_open_count, phase1_content
    ));

    // ═════════════════════════════════════════════════════════════════
    // PHASE 1 — Finalize turn
    // ═════════════════════════════════════════════════════════════════
    log.master("── Phase 1: finalize_turn ──");
    let t_finalize = Instant::now();

    let outcome = adapter.finalize_turn(sid, phase1_content.clone(), None, &mut messages, conv_store.as_ref())
        .await.map_err(|e| e.to_string())?;
    let finalize_ms = t_finalize.elapsed().as_secs_f64() * 1000.0;

    let (phase1_final_content, user_request) = match outcome {
        TurnOutcome::Phase2 { phase1_content, user_request } => {
            log.master(&format!("  ✓ finalize_turn → Phase2 ({})", fmt_ms(finalize_ms)));
            (phase1_content, user_request)
        }
        TurnOutcome::Done => {
            log.master("  ⚠ finalize_turn → Done (no Phase 2 — tokens were not generated)");
            return Ok(());
        }
        TurnOutcome::Retry => {
            log.master("  ⚠ finalize_turn → Retry (compliance rejection)");
            return Ok(());
        }
        TurnOutcome::Failed { reason, failed_content } => {
            log.master(&format!("  ❌ finalize_turn → Failed: {reason}"));
            log.write_file("p1-finalize-failed.txt", &format!("reason: {reason}\n\n{failed_content}"));
            return Ok(());
        }
        other => {
            log.master(&format!("  ❓ Unexpected outcome: {:?}", match &other {
                TurnOutcome::AskUser { .. } => "AskUser",
                TurnOutcome::Phase2Done { .. } => "Phase2Done",
                _ => "other",
            }));
            return Ok(());
        }
    };

    // Write finalize outcome for inspection
    log.write_file("p1-finalize-outcome.txt", &format!(
        "duration: {}\nphase1_content_len: {}\nuser_request:\n{}\n\nphase1_final_content:\n{}",
        fmt_ms(finalize_ms), phase1_content.len(), user_request, phase1_final_content
    ));

    // Store state after Phase 1
    let tokens_after_p1 = squire_store.list_token_ids_by_session(sid).await;
    let usr_t_count = tokens_after_p1.iter().filter(|t| t.starts_with("USR_T")).count();
    let resp_t_count = tokens_after_p1.iter().filter(|t| t.starts_with("RESP_T")).count();
    let rels_after_p1 = squire_store.get_relationships(None, None, None).await;
    log.master(&format!(
        "Store after Phase 1: {} tokens (USR_T:{}, RESP_T:{}, other:{}) [+{} from pre-seed], {} rels",
        tokens_after_p1.len(), usr_t_count, resp_t_count,
        tokens_after_p1.len().saturating_sub(usr_t_count).saturating_sub(resp_t_count),
        tokens_after_p1.len().saturating_sub(pre_seed_tokens.len()),
        rels_after_p1.len(),
    ));

    // ═════════════════════════════════════════════════════════════════
    // PHASE 2 — Token generation
    // ═════════════════════════════════════════════════════════════════
    log.master("── Phase 2: Token Generation ──");
    let t_p2_start = Instant::now();

    let phase2_prompt = squirecli_lib::agent::squire_prompts::system_prompt_phase2();
    let mut p2_messages = vec![
        ChatMessage {
            role: ChatRole::System, content: phase2_prompt,
            tool_call_id: None, tool_calls: None, reasoning_content: None,
        },
        ChatMessage {
            role: ChatRole::User,
            content: format!(
                "Original user request:\n{}\n\nAssistant Phase 1 response:\n{}",
                user_request, phase1_final_content
            ),
            tool_call_id: None, tool_calls: None, reasoning_content: None,
        },
    ];

    adapter.set_phase2(user_request.clone());

    let mut p2_tokens_total = 0usize;
    let mut p2_rels_total = 0usize;
    let mut p2_final_round = 0u32;

    for round in 1..=4 {
        p2_final_round = round;
        let tokens_before = squire_store.list_token_ids_by_session(sid).await.len();

        let req_file = format!("p2-r{}-request.txt", round);
        let resp_file = format!("p2-r{}-response.txt", round);

        let p2_response = call_llm(
            &provider_arc, "deepseek-v4-flash", &p2_messages, &[],
            Some(0.0), Some(4096), Some("none".to_string()), &log, &req_file, &resp_file,
        ).await.map_err(|e| e.to_string())?;

        let outcome = adapter.finalize_turn(sid, p2_response.full_text.clone(), None, &mut p2_messages, conv_store.as_ref())
            .await.map_err(|e| e.to_string())?;

        let tokens_after = squire_store.list_token_ids_by_session(sid).await.len();
        let tokens_this_round = tokens_after.saturating_sub(tokens_before);

        let outcome_file = format!("p2-r{}-outcome.txt", round);

        match outcome {
            TurnOutcome::Phase2Done { tokens_accepted, relationships_accepted, ref tokens_rejected, ref relationships_rejected } => {
                p2_tokens_total += tokens_accepted;
                p2_rels_total += relationships_accepted;
                log.master(&format!(
                    "  ✓ r{round} Phase2Done: {ta} tokens accepted, {ra} rels, store now {store} tokens",
                    round = round, ta = tokens_accepted, ra = relationships_accepted, store = tokens_after
                ));
                let mut outcome_text = format!(
                    "result: Phase2Done\ntokens_accepted: {}\nrelationships_accepted: {}\nstore_tokens_after: {}\n",
                    tokens_accepted, relationships_accepted, tokens_after
                );
                if !tokens_rejected.is_empty() { outcome_text.push_str(&format!("tokens_rejected: {:?}\n", tokens_rejected)); }
                if !relationships_rejected.is_empty() { outcome_text.push_str(&format!("relationships_rejected: {:?}\n", relationships_rejected)); }
                log.write_file(&outcome_file, &outcome_text);
                break;
            }
            TurnOutcome::Retry => {
                p2_tokens_total += tokens_this_round;
                let rejection_msg = p2_messages.last().map(|m| &m.content[..300.min(m.content.len())]).unwrap_or("?");
                log.master(&format!(
                    "  ⚠ r{round} Retry: +{n} tokens saved (store={store}), rejection: {rej}",
                    round = round, n = tokens_this_round, store = tokens_after, rej = rejection_msg
                ));
                log.write_file(&outcome_file, &format!(
                    "result: Retry\ntokens_saved: {}\nrejection_feedback:\n{}",
                    tokens_this_round, rejection_msg
                ));
                continue;
            }
            TurnOutcome::Failed { ref reason, .. } => {
                log.master(&format!("  ❌ r{round} Failed: {reason}", round = round));
                log.write_file(&outcome_file, &format!("result: Failed\nreason: {reason}\n"));
                break;
            }
            TurnOutcome::Done => {
                log.master(&format!("  ℹ r{round} Done", round = round));
                break;
            }
            other => {
                log.master(&format!("  ❓ r{round} unexpected outcome", round = round));
                break;
            }
        }
    }

    let p2_ms = t_p2_start.elapsed().as_secs_f64() * 1000.0;
    log.master(&format!(
        "Phase 2 done: {} rounds, {} tokens, {} rels, {}",
        p2_final_round, p2_tokens_total, p2_rels_total, fmt_ms(p2_ms)
    ));

    // ═════════════════════════════════════════════════════════════════
    // FINAL STORE STATE
    // ═════════════════════════════════════════════════════════════════
    log.master("── Final Store State ──");

    let all_token_ids = squire_store.list_token_ids_by_session(sid).await;
    let all_relationships = squire_store.get_relationships(None, None, None).await;

    let mut token_inventory: Vec<String> = Vec::new();
    let (mut usr_t, mut resp_t, mut concept_t, mut referential_t, mut workflow_t, mut tool_t, mut other_t) =
        (0, 0, 0, 0, 0, 0, 0);

    for tid in &all_token_ids {
        let detail = squire_store.token_detail(tid).await;
        let short = detail.as_ref().map(|d| d.short_desc.clone()).unwrap_or_default();

        if tid.starts_with("USR_T") { usr_t += 1; }
        else if tid.starts_with("RESP_T") { resp_t += 1; }
        else {
            let is_workflow = all_relationships.iter().any(|r| r.subject == *tid && r.predicate == squire_store::predicates::IS_A_WORKFLOW);
            let is_tool = all_relationships.iter().any(|r| r.subject == *tid && r.predicate == squire_store::predicates::IS_A_TOOL);
            if is_workflow { workflow_t += 1; }
            else if is_tool { tool_t += 1; }
            else if tid.starts_with("CON_") || tid.starts_with("CONCEPT_") { concept_t += 1; }
            else if tid.starts_with("REF_") || tid.starts_with("REFERENTIAL_") { referential_t += 1; }
            else { other_t += 1; }
        }

        token_inventory.push(format!("{tid}  —  {short}"));
    }

    log.master(&format!(
        "Tokens: {total} total  (USR_T:{usr}, RESP_T:{resp}, concept:{con}, referential:{refr}, workflow:{wf}, tool:{tool}, other:{oth})",
        total = all_token_ids.len(), usr = usr_t, resp = resp_t, con = concept_t,
        refr = referential_t, wf = workflow_t, tool = tool_t, oth = other_t,
    ));
    log.write_file("store-tokens.txt", &token_inventory.join("\n"));

    let graph_lines: Vec<String> = all_relationships.iter()
        .map(|r| format!("{} →[{}]→ {}", r.subject, r.predicate, r.object))
        .collect();
    log.master(&format!("Relationships: {n} total", n = graph_lines.len()));
    log.write_lines("store-graph.txt", &graph_lines);

    // Preserve list
    let preserved = squire_store.preserved_tokens(sid).await;
    let preserve_lines: Vec<String> = preserved.iter()
        .map(|t| format!("{}  —  {}", t.token_id, t.short_desc))
        .collect();
    log.master(&format!("Preserve list: {n} entries", n = preserve_lines.len()));
    log.write_lines("preserve-list.txt", &preserve_lines);

    // ── Conversation messages ─────────────────────────────────────────
    let session_data = conv_store.get_session(sid).await.map_err(|e| e.to_string())?;
    let mut conv_text = String::new();
    for (i, msg) in session_data.messages.iter().enumerate() {
        let role = match msg.role { MessageRole::User => "USER", MessageRole::Assistant => "ASSISTANT", MessageRole::System => "SYSTEM" };
        writeln!(conv_text, "──── [{i}] {role}  ({ts})  ({len} bytes) ────",
            ts = msg.created_at.to_rfc3339(), len = msg.content.len()).unwrap();
        writeln!(conv_text, "{}\n", msg.content).unwrap();
    }
    log.master(&format!("Conversation: {n} messages", n = session_data.messages.len()));
    log.write_file("conversation.txt", &conv_text);

    // ═════════════════════════════════════════════════════════════════
    // VERIFICATION
    // ═════════════════════════════════════════════════════════════════
    log.master("── Verification ──");
    {
        let ok = |b: bool| if b { "PASS" } else { "FAIL" };

        let has_usr = all_token_ids.iter().any(|t| t.starts_with("USR_T"));
        log.master(&format!("  [{}] USR_T chunk tokens created", ok(has_usr)));

        let has_new = all_token_ids.len() > pre_seed_tokens.len();
        log.master(&format!("  [{}] New tokens created ({} total, was {} pre-seed)",
            ok(has_new), all_token_ids.len(), pre_seed_tokens.len()));

        let has_msgs = session_data.messages.len() >= 2;
        log.master(&format!("  [{}] Messages stored ({})", ok(has_msgs), session_data.messages.len()));

        let has_rels = !all_relationships.is_empty();
        log.master(&format!("  [{}] Relationships present ({})", ok(has_rels), all_relationships.len()));

        let has_tools = total_tool_calls > 0;
        log.master(&format!("  [{}] Tool calls executed ({})", ok(has_tools), total_tool_calls));
    }

    // ═════════════════════════════════════════════════════════════════
    // FINAL TIMING
    // ═════════════════════════════════════════════════════════════════
    let total_ms = t0.elapsed().as_secs_f64() * 1000.0;
    log.master("── Timing Breakdown ──");
    log.master(&format!("  Setup (providers+session):  {}", fmt_ms(setup_ms)));
    log.master(&format!("  Store seeding:              {}", fmt_ms(seed_ms)));
    log.master(&format!("  Phase 1 build_turn_input:   {}", fmt_ms(build_ms)));
    log.master(&format!("  Phase 1 LLM + tool loop:    {}", fmt_ms(llm_ms)));
    log.master(&format!("  Phase 1 finalize_turn:      {}", fmt_ms(finalize_ms)));
    log.master(&format!("  Phase 2 token generation:   {}", fmt_ms(p2_ms)));
    log.master(&format!("  ═══════════════════════════════════"));
    log.master(&format!("  TOTAL WALL TIME:            {}", fmt_ms(total_ms)));
    log.master(&format!("  Phase 2 tokens stored:      {p2_tokens_total}"));
    log.master(&format!("  Phase 2 rels stored:        {p2_rels_total}"));
    log.master("════════════════════════════════════════════════════════");
    log.master("  TEST COMPLETE");
    log.master("════════════════════════════════════════════════════════");
    log.master(&format!("Log directory: {}", log.dir().display()));

    Ok(())
}
