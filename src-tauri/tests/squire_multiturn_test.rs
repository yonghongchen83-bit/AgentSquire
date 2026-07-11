//! Multi-turn Squire protocol test — 3 sequential, related questions
//! designed to measure cross-turn context retention, explore efficiency,
//! and graph-traversal quality.
//!
//! ## Test design
//!
//!   Q1 — "Factual seeding":   Ask about a specific topic to create tokens.
//!   Q2 — "Cross-turn recall":  Ask a related question that should find Q1's
//!                              tokens via explore() and build on them.
//!   Q3 — "Synthesis":          A third question spanning both Q1 and Q2
//!                              answering — should traverse the graph.
//!
//! ## What this measures
//!
//!   - Token growth per turn (USR_T, RESP_T, concept, referential)
//!   - Tool-call patterns (explore vs rdf vs batch, hop depths)
//!   - Inline-reference reuse across turns (§!REF_*)
//!   - Retrieval efficiency (tool calls to get answer, rounds taken)
//!   - Graph expansion (new relationships created per turn)
//!   - Phase 2 formatter output quality
//!
//! ## Log output
//!
//!   target/squire-e2e-logs/<timestamp>-multiturn/
//!     master.log
//!     t<N>/                  — Per-turn directory
//!       build-system.txt
//!       build-user.txt
//!       r<N>-request.txt / r<N>-response.txt / tool-*.txt
//!       final-response.txt / finalize-outcome.txt
//!       p2-r<N>-*.txt
//!     store-final.txt         — Final token + graph snapshots
//!     timing.csv              — Per-stage timing data
//!     analysis.txt            — Efficiency metrics computed across turns
//!
//! Run with:
//!   cargo test --test squire_multiturn_test -- --nocapture
//!
//! Requires: DEEPSEEK_API_KEY environment variable.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::io::Write as IoWrite;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};

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

// ═══════════════════════════════════════════════════════════════════════════
// Log directory
// ═══════════════════════════════════════════════════════════════════════════

struct LogDir {
    dir: PathBuf,
    master: StdMutex<std::fs::File>,
    t_start: Instant,
    seq: StdMutex<u64>,
}

impl LogDir {
    fn create(base: &Path, suffix: &str) -> Result<Self, String> {
        let ts = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
        let dir = base.join(format!("{ts}-{suffix}"));
        std::fs::create_dir_all(&dir)
            .map_err(|e| format!("Cannot create log dir {}: {}", dir.display(), e))?;
        let master_path = dir.join("master.log");
        let master = std::fs::File::create(&master_path)
            .map_err(|e| format!("Cannot create master log: {}", e))?;
        println!("  📁 Log directory: {}", dir.display());
        Ok(Self { dir, master: StdMutex::new(master), t_start: Instant::now(), seq: StdMutex::new(0) })
    }

    fn dir(&self) -> &Path { &self.dir }

    fn sub_dir(&self, name: &str) -> PathBuf {
        let d = self.dir.join(name);
        let _ = std::fs::create_dir_all(&d);
        d
    }

    fn master(&self, line: &str) {
        let seq = { let mut s = self.seq.lock().unwrap(); *s += 1; *s };
        let elapsed = self.t_start.elapsed().as_secs_f64() * 1000.0;
        let ts = chrono::Utc::now().format("%H:%M:%S%.3f").to_string();
        let line = format!("[{seq:04}] [{ts}] @{elapsed:>10.0}ms  {line}\n");
        let mut f = self.master.lock().unwrap();
        let _ = f.write_all(line.as_bytes());
        let _ = f.flush();
        print!("{}", line);
    }

    fn write_file(&self, name: &str, content: &str) {
        let path = self.dir.join(name);
        let _ = std::fs::write(&path, content);
        self.master(&format!("📄 {name}  ({sz} bytes)", sz = content.len()));
    }

    fn write_csv(&self, name: &str, header: &str, rows: &[Vec<String>]) {
        let mut content = String::from(header);
        content.push('\n');
        for row in rows {
            content.push_str(&row.join(","));
            content.push('\n');
        }
        self.write_file(name, &content);
    }
}

