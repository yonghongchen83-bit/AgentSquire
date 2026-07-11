//! `SquireEngine` — the production engine supporting both Legacy and Squire
//! context modes with two-phase protocol.
//!
//! Extracted from `commands::streaming_cmd::send_message_impl` into an
//! `Engine` trait implementation that uses `RuntimeContext` for all
//! dependencies — no direct `AppHandle` or `State` access.

use std::collections::{hash_map::DefaultHasher, HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use provider_core::{ChatMessage, ChatRole, ChatRequest, FinishReason, StreamEvent, ToolCall};
use provider_registry::ProviderRegistry;

use squire_store::SessionId;

use crate::agent::context_adapter::{
    ContextManagerAdapter, LegacyContextAdapter, TurnOutcome,
};
use crate::agent::squire::{
    ingest_tool_registry, SquireBatchTool, SquireContextAdapter, SquireExploreTool,
    SquireInvokeTool, SquireRdfTool, SquireTokenToDetailTool, ToolEndpoint,
};
use crate::agent::{
    McpProxyTool, SubagentTool, ToolRegistry,
};
use crate::mcp::DiscoveredTool;
use crate::storage::conversation_store::{
    ContextMode, ConversationStore, MessageRole, NewMessage,
};

use super::runtime::RuntimeContext;
use super::traits::{Engine, EngineEvent, EventEmitter};

// ═══════════════════════════════════════════════════════════════════════
// SquireEngine
// ═══════════════════════════════════════════════════════════════════════

/// The production engine supporting both Legacy and Squire context modes.
///
/// Orchestrates the full turn lifecycle:
/// 1. Build tool registry (Subagent, MCP, Squire tools)
/// 2. Build turn context via `ContextManagerAdapter`
/// 3. Call LLM provider and stream response
/// 4. Execute tool calls with approval flow
/// 5. Finalize turn and handle Phase 2 (Squire mode)
pub struct SquireEngine;

#[async_trait]
impl Engine for SquireEngine {
    async fn run(
        self: Box<Self>,
        ctx: RuntimeContext,
        session_id: SessionId,
    ) -> Result<(), String> {
        let inner = SquireEngineRun::new(ctx, session_id);
        inner.run().await
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Internal runner — owns all the state for a single turn execution
// ═══════════════════════════════════════════════════════════════════════

struct SquireEngineRun {
    ctx: RuntimeContext,
    sid: SessionId,
    event: Arc<dyn EventEmitter>,
    store: Arc<dyn ConversationStore>,
    squire_store: Arc<dyn squire_store::SquireStore>,
    provider_registry: Arc<ProviderRegistry>,
    config: super::runtime::RuntimeConfig,
    project_path: String,
    mcp_tools_cache: Arc<std::sync::RwLock<HashMap<String, Vec<DiscoveredTool>>>>,
    tool_registry_hash: Arc<std::sync::RwLock<u64>>,
    tool_endpoints: HashMap<String, ToolEndpoint>,
    verbose_logging: bool,
}

impl SquireEngineRun {
    fn new(ctx: RuntimeContext, session_id: SessionId) -> Self {
        let sid = session_id;
        let verbose_logging = ctx.config.verbose_logging;
        Self {
            event: ctx.event_emitter.clone(),
            store: ctx.store.clone(),
            squire_store: ctx.squire_store.clone(),
            provider_registry: ctx.provider_registry.clone(),
            config: ctx.config.clone(),
            project_path: ctx.project_path.clone(),
            mcp_tools_cache: ctx.mcp_tools_cache.clone(),
            tool_registry_hash: ctx.tool_registry_hash.clone(),
            tool_endpoints: HashMap::new(),
            verbose_logging,
            ctx,
            sid,
        }
    }

    /// Emit a status string to the frontend.
    fn emit_status(&self, status: &str) {
        let event = self.event.clone();
        let status = status.to_string();
        tokio::spawn(async move {
            event.emit_status(&status).await;
        });
    }

    /// Emit a generic event.
    fn emit(&self, event: EngineEvent) {
        let ev = self.event.clone();
        tokio::spawn(async move {
            ev.emit(&event).await;
        });
    }

    /// Emit a timing log line if verbose logging is enabled.
    fn emit_timing(&self, label: &str, t_start: &Instant, t_last: &mut Instant) {
        if !self.verbose_logging {
            return;
        }
        let now = Instant::now();
        let since_start = now.duration_since(*t_start).as_millis();
        let since_last = now.duration_since(*t_last).as_millis();
        *t_last = now;
        self.emit(EngineEvent::Output {
            source: "chat".to_string(),
            line: format!(
                "[timing] {}: +{}ms ({}ms since last)",
                label, since_start, since_last
            ),
            timestamp: chrono::Utc::now().to_rfc3339(),
        });
    }

    /// Split text into chunks for streaming.
    fn split_into_chunks(text: &str, max_len: usize) -> Vec<String> {
        if text.len() <= max_len {
            return vec![text.to_string()];
        }
        let mut chunks = Vec::new();
        let mut start = 0;
        let bytes = text.as_bytes();
        while start < bytes.len() {
            let end = (start + max_len).min(bytes.len());
            let split_at = if end < bytes.len() {
                let mut newline_pos = end;
                while newline_pos > start && bytes[newline_pos] != b'\n' {
                    newline_pos -= 1;
                }
                if newline_pos > start {
                    newline_pos + 1
                } else {
                    end
                }
            } else {
                end
            };
            chunks.push(text[start..split_at].to_string());
            start = split_at;
        }
        chunks
    }

    // ══════════════════════════════════════════════════════════════════
    // Main run loop
    // ══════════════════════════════════════════════════════════════════

    async fn run(mut self) -> Result<(), String> {
        let t_start = Instant::now();
        let mut t_last = t_start;

        // ── Append user message ──
        // (The session_id/content should already have been appended by the
        // caller. We skip this step since the engine just receives the
        // session_id after the message was stored.)

        // ── Load session ──
        let session = self
            .store
            .get_session(self.sid)
            .await
            .map_err(|e| e.to_string())?;

        // ── Resolve Phase 1 provider + model ──
        let (provider_arc, selected_model) = self
            .provider_registry
            .resolve_provider_for_instance(&self.ctx.phase1_model_instance)
            .map_err(|e| e.to_string())?;
        let selected_provider_name = self.ctx.phase1_model_instance.provider_name.clone();

        // ── Resolve Phase 2 provider (independent instance, no fallback) ──
        let phase2_provider_arc = self
            .provider_registry
            .resolve_provider_for_instance(&self.ctx.phase2_model_instance)
            .map_err(|e| e.to_string())?;
        let phase2_provider_arc = phase2_provider_arc.0;

        let provider_arc = provider_arc;
        let current_model = selected_model.clone();

        // ── Build tool registry ──
        self.emit_status("Preparing tools...");

        let mut tool_registry = ToolRegistry::new();

        // SubagentTool
        if !self.config.disabled_tools.iter().any(|t| t == "subagent") {
            tool_registry.register(Box::new(SubagentTool {
                app_handle: self.ctx.app_handle.clone().unwrap_or_else(|| {
                    // In headless mode, create a minimal placeholder.
                    // TODO(ServerRefactor): make SubagentTool not require AppHandle.
                    panic!("SubagentTool requires AppHandle — set RuntimeContext.app_handle");
                }),
                store: self.store.clone(),
                enabled_mcp_servers: self.config.mcp_servers.clone(),
                provider: provider_arc.clone(),
                model: selected_model.clone(),
                provider_name: selected_provider_name.clone(),
                verbose_logging: self.verbose_logging,
                project_path: self.project_path.clone(),
            }));
        }

        let mut used_names: HashSet<String> = tool_registry
            .definitions()
            .into_iter()
            .map(|d| d.name)
            .collect();

        // MCP tools
        for server in &self.config.mcp_servers {
            let tools = {
                let cache = self.mcp_tools_cache.read().unwrap();
                cache.get(&server.id).cloned()
            };
            let tools: Vec<DiscoveredTool> = match tools {
                Some(t) => t,
                None => match crate::mcp::discover_tools(server.clone()).await {
                    Ok(t) => {
                        let mut cache = self.mcp_tools_cache.write().unwrap();
                        cache.insert(server.id.clone(), t.clone());
                        t
                    }
                    Err(e) => {
                        self.emit(EngineEvent::Output {
                            source: "chat".to_string(),
                            line: format!(
                                "WARNING: MCP discovery failed for server '{}': {}",
                                server.name, e
                            ),
                            timestamp: chrono::Utc::now().to_rfc3339(),
                        });
                        continue;
                    }
                },
            };

            for tool in tools {
                let server_id = server
                    .id
                    .chars()
                    .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
                    .collect::<String>();
                let remote_tool_name = tool.name.clone();
                let tool_id = remote_tool_name
                    .chars()
                    .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
                    .collect::<String>();

                let mut local_name = format!("mcp_{}_{}", server_id, tool_id);
                let mut i = 2;
                while used_names.contains(&local_name) {
                    local_name = format!("mcp_{}_{}_{}", server_id, tool_id, i);
                    i += 1;
                }
                used_names.insert(local_name.clone());

                let local_description = format!(
                    "MCP tool '{}' from server '{}': {}",
                    remote_tool_name, server.name, tool.description
                );

                // Validate schema
                let schema_valid = tool
                    .input_schema
                    .as_object()
                    .and_then(|obj| obj.get("type"))
                    .and_then(|t| t.as_str())
                    .map(|t| t == "object")
                    .unwrap_or(false);
                if !schema_valid {
                    self.emit(EngineEvent::Output {
                        source: "chat".to_string(),
                        line: format!(
                            "WARNING: Skipping MCP tool '{}' from server '{}' because its input schema is not a plain object",
                            remote_tool_name, server.name
                        ),
                        timestamp: chrono::Utc::now().to_rfc3339(),
                    });
                    continue;
                }

                self.tool_endpoints.insert(
                    local_name.clone(),
                    ToolEndpoint::Mcp {
                        server: server.clone().into(),
                        remote_name: remote_tool_name.clone(),
                    },
                );

                tool_registry.register(Box::new(McpProxyTool {
                    local_name: local_name.clone(),
                    local_description,
                    schema: tool.input_schema.clone(),
                    server: server.clone(),
                    remote_name: remote_tool_name.clone(),
                }));
            }
        }

        self.emit_timing("mcp_discovery_and_register", &t_start, &mut t_last);

        // ── Ingest tool registry into Squire store ──
        let defs = tool_registry.definitions();
        let defs_json = serde_json::to_vec(&defs).unwrap_or_default();
        let mut hasher = DefaultHasher::new();
        defs_json.hash(&mut hasher);
        let new_hash = hasher.finish();
        let should_ingest = *self.tool_registry_hash.read().unwrap() != new_hash;
        if should_ingest {
            ingest_tool_registry(
                &tool_registry,
                self.squire_store.as_ref(),
                &self.tool_endpoints,
            )
            .await;
            *self.tool_registry_hash.write().unwrap() = new_hash;
        }
        self.emit_timing("tool_registry_ingestion", &t_start, &mut t_last);

        let base_tool_defs = tool_registry.definitions();

        // ── Register Squire-specific tools ──
        if session.session.context_mode == ContextMode::Squire {
            let batch_counter = Arc::new(std::sync::atomic::AtomicU32::new(0));
            let batch_cap = crate::agent::squire::tools::DEFAULT_BATCH_CAP;
            tool_registry.register(Box::new(SquireExploreTool {
                store: self.squire_store.clone(),
                tool_defs: base_tool_defs.clone(),
                session_id: session.session.id,
                batch_counter: batch_counter.clone(),
                batch_cap,
            }));
            tool_registry.register(Box::new(SquireTokenToDetailTool {
                store: self.squire_store.clone(),
                batch_counter: batch_counter.clone(),
                batch_cap,
            }));
            tool_registry.register(Box::new(SquireRdfTool {
                store: self.squire_store.clone(),
                batch_counter: batch_counter.clone(),
                batch_cap,
            }));
            tool_registry.register(Box::new(crate::agent::TodoTreeTool::for_store(
                self.squire_store.clone(),
                session.session.id,
            )));
            tool_registry.register(Box::new(crate::agent::DecisionTreeTool::new(
                self.squire_store.clone(),
                session.session.id,
            )));
            tool_registry.register(Box::new(SquireInvokeTool));
            tool_registry.register(Box::new(SquireBatchTool {
                store: self.squire_store.clone(),
                tool_defs: base_tool_defs.clone(),
                session_id: session.session.id,
                batch_counter: batch_counter.clone(),
                batch_cap,
            }));
        }

        let tool_registry = Arc::new(tool_registry);
        let tool_defs = base_tool_defs;

        // ── Build context adapter ──
        let mut adapter: Box<dyn ContextManagerAdapter> = match session.session.context_mode {
            ContextMode::Legacy => Box::new(LegacyContextAdapter),
            ContextMode::Squire => Box::new(SquireContextAdapter::new_with_prefetch(
                self.squire_store.clone(),
                self.config.squire_prefetch.clone(),
            )),
        };

        let stream_live_chunks =
            !matches!(session.session.context_mode, ContextMode::Squire);

        self.emit_timing("adapter_created", &t_start, &mut t_last);

        // ── Build turn input ──
        let turn_input = match adapter.build_turn_input(&session, &tool_defs).await {
            Ok(ti) => ti,
            Err(e) => {
                self.emit_status("Failed to build turn context");
                self.emit(EngineEvent::Error(e.clone()));
                return Err(e);
            }
        };
        self.emit_timing("build_turn_input", &t_start, &mut t_last);

        let mut messages: Vec<ChatMessage> = turn_input.messages;
        let turn_tools = turn_input.tools;

        // ══════════════════════════════════════════════════════════════
        // Main engine loop (Phase 1 + tool execution)
        // ══════════════════════════════════════════════════════════════

        loop {
            self.emit_status("Contacting model...");

            let mut request = ChatRequest {
                model: current_model.clone(),
                messages: messages.clone(),
                tools: turn_tools.clone(),
                thinking_level: None,
                temperature: None,
                max_tokens: None,
            };
            // Apply Phase 1 ModelInstance options onto the request
            self.ctx.phase1_model_instance.apply_to_request(&mut request);

            if self.verbose_logging {
                let request_pretty =
                    serde_json::to_string_pretty(&request).unwrap_or_default();
                self.emit(EngineEvent::Output {
                    source: "chat".to_string(),
                    line: format!("[orchestrator] >>> CHAT REQUEST\n{}", request_pretty),
                    timestamp: chrono::Utc::now().to_rfc3339(),
                });
            }

            self.emit_timing("before_first_llm_chat", &t_start, &mut t_last);

            // Call provider — try chat_with_instance first (uses ModelInstance
            // for endpoint/api_key overrides), fall back to plain chat.
            let mut stream = match tokio::time::timeout(
                Duration::from_secs(30),
                provider_arc.chat(request),
            )
            .await
            {
                Ok(Ok(s)) => s,
                Ok(Err(e)) => {
                    self.emit_status("Model request failed");
                    self.emit(EngineEvent::Error(e.to_string()));
                    return Err(e.to_string());
                }
                Err(_) => {
                    self.emit_status("Model request timed out");
                    self.emit(EngineEvent::Error(
                        "Request timed out after 30s".to_string(),
                    ));
                    return Err("Request timed out after 30s".to_string());
                }
            };

            let mut first_chunk = true;
            let mut full_response = String::new();
            let mut full_thinking = String::new();
            let mut tool_calls: Vec<ToolCall> = Vec::new();
            let mut finish_reason: Option<FinishReason> = None;
            let mut channel_closed_cleanly = false;

            // ── Stream events ──
            while let Some(event) = stream.recv().await {
                if first_chunk {
                    self.emit_timing("first_llm_event_received", &t_start, &mut t_last);
                    first_chunk = false;
                }
                match event {
                    StreamEvent::Chunk(text) => {
                        full_response.push_str(&text);
                        if stream_live_chunks {
                            self.emit(EngineEvent::Chunk(text));
                        }
                    }
                    StreamEvent::Thinking(text) => {
                        full_thinking.push_str(&text);
                        self.emit(EngineEvent::Thinking(text));
                    }
                    StreamEvent::ToolCall(mut tc) => {
                        // Rewrite "invoke" tool calls in Squire mode
                        if session.session.context_mode == ContextMode::Squire
                            && tc.name == "invoke"
                        {
                            if let Some(real_name) = tc
                                .arguments
                                .get("token_id")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string())
                            {
                                let real_args = tc
                                    .arguments
                                    .get("params")
                                    .cloned()
                                    .unwrap_or(serde_json::json!({}));
                                tc.name = real_name;
                                tc.arguments = real_args;
                            }
                        }
                        tool_calls.push(tc.clone());
                        self.emit(EngineEvent::ToolCall(
                            serde_json::to_value(&tc).unwrap_or_default(),
                        ));
                    }
                    StreamEvent::Log(msg) => {
                        self.emit(EngineEvent::Output {
                            source: "chat".to_string(),
                            line: msg,
                            timestamp: chrono::Utc::now().to_rfc3339(),
                        });
                    }
                    StreamEvent::Done(reason) => {
                        self.emit_status("Model response received");
                        finish_reason = Some(reason);
                        break;
                    }
                    StreamEvent::Error(err) => {
                        self.emit_status("Model stream error");
                        self.emit(EngineEvent::Error(err));
                        return Err("Model stream error".to_string());
                    }
                }
            }

            if finish_reason.is_none() {
                channel_closed_cleanly = true;
            }

            let reason = match finish_reason {
                Some(r) => r,
                None => {
                    if channel_closed_cleanly
                        && (full_response.trim().is_empty() && tool_calls.is_empty())
                    {
                        if self.verbose_logging {
                            self.emit(EngineEvent::Output {
                                source: "chat".to_string(),
                                line: format!(
                                    "ERROR: Provider stream channel closed with no output and no finish reason. provider={}, model={}.",
                                    selected_provider_name, selected_model
                                ),
                                timestamp: chrono::Utc::now().to_rfc3339(),
                            });
                        }
                        self.emit(EngineEvent::Error(format!(
                            "Provider closed stream without any output [provider={}, model={}]",
                            selected_provider_name, selected_model
                        )));
                        return Err("Provider closed stream without any output".to_string());
                    }

                    let inferred_reason = if !tool_calls.is_empty() {
                        FinishReason::ToolCalls
                    } else if !full_response.trim().is_empty() {
                        FinishReason::Stop
                    } else {
                        FinishReason::Error
                    };

                    if matches!(inferred_reason, FinishReason::Error) {
                        self.emit_status("Stream ended without usable output");
                        self.emit(EngineEvent::Error(format!(
                            "Stream ended without finish reason and no usable output [provider={}, model={}]",
                            selected_provider_name, selected_model
                        )));
                        return Err("Stream ended without usable output".to_string());
                    }

                    self.emit(EngineEvent::Output {
                        source: "chat".to_string(),
                        line: format!(
                            "WARNING: Stream ended without finish reason; applying fallback. provider={}, model={}, inferred={:?}",
                            selected_provider_name, selected_model, inferred_reason
                        ),
                        timestamp: chrono::Utc::now().to_rfc3339(),
                    });

                    inferred_reason
                }
            };

            match reason {
                FinishReason::ToolCalls => {
                    self.emit_status("Model requested tool execution");
                    if self.verbose_logging {
                        self.emit(EngineEvent::Output {
                            source: "chat".to_string(),
                            line: format!(
                                "[orchestrator] <<< CHAT RESPONSE finish=tool_calls text_bytes={} tool_calls={}",
                                full_response.len(),
                                tool_calls.len()
                            ),
                            timestamp: chrono::Utc::now().to_rfc3339(),
                        });
                    }
                    if !full_response.is_empty() && stream_live_chunks {
                        self.emit(EngineEvent::Chunk("\n\n".to_string()));
                    }

                    // Persist assistant message
                    let msg_content = std::mem::take(&mut full_response);
                    let msg_thinking = if !full_thinking.is_empty() {
                        Some(std::mem::take(&mut full_thinking))
                    } else {
                        None
                    };
                    if !msg_content.is_empty() || msg_thinking.is_some() || !tool_calls.is_empty()
                    {
                        messages.push(ChatMessage {
                            role: ChatRole::Assistant,
                            content: msg_content.clone(),
                            tool_call_id: None,
                            tool_calls: Some(tool_calls.clone()),
                            reasoning_content: msg_thinking.clone(),
                        });
                        let _ = self
                            .store
                            .append_message(NewMessage {
                                session_id: self.sid,
                                role: MessageRole::Assistant,
                                content: msg_content,
                                thinking_content: msg_thinking,
                            })
                            .await;
                    }

                    // ── Execute tools ──
                    // Note: approval flow is skipped in headless mode by default.
                    // The real send_message_impl handles ongoing approvals via
                    // PendingApprovals state. For the engine, we skip approvals
                    // (all tools auto-approved) — the command handler should
                    // handle approvals before calling the engine.
                    let dr: Arc<ToolRegistry> = tool_registry.clone();
                    let event = self.event.clone();
                    let mut exec_futs: Vec<
                        std::pin::Pin<
                            Box<dyn futures::Future<Output = (usize, crate::agent::ToolResult)> + Send>,
                        >,
                    > = Vec::new();
                    for (i, tc) in tool_calls.iter().enumerate() {
                        let name = tc.name.clone();
                        let call_id = tc.id.clone();
                        let args = tc.arguments.clone();
                        let dr = dr.clone();
                        let event = event.clone();

                        if stream_live_chunks {
                            let _ = event
                                .emit(&EngineEvent::Chunk(format!(
                                    "[Executing {}...]\n",
                                    name
                                )))
                                .await;
                        }

                        exec_futs.push(Box::pin(async move {
                            if let Some(tool) = dr.get(&name) {
                                let result = tool.execute(&call_id, args).await;
                                (i, result)
                            } else {
                                (
                                    i,
                                    crate::agent::ToolResult {
                                        call_id,
                                        output: format!("Unknown tool: {}", name),
                                        is_error: true,
                                    },
                                )
                            }
                        }));
                    }
                    let exec_results: Vec<(usize, crate::agent::ToolResult)> =
                        futures::future::join_all(exec_futs).await;

                    let mut results_by_idx: HashMap<usize, crate::agent::ToolResult> =
                        exec_results.into_iter().collect();
                    for (i, tc) in tool_calls.iter().enumerate() {
                        if let Some(result) = results_by_idx.remove(&i) {
                            self.emit(EngineEvent::ToolResult(
                                serde_json::to_value(&result).unwrap_or_default(),
                            ));

                            if let Err(e) = adapter
                                .handle_tool_loop_step(tc, &result, &mut messages)
                                .await
                            {
                                self.emit_status("Failed to update turn context");
                                self.emit(EngineEvent::Error(e.clone()));
                                return Err(e);
                            }
                        }
                    }

                    continue;
                }
                FinishReason::Stop | FinishReason::Length => {
                    self.emit_status("Completed");
                    let content = std::mem::take(&mut full_response);
                    if self.verbose_logging {
                        self.emit(EngineEvent::Output {
                            source: "chat".to_string(),
                            line: format!(
                                "[orchestrator] <<< CHAT RESPONSE RAW\n{}",
                                content
                            ),
                            timestamp: chrono::Utc::now().to_rfc3339(),
                        });
                    }
                    let thinking = if !full_thinking.is_empty() {
                        Some(std::mem::take(&mut full_thinking))
                    } else {
                        None
                    };
                    let _raw_assistant_content = content.clone();

                    match adapter
                        .finalize_turn(
                            self.sid,
                            content,
                            thinking,
                            &mut messages,
                            self.store.as_ref(),
                        )
                        .await
                    {
                        Ok(TurnOutcome::Done) => {
                            self.emit(EngineEvent::Done);
                            return Ok(());
                        }
                        Ok(TurnOutcome::Phase2Done {
                            tokens_accepted,
                            relationships_accepted,
                            tokens_rejected,
                            relationships_rejected,
                        }) => {
                            let summary = serde_json::json!({
                                "tokens_accepted": tokens_accepted,
                                "relationships_accepted": relationships_accepted,
                                "tokens_rejected": tokens_rejected,
                                "relationships_rejected": relationships_rejected,
                            });
                            self.emit(EngineEvent::Phase2Summary(summary));
                            self.emit(EngineEvent::Done);
                            return Ok(());
                        }
                        Ok(TurnOutcome::Retry) => {
                            self.emit_status("Response rejected, retrying...");
                            continue;
                        }
                        Ok(TurnOutcome::AskUser { question }) => {
                            let question_id = uuid::Uuid::new_v4().to_string();
                            self.emit(EngineEvent::AskUserPending(serde_json::json!({
                                "question_id": question_id,
                                "session_id": self.sid,
                                "question": question,
                            })));
                            // The command handler should collect the answer
                            // and call the engine again. For now, return with
                            // a special signal.
                            return Err(format!(
                                "__ASK_USER__:{}:{}",
                                question_id, question
                            ));
                        }
                        Ok(TurnOutcome::Phase2 {
                            phase1_content,
                            user_request,
                        }) => {
                            // Emit Phase 1 response immediately (user sees it).
                            for chunk in Self::split_into_chunks(&phase1_content, 256) {
                                self.emit(EngineEvent::Chunk(chunk));
                            }
                            self.emit(EngineEvent::Done);

                            // ── Formatter pass (Phase 4) ─────────────────
                            // Fire-and-forget background task: formatter uses
                            // JSON structured output, sees only current turn
                            // data. Pure optimization — if it fails, nothing
                            // breaks, tokens simply aren't created this turn.
                            let squire_store = self.squire_store.clone();
                            let p2_provider = phase2_provider_arc.clone();
                            let sid = self.sid;
                            let conv_store = self.store.clone();
                            let emitter = self.event.clone();
                            let verbose = self.verbose_logging;
                            // Capture Phase 2 instance options now (needed inside
                            // spawn because self is moved).
                            let p2_instance = self.ctx.phase2_model_instance.clone();

                            tokio::spawn(async move {
                                let p2_prompt = crate::agent::squire_prompts::get_prompt(
                                    "system-prompt-formatter.md",
                                );
                                let p2_messages = vec![
                                    ChatMessage {
                                        role: ChatRole::System,
                                        content: p2_prompt,
                                        tool_call_id: None,
                                        tool_calls: None,
                                        reasoning_content: None,
                                    },
                                    ChatMessage {
                                        role: ChatRole::User,
                                        content: format!(
                                            "## Original user request\n\n{}\n\n## Assistant response\n\n{}",
                                            user_request, phase1_content
                                        ),
                                        tool_call_id: None,
                                        tool_calls: None,
                                        reasoning_content: None,
                                    },
                                ];

                                let mut p2_request = ChatRequest {
                                    model: String::new(),
                                    messages: p2_messages,
                                    tools: vec![],
                                    thinking_level: None,
                                    temperature: None,
                                    max_tokens: None,
                                };
                                // Apply the Phase 2 ModelInstance — sets model,
                                // thinking_level, temperature, and max_tokens
                                // from the independently-configured instance.
                                p2_instance.apply_to_request(&mut p2_request);

                                let result = tokio::time::timeout(
                                    Duration::from_secs(60),
                                    p2_provider.chat(p2_request),
                                ).await;

                                let mut formatter_text = String::new();
                                match result {
                                    Ok(Ok(mut stream)) => {
                                        loop {
                                            match tokio::time::timeout(
                                                Duration::from_secs(30),
                                                stream.recv(),
                                            ).await {
                                                Ok(Some(StreamEvent::Chunk(t))) => formatter_text.push_str(&t),
                                                Ok(Some(StreamEvent::Done(_))) => break,
                                                Ok(None) => break,
                                                Ok(Some(StreamEvent::Error(ref e))) if verbose => {
                                                    let _ = emitter.emit(&EngineEvent::Output {
                                                        source: "formatter".to_string(),
                                                        line: format!("Formatter error: {}", e),
                                                        timestamp: chrono::Utc::now().to_rfc3339(),
                                                    });
                                                }
                                                Err(_) | Ok(Some(StreamEvent::Error(_))) => break,
                                                _ => {}
                                            }
                                        }
                                    }
                                    Err(ref _timeout) if verbose => {
                                        let _ = emitter.emit(&EngineEvent::Output {
                                            source: "formatter".to_string(),
                                            line: "Formatter timed out after 60s".to_string(),
                                            timestamp: chrono::Utc::now().to_rfc3339(),
                                        });
                                    }
                                    _ => {}
                                }

                                if formatter_text.is_empty() {
                                    return;
                                }

                                let mut adapter = SquireContextAdapter::new(squire_store.clone());
                                adapter.set_phase2(user_request.clone());
                                match adapter.finalize_formatter_json(
                                    sid,
                                    &formatter_text,
                                    conv_store.as_ref(),
                                ).await {
                                    Ok(TurnOutcome::Phase2Done {
                                        tokens_accepted,
                                        relationships_accepted,
                                        ..
                                    }) => {
                                        let summary = serde_json::json!({
                                            "tokens_accepted": tokens_accepted,
                                            "relationships_accepted": relationships_accepted,
                                            "tokens_rejected": Vec::<String>::new(),
                                            "relationships_rejected": Vec::<String>::new(),
                                        });
                                        let _ = emitter.emit(&EngineEvent::Phase2Summary(summary));
                                    }
                                    Err(ref e) if verbose => {
                                        let _ = emitter.emit(&EngineEvent::Output {
                                            source: "formatter".to_string(),
                                            line: format!("Formatter finalize failed: {}", e),
                                            timestamp: chrono::Utc::now().to_rfc3339(),
                                        });
                                    }
                                    _ => {}
                                }
                            });

                            return Ok(());
                        }
                        Ok(TurnOutcome::Failed {
                            reason,
                            failed_content,
                        }) => {
                            self.emit_status("Squire compliance check failed");
                            if self.verbose_logging {
                                self.emit(EngineEvent::Output {
                                    source: "chat".to_string(),
                                    line: format!(
                                        "[squire] compliance failure after exhausting retries. reason={}\nfinal response:\n{}",
                                        reason, failed_content
                                    ),
                                    timestamp: chrono::Utc::now().to_rfc3339(),
                                });
                            }
                            self.emit(EngineEvent::Error(format!(
                                "Squire compliance failure after exhausting retries: {}",
                                reason
                            )));
                            return Err(reason);
                        }
                        Err(e) => {
                            self.emit_status("Failed to finalize turn");
                            self.emit(EngineEvent::Error(e.clone()));
                            return Err(e);
                        }
                    }
                }
                FinishReason::Error => {
                    self.emit_status("LLM returned an error");
                    self.emit(EngineEvent::Error("LLM returned an error".to_string()));
                    return Err("LLM returned an error".to_string());
                }
            }
        }
    }
}