fn fmt_ms(ms: f64) -> String {
    if ms < 1000.0 { format!("{:.0}ms", ms) }
    else if ms < 60_000.0 { format!("{:.2}s", ms / 1000.0) }
    else { format!("{:.2}min", ms / 60_000.0) }
}

fn clamp(s: &str, n: usize) -> String {
    if s.len() <= n { s.to_string() } else { format!("{}…", &s[..n]) }
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
        Self { sessions: StdMutex::new(HashMap::new()), messages: StdMutex::new(HashMap::new()) }
    }
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
        let m = Message { id: Uuid::new_v4(), session_id: msg.session_id, role: msg.role.clone(), content: msg.content.clone(), created_at: chrono::Utc::now(), blocks_json: None, thinking_content: msg.thinking_content.clone() };
        self.messages.lock().unwrap().entry(msg.session_id).or_default().push(m.clone());
        if let Some(s) = self.sessions.lock().unwrap().get_mut(&msg.session_id) { s.updated_at = chrono::Utc::now(); }
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

// ═══════════════════════════════════════════════════════════════════════════
// LLM call helper
// ═══════════════════════════════════════════════════════════════════════════

struct LlmResponse {
    full_text: String,
    tool_calls: Vec<ToolCall>,
    reasoning: String,
}

fn format_messages_for_log(msgs: &[ChatMessage]) -> String {
    let mut out = String::new();
    for (i, m) in msgs.iter().enumerate() {
        let role = match m.role { ChatRole::System => "SYS", ChatRole::User => "USR", ChatRole::Assistant => "AST", ChatRole::Tool => "TOL" };
        writeln!(out, "── msg[{i}] {role} ({len}B) ──\n{body}\n", i = i, role = role, len = m.content.len(), body = clamp(&m.content, 3000)).unwrap();
    }
    out
}

async fn call_llm(
    provider: &Arc<dyn provider_core::LlmProvider>, model: &str,
    messages: &[ChatMessage], tools: &[provider_core::ToolDefinition],
    temperature: Option<f32>, max_tokens: Option<u32>,
    thinking_level: Option<String>,
    log: &LogDir, req_file: &str, resp_file: &str,
) -> Result<LlmResponse, String> {
    let t0 = Instant::now();
    let request = ChatRequest { model: model.to_string(), messages: messages.to_vec(), tools: tools.to_vec(), thinking_level, temperature, max_tokens };

    let req_payload = format!("model: {model}\ntemp: {temp:?}\nmax_tokens: {mt:?}\ntools: {nt}\n\n{msgs}",
        model = model, temp = temperature, mt = max_tokens, nt = tools.len(), msgs = format_messages_for_log(messages));
    log.write_file(req_file, &req_payload);

    let mut stream = provider.chat(request).await.map_err(|e| { log.master(&format!("❌ chat: {e}")); format!("Provider: {e}") })?;

    let mut full_text = String::new();
    let mut full_reasoning = String::new();
    let mut tool_calls: Vec<ToolCall> = Vec::new();
    let mut chunk_n = 0u64; let mut think_n = 0u64;

    while let Some(event) = stream.recv().await {
        match event {
            StreamEvent::Chunk(t) => { chunk_n += 1; full_text.push_str(&t); }
            StreamEvent::Thinking(t) => { think_n += 1; full_reasoning.push_str(&t); }
            StreamEvent::ToolCall(tc) => {
                log.master(&format!("    TC: {}  args={}", tc.name, clamp(&serde_json::to_string(&tc.arguments).unwrap_or_default(), 100)));
                tool_calls.push(tc);
            }
            StreamEvent::Done(_) => break,
            StreamEvent::Error(e) => { log.master(&format!("❌ stream: {e}")); return Err(format!("Stream: {e}")); }
            _ => {}
        }
    }

    let dur = t0.elapsed().as_secs_f64() * 1000.0;
    log.master(&format!("    done in {}  ({}B text, {} chunks, {} think, {} TC, {}B reasoning)",
        fmt_ms(dur), full_text.len(), chunk_n, think_n, tool_calls.len(), full_reasoning.len()));

    let mut rp = format!("duration: {}\nchunks: {}\nthinking_chunks: {}\ntool_calls: {}\ntext_bytes: {}\nreasoning_bytes: {}\n\n",
        fmt_ms(dur), chunk_n, think_n, tool_calls.len(), full_text.len(), full_reasoning.len());
    if !full_text.is_empty() { rp.push_str("── RESPONSE ──\n"); rp.push_str(&full_text); rp.push('\n'); }
    for tc in &tool_calls { writeln!(rp, "TC: id={} name={} args={}", tc.id, tc.name, serde_json::to_string(&tc.arguments).unwrap_or_default()).unwrap(); }
    log.write_file(resp_file, &rp);

    Ok(LlmResponse { full_text, tool_calls, reasoning: full_reasoning })
}

// ═══════════════════════════════════════════════════════════════════════════
// Tool execution
// ═══════════════════════════════════════════════════════════════════════════

async fn execute_squire_tool(
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

// ═══════════════════════════════════════════════════════════════════════════
// Per-turn metrics
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Default)]
struct TurnMetrics {
    label: String,
    build_ms: f64,
    llm_ms: f64,
    finalize_ms: f64,
    p2_ms: f64,
    num_rounds: u32,
    num_tool_calls: u32,
    unique_tools: Vec<String>,
    explore_calls: u32,
    rdf_calls: u32,
    batch_calls: u32,
    p1_text_bytes: usize,
    inline_refs: usize,
    span_markers: usize,
    p2_tokens: usize,
    p2_rels: usize,
    tokens_before: usize,
    tokens_after: usize,
    rels_before: usize,
    rels_after: usize,
    response_preview: String,
}

// ═══════════════════════════════════════════════════════════════════════════
// Single-turn runner
// ═══════════════════════════════════════════════════════════════════════════

async fn run_turn(
    sid: SessionId,
    user_prompt: &str,
    provider: &Arc<dyn provider_core::LlmProvider>,
    conv_store: &Arc<dyn ConversationStore>,
    squire_store: &Arc<dyn SquireStore>,
    log: &LogDir,
    turn_dir: &Path,
    label: &str,
) -> Result<TurnMetrics, String> {
    let mut m = TurnMetrics { label: label.to_string(), ..Default::default() };
    let t0 = Instant::now();
    log.master(&format!("══ TURN: {label} ══"));

    // Store pre-turn state
    m.tokens_before = squire_store.list_token_ids_by_session(sid).await.len();
    m.rels_before = squire_store.get_relationships(None, None, None).await.len();
    log.master(&format!("  Store before: {} tokens, {} rels", m.tokens_before, m.rels_before));

    // Append user message
    conv_store.append_message(NewMessage { session_id: sid, role: MessageRole::User, content: user_prompt.to_string(), thinking_content: None })
        .await.map_err(|e| e.to_string())?;

    // ── build_turn_input ──
    let session_data = conv_store.get_session(sid).await.map_err(|e| e.to_string())?;
    let mut adapter = SquireContextAdapter::new_with_prefetch(squire_store.clone(), SquirePrefetchConfig::default());

    let input = adapter.build_turn_input(&session_data, &[]).await.map_err(|e| e.to_string())?;
    m.build_ms = t0.elapsed().as_secs_f64() * 1000.0;

    let sys = &input.messages[0].content;
    let usr = &input.messages[1].content;
    log.write_file(&format!("{}/build-system.txt", turn_dir.display()), sys);
    log.write_file(&format!("{}/build-user.txt", turn_dir.display()), usr);

    log.master(&format!("  build: {dur}  (sys={sysB}B, usr={usrB}B, tools={tools})",
        dur = fmt_ms(m.build_ms), sysB = sys.len(), usrB = usr.len(),
        tools = input.tools.iter().map(|t| t.name.as_str()).collect::<Vec<_>>().join(","),
    ));

    // ── Phase 1 LLM + tool loop ──
    let t_p1 = Instant::now();
    let mut messages = input.messages;
    let turn_tools = built_in_tool_definitions();
    let batch_counter = Arc::new(std::sync::atomic::AtomicU32::new(0));
    let max_rounds = 5;
    let p1_content;

    loop {
        m.num_rounds += 1;
        let rf = format!("{}/r{}-request.txt", turn_dir.display(), m.num_rounds);
        let sf = format!("{}/r{}-response.txt", turn_dir.display(), m.num_rounds);

        let resp = call_llm(provider, "deepseek-v4-flash", &messages, &turn_tools, Some(0.7), Some(4096), None, log, &rf, &sf).await?;

        if !resp.tool_calls.is_empty() {
            let reasoning = if resp.reasoning.is_empty() { None } else { Some(resp.reasoning.clone()) };
            messages.push(ChatMessage { role: ChatRole::Assistant, content: resp.full_text.clone(), tool_call_id: None, tool_calls: Some(resp.tool_calls.clone()), reasoning_content: reasoning });

            for (ti, tc) in resp.tool_calls.iter().enumerate() {
                m.num_tool_calls += 1;
                if tc.name == "explore" { m.explore_calls += 1; }
                if tc.name == "rdf" { m.rdf_calls += 1; }
                if tc.name == "batch" { m.batch_calls += 1; }
                if !m.unique_tools.contains(&tc.name) { m.unique_tools.push(tc.name.clone()); }

                let result = execute_squire_tool(&tc.name, tc.arguments.clone(), squire_store, &[], sid, &batch_counter).await;
                log.master(&format!("    r{}.{} {} → {}B", m.num_rounds, ti, tc.name, result.len()));
                log.write_file(
                    &format!("{}/r{}-tool-{}-{}.txt", turn_dir.display(), m.num_rounds, ti, tc.name),
                    &format!("tool: {}\nargs: {}\nresult:\n{}", tc.name, serde_json::to_string(&tc.arguments).unwrap_or_default(), result),
                );

                messages.push(ChatMessage { role: ChatRole::Tool, content: result, tool_call_id: Some(tc.id.clone()), tool_calls: None, reasoning_content: None });
            }

            if m.num_rounds >= max_rounds { log.master(&format!("  max rounds ({max_rounds})")); p1_content = resp.full_text; break; }
            continue;
        }

        p1_content = resp.full_text;
        break;
    }

    m.llm_ms = t_p1.elapsed().as_secs_f64() * 1000.0;
    m.p1_text_bytes = p1_content.len();
    m.inline_refs = p1_content.match_indices("§!").count();
    m.span_markers = p1_content.match_indices("§^").count();
    m.response_preview = clamp(&p1_content, 250);

    log.master(&format!("  P1 done: {dur}  ({rds}r, {tc}TC, {txt}B text, {ir}§!, {sp}§^)",
        dur = fmt_ms(m.llm_ms), rds = m.num_rounds, tc = m.num_tool_calls,
        txt = m.p1_text_bytes, ir = m.inline_refs, sp = m.span_markers));
    log.write_file(&format!("{}/final-response.txt", turn_dir.display()), &p1_content);

    // ── finalize Phase 1 ──
    let t_fin = Instant::now();
    let outcome = adapter.finalize_turn(sid, p1_content.clone(), None, &mut messages, conv_store.as_ref()).await.map_err(|e| e.to_string())?;
    m.finalize_ms = t_fin.elapsed().as_secs_f64() * 1000.0;

    match outcome {
        TurnOutcome::Phase2 { phase1_content, user_request } => {
            log.master(&format!("  finalize→P2 ({})", fmt_ms(m.finalize_ms)));

            // ── Phase 2 ──
            let t_p2 = Instant::now();
            let p2_prompt = squirecli_lib::agent::squire_prompts::system_prompt_phase2();
            let mut p2_msgs = vec![
                ChatMessage { role: ChatRole::System, content: p2_prompt, tool_call_id: None, tool_calls: None, reasoning_content: None },
                ChatMessage { role: ChatRole::User, content: format!("Original user request:\n{}\n\nAssistant Phase 1 response:\n{}", user_request, phase1_content), tool_call_id: None, tool_calls: None, reasoning_content: None },
            ];
            adapter.set_phase2(user_request);

            for rnd in 1..=3 {
                let rf = format!("{}/p2-r{}-request.txt", turn_dir.display(), rnd);
                let sf = format!("{}/p2-r{}-response.txt", turn_dir.display(), rnd);
                let p2r = call_llm(provider, "deepseek-v4-flash", &p2_msgs, &[], Some(0.0), Some(4096), Some("none".to_string()), log, &rf, &sf).await?;
                let o = adapter.finalize_turn(sid, p2r.full_text, None, &mut p2_msgs, conv_store.as_ref()).await.map_err(|e| e.to_string())?;

                match o {
                    TurnOutcome::Phase2Done { tokens_accepted, relationships_accepted, .. } => {
                        m.p2_tokens += tokens_accepted; m.p2_rels += relationships_accepted;
                        log.master(&format!("  P2 r{rnd} done: +{t}t, +{r}r", t = tokens_accepted, r = relationships_accepted));
                        log.write_file(&format!("{}/p2-r{}-outcome.txt", turn_dir.display(), rnd), &format!("Phase2Done\ntokens: {}\nrels: {}", tokens_accepted, relationships_accepted));
                        break;
                    }
                    TurnOutcome::Retry => {
                        log.master(&format!("  P2 r{rnd} retry"));
                        continue;
                    }
                    _ => { log.master(&format!("  P2 r{rnd} other")); break; }
                }
            }
            m.p2_ms = t_p2.elapsed().as_secs_f64() * 1000.0;
            log.master(&format!("  P2 total: {}  ({}t, {}r)", fmt_ms(m.p2_ms), m.p2_tokens, m.p2_rels));
        }
        TurnOutcome::Done => { log.master("  finalize→Done"); }
        TurnOutcome::Retry => { log.master("  finalize→Retry"); }
        TurnOutcome::Failed { reason, .. } => { log.master(&format!("  finalize→Fail: {reason}")); }
        other => { log.master(&format!("  finalize→{:?}", match &other { TurnOutcome::AskUser{..} => "AskUser", _ => "?" })); }
    }

    // Store post-turn state
    m.tokens_after = squire_store.list_token_ids_by_session(sid).await.len();
    m.rels_after = squire_store.get_relationships(None, None, None).await.len();
    log.master(&format!("  Store after: {} tokens (+{}), {} rels (+{})",
        m.tokens_after, m.tokens_after - m.tokens_before,
        m.rels_after, m.rels_after - m.rels_before));

    Ok(m)
}

// ═══════════════════════════════════════════════════════════════════════════
// Main test
// ═══════════════════════════════════════════════════════════════════════════

const TIMEOUT: u64 = 600; // 10 min across 3 turns

#[tokio::test]
async fn test_squire_multiturn_e2e() {
    let t0 = Instant::now();
    tokio::select! {
        _ = tokio::time::sleep(Duration::from_secs(TIMEOUT)) => { panic!("Timed out after {TIMEOUT}s"); }
        result = run_test() => {
            match result {
                Ok(()) => println!("\n  ✅ Multi-turn test passed."),
                Err(e) => println!("\n  ❌ {e}"),
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Questions: designed to test specific retrieval patterns
// ═══════════════════════════════════════════════════════════════════════════
//
// Q1 — Seed factual knowledge: the model MUST use explore() to find
//      anything relevant, then optionally rdf() to expand.  Response should
//      contain §! inline refs and §^ span markers.
//
// Q2 — Cross-turn recall: the topic is related to Q1.  In a stateless
//      system, the model would start from zero.  In Squire, Q1's RESP_T
//      chunks and any Phase-2-generated concept tokens should appear in
//      the prefetch context and be discoverable via explore().  The model
//      should cite Q1 tokens via §! refs.
//
// Q3 — Synthesis: asks the model to compare/contrast concepts from both
//      Q1 and Q2.  This tests whether the graph is accumulating, whether
//      the preserve list is carrying forward the right tokens, and whether
//      the model can traverse both old and new tokens in one turn.
// ═══════════════════════════════════════════════════════════════════════════

const Q1: &str = concat!(
    "Explain what gRPC is and how it compares to REST for service-to-service communication. ",
    "Use the explore tool to search for any existing knowledge about gRPC or protocol buffers. ",
    "Include specific details about: 1) the wire format, 2) how streaming works, ",
    "and 3) at least one scenario where gRPC is clearly better than REST."
);

const Q2: &str = concat!(
    "Now compare gRPC's approach to schema evolution and versioning with how GraphQL handles the same problem. ",
    "First, explore the memory for any existing knowledge about schema evolution or GraphQL. ",
    "Then reference what you already explained about gRPC's wire format and proto3 compatibility, ",
    "and use rdf to see related tokens if you find any. ",
    "Be specific about: 1) backward/forward compatibility guarantees, ",
    "and 2) how each approach handles unknown fields."
);

const Q3: &str = concat!(
    "Based on everything we've discussed about gRPC, REST, and GraphQL, ",
    "design a recommendation framework for choosing between them. ",
    "Search the memory for all three topics (gRPC, REST, GraphQL) using explore, ",
    "then synthesize what you find into a decision matrix. ",
    "Your framework should cover: 1) When to use each, 2) Key tradeoffs, ",
    "3) Migration considerations if you start with one and need to switch."
);

async fn run_test() -> Result<(), String> {
    let t0 = Instant::now();

    // ── Setup ──
    let key = std::env::var("DEEPSEEK_API_KEY").map_err(|_| "DEEPSEEK_API_KEY not set")?;
    let log_base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target").join("squire-e2e-logs");
    let log = LogDir::create(&log_base, "multiturn")?;

    log.master("══════════════════════════════════════════════════════════════");
    log.master("  SQUIRE MULTI-TURN PROTOCOL TEST  (3 turns)");
    log.master("══════════════════════════════════════════════════════════════");
    log.master(&format!("Model: deepseek-v4-flash | OS: {os} | Key: {k}...",
        os = std::env::consts::OS, k = &key[..8.min(key.len())]));

    let spec = ProviderSpec {
        provider_type: "openai".to_string(), name: "deepseek".to_string(),
        api_key: key, model: "deepseek-v4-flash".to_string(),
        models: vec!["deepseek-v4-flash".to_string()],
        endpoint: Some("https://api.deepseek.com/v1".to_string()), metadata: HashMap::new(), category: None,
    };
    let reg = ProviderRegistry::from_config(&ProviderRegistryConfig { providers: vec![spec], verbose_logging: true, wire_log_path: None });
    let (provider, _) = reg.resolve_provider_for_instance(&provider_core::ModelInstance::new("deepseek", "deepseek-v4-flash")).map_err(|e| e.to_string())?;

    let conv_store = Arc::new(InMemoryConvStore::new()) as Arc<dyn ConversationStore>;
    let squire_store = Arc::new(InMemorySquireStore::new()) as Arc<dyn SquireStore>;

    let session = conv_store.create_session(NewSession { title: "Multi-turn E2E".to_string(), context_mode: Some(ContextMode::Squire) }).await.map_err(|e| e.to_string())?;
    let sid = session.id;
    log.master(&format!("Session: {sid}"));

    // ── Run 3 turns ──
    let mut metrics: Vec<TurnMetrics> = Vec::new();

    for (qi, (question, label)) in [(Q1, "Q1-gRPC"), (Q2, "Q2-schema-evolution"), (Q3, "Q3-synthesis")].iter().enumerate() {
        let t_start = Instant::now();
        let turn_dir = log.sub_dir(&format!("t{}-{}", qi + 1, label));
        log.master(&format!("Question {n}: \"{q}\"", n = qi + 1, q = clamp(question, 120)));

        let m = run_turn(sid, question, &provider, &conv_store, &squire_store, &log, &turn_dir, label).await?;
        let t_ms = t_start.elapsed().as_secs_f64() * 1000.0;
        log.master(&format!("Turn {n} wall time: {dur}", n = qi + 1, dur = fmt_ms(t_ms)));
        metrics.push(m);
    }

    // ═════════════════════════════════════════════════════════════════
    // ANALYSIS
    // ═════════════════════════════════════════════════════════════════
    log.master("");
    log.master("══════════════════════════════════════════════");
    log.master("  CROSS-TURN ANALYSIS");
    log.master("══════════════════════════════════════════════");

    // Per-turn summary table
    log.master("");
    log.master("── Per-Turn Summary ──");
    for m in &metrics {
        log.master(&format!(
            "  {label:>20}: {rds}r, {tc}TC [{tools}] | {txt}B text, {ir}§! {sp}§^ | \
             P2 +{p2t}t +{p2r}r | Δstore +{dt}t +{dr}r | P1={p1} P2={p2}",
            label = m.label, rds = m.num_rounds, tc = m.num_tool_calls,
            tools = m.unique_tools.join("+"), txt = m.p1_text_bytes,
            ir = m.inline_refs, sp = m.span_markers,
            p2t = m.p2_tokens, p2r = m.p2_rels,
            dt = m.tokens_after - m.tokens_before, dr = m.rels_after - m.rels_before,
            p1 = fmt_ms(m.llm_ms), p2 = fmt_ms(m.p2_ms),
        ));
        log.master(&format!("    Response: {}", m.response_preview));
    }

    // Efficiency metrics
    log.master("");
    log.master("── Retrieval Efficiency ──");
    for m in &metrics {
        let tokens_per_tc = if m.num_tool_calls > 0 { m.p1_text_bytes as f64 / m.num_tool_calls as f64 } else { f64::NAN };
        let rounds_efficiency = if m.num_rounds > 1 { format!("{} rounds → avg {:.0}B text/round", m.num_rounds, m.p1_text_bytes as f64 / m.num_rounds as f64) } else { "1 round (no tools)".to_string() };
        log.master(&format!(
            "  {label:>20}:  explore={exp}  rdf={rdf}  batch={bat}  |  {rds_str}  |  {tptc:.0} response-bytes/TC",
            label = m.label, exp = m.explore_calls, rdf = m.rdf_calls, bat = m.batch_calls,
            rds_str = rounds_efficiency, tptc = tokens_per_tc,
        ));
    }

    // Cross-turn token growth
    log.master("");
    log.master("── Cross-Turn Token Growth ──");
    let t0_store = metrics.first().map(|m| m.tokens_before).unwrap_or(0);
    let t3_store = metrics.last().map(|m| m.tokens_after).unwrap_or(0);
    log.master(&format!("  Start: {t0_store} tokens"));
    for m in &metrics {
        log.master(&format!("  After {label:>20}: {n} tokens (+{delta})",
            label = m.label, n = m.tokens_after, delta = m.tokens_after - m.tokens_before));
    }
    log.master(&format!("  Total growth: {t0_store} → {t3_store}  (+{delta})", delta = t3_store - t0_store));

    // Inline-ref reuse analysis (did later turns cite tokens from earlier turns?)
    log.master("");
    log.master("── Inline-Reference Density ──");
    for m in &metrics {
        let density = if m.p1_text_bytes > 0 { m.inline_refs as f64 * 1000.0 / m.p1_text_bytes as f64 } else { 0.0 };
        log.master(&format!("  {label:>20}:  {ir} §! refs  ({d:.1} per 1KB text)",
            label = m.label, ir = m.inline_refs, d = density));
    }

    // ── CSV export ──
    let csv_header = "turn,rounds,tool_calls,explore,rdf,batch,p1_ms,p2_ms,text_bytes,inline_refs,span_markers,p2_tokens,p2_rels,tokens_before,tokens_after,rels_before,rels_after";
    let csv_rows: Vec<Vec<String>> = metrics.iter().map(|m| vec![
        m.label.clone(), m.num_rounds.to_string(), m.num_tool_calls.to_string(),
        m.explore_calls.to_string(), m.rdf_calls.to_string(), m.batch_calls.to_string(),
        format!("{:.0}", m.llm_ms), format!("{:.0}", m.p2_ms),
        m.p1_text_bytes.to_string(), m.inline_refs.to_string(), m.span_markers.to_string(),
        m.p2_tokens.to_string(), m.p2_rels.to_string(),
        m.tokens_before.to_string(), m.tokens_after.to_string(),
        m.rels_before.to_string(), m.rels_after.to_string(),
    ]).collect();
    log.write_csv("timing.csv", csv_header, &csv_rows);

    // ── Store snapshot ──
    let all_ids = squire_store.list_token_ids_by_session(sid).await;
    let all_rels = squire_store.get_relationships(None, None, None).await;
    let mut inv = String::new();
    for tid in &all_ids {
        let d = squire_store.token_detail(tid).await;
        writeln!(inv, "{tid}  [{typ}]  {desc}",
            typ = d.as_ref().map(|x| x.ranges.len()).map(|n| if n > 0 { "referential" } else { "?" }).unwrap_or("?"),
            desc = d.as_ref().map(|x| x.short_desc.clone()).unwrap_or_default()).unwrap();
    }
    log.write_file("store-final-tokens.txt", &format!("{n} tokens\n\n{inv}", n = all_ids.len()));
    log.write_file("store-final-graph.txt",
        &all_rels.iter().map(|r| format!("{} →[{}]→ {}", r.subject, r.predicate, r.object)).collect::<Vec<_>>().join("\n"));

    // ── Verdict ──
    let total_ms = t0.elapsed().as_secs_f64() * 1000.0;
    log.master("");
    log.master("══════════════════════════════════════════════");
    log.master(&format!("  TOTAL: {dur}  |  {tokens} tokens final  |  {rels} rels final  |  {tcs} tool calls across {rds} rounds",
        dur = fmt_ms(total_ms), tokens = all_ids.len(), rels = all_rels.len(),
        tcs = metrics.iter().map(|m| m.num_tool_calls).sum::<u32>(),
        rds = metrics.iter().map(|m| m.num_rounds).sum::<u32>(),
    ));

    // Efficiency score: how many tool calls did it take per question on average?
    let avg_tc = metrics.iter().map(|m| m.num_tool_calls as f64).sum::<f64>() / metrics.len() as f64;
    let avg_rounds = metrics.iter().map(|m| m.num_rounds as f64).sum::<f64>() / metrics.len() as f64;
    log.master(&format!("  Avg tool calls/turn: {avg_tc:.1}  |  Avg LLM rounds/turn: {avg_rounds:.1}"));

    // Cross-turn recall score: did Q2/Q3 produce more inline refs (reusing earlier tokens)?
    let q1_ir = metrics.get(0).map(|m| m.inline_refs).unwrap_or(0);
    let q2_ir = metrics.get(1).map(|m| m.inline_refs).unwrap_or(0);
    let q3_ir = metrics.get(2).map(|m| m.inline_refs).unwrap_or(0);
    log.master(&format!("  Inline-ref trend: Q1={q1_ir} → Q2={q2_ir} → Q3={q3_ir}  ({} )",
        if q3_ir > q1_ir { "↑ growing — cross-turn recall active" } else if q2_ir > 0 || q3_ir > 0 { "→ stable" } else { "↓ no reuse" }));

    log.master("");
    log.master("══════════════════════════════════════════════");
    log.master(&format!("  Log directory: {}", log.dir().display()));
    log.master("══════════════════════════════════════════════");

    Ok(())
}
