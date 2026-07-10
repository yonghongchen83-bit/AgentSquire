//! `SquireContextAdapter` — the `ContextManagerAdapter` implementation for
//! Squire context mode.
//!
//! Adapter control flow, strict tool-surface enforcement, protocol validation
//! gates that drive retry/compliance-failure classification (Q6), and the
//! turn-input-building / turn-finalizing logic (spec §9).

use std::collections::HashSet;
use std::sync::Arc;

use async_trait::async_trait;

use super::ingestion::{ingest_response_chunks, ingest_user_input_chunks};
use super::protocol::{
    check_malformed_sigils, extract_inline_refs, extract_spans, is_range_spec, parse_range_spec,
    resolve_range_spec, resolve_ranges, strip_span_markers, take_token_id,
    unmarked_residual,
};
use super::SquireStore;
use super::tools::built_in_tool_definitions;
use super::types::{ComplianceFailureRecord, detect_and_parse};
use crate::agent::context_adapter::{ContextManagerAdapter, TurnInput, TurnOutcome};
use crate::agent::ToolResult;
use crate::llm::provider::{ChatMessage, ChatRole, ToolCall, ToolDefinition};
use crate::state::config::SquirePrefetchConfig;
use crate::storage::conversation_store::{
    ConversationStore, MessageRole, NewMessage, SessionId, SessionWithMessages,
};

// ═══════════════════════════════════════════════════════════════════════
// System prompt — loaded from external file via squire_prompts module.
// Define the prompt in prompts/system-prompt.md. Users can override it
// at {config_dir}/prompts/system-prompt.md or project/.squire/prompts/system-prompt.md
// without recompiling.

// ═══════════════════════════════════════════════════════════════════════
// Two-phase protocol
// ═══════════════════════════════════════════════════════════════════════

/// Which phase of the two-turn Squire protocol the adapter is in.
///
/// Phase 1: the model generates response content with bookmarks and spans
/// (sigils + explorer tools). No token/relationship sections are required.
///
/// Phase 2: the model receives the Phase 1 response + original user request
/// and generates referential tokens, concept tokens, and relationships only
/// (no tools, no content text).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SquirePhase {
    Phase1,
    Phase2,
}

// ═══════════════════════════════════════════════════════════════════════
// Adapter struct
// ═══════════════════════════════════════════════════════════════════════

pub struct SquireContextAdapter {
    pub(crate) store: Arc<dyn SquireStore>,
    prefetch: SquirePrefetchConfig,
    max_retries: u32,
    retry_count: u32,
    phase: SquirePhase,
    /// The original user request text with §^chunk_N§^ bookmark markers,
    /// captured during build_turn_input for use in Phase 2.
    user_request_text: String,
}

impl SquireContextAdapter {
    pub fn new(store: Arc<dyn SquireStore>) -> Self {
        Self::new_with_prefetch(store, SquirePrefetchConfig::default())
    }

    pub fn new_with_prefetch(store: Arc<dyn SquireStore>, prefetch: SquirePrefetchConfig) -> Self {
        Self {
            store,
            prefetch,
            max_retries: 3,
            retry_count: 0,
            phase: SquirePhase::Phase1,
            user_request_text: String::new(),
        }
    }

    /// Switch the adapter into Phase 2 (token generation) mode.
    /// Called by the orchestrator after receiving `TurnOutcome::Phase2`.
    pub fn set_phase2(&mut self, user_request_text: String) {
        self.phase = SquirePhase::Phase2;
        self.user_request_text = user_request_text;
        self.retry_count = 0;
    }

    async fn expand_for_display(&self, content: &str) -> String {
        let stripped = strip_span_markers(content);
        let parts: Vec<&str> = stripped.split('§').collect();
        let mut out = String::new();
        out.push_str(parts[0]);
        for part in parts.iter().skip(1) {
            if let Some(rest) = part.strip_prefix('!') {
                let token_id = take_token_id(rest);
                let remainder = &rest[token_id.len()..];
                let detail = self.store.token_detail(&token_id).await;
                let resolved = match &detail {
                    Some(d) if !d.ranges.is_empty() => {
                        // Resolve ranges into concatenated text
                        resolve_ranges(&d.ranges, self.store.as_ref()).await
                    }
                    Some(d) => d.short_desc.clone(),
                    None => token_id.clone(),
                };
                out.push_str(&resolved);
                out.push_str(remainder);
            } else {
                out.push('§');
                out.push_str(part);
            }
        }
        out
    }

    /// Records a rejection and decides retry vs. final failure per Q6.
    fn reject(
        &mut self,
        messages: &mut Vec<ChatMessage>,
        failed_content: String,
        reason: String,
    ) -> TurnOutcome {
        self.retry_count += 1;
        if self.retry_count > self.max_retries {
            return TurnOutcome::Failed {
                reason,
                failed_content,
            };
        }
        messages.push(ChatMessage {
            role: ChatRole::Assistant,
            content: failed_content,
            tool_call_id: None,
            tool_calls: None,
            reasoning_content: None,
        });
        messages.push(ChatMessage {
            role: ChatRole::User,
            content: serde_json::json!({ "rejected": true, "reason": reason }).to_string(),
            tool_call_id: None,
            tool_calls: None,
            reasoning_content: None,
        });
        TurnOutcome::Retry
    }

    /// Wraps `reject` with the Q6 final-failure side effects: a short,
    /// machine-stable `rule` id classifying the reason (for the structured
    /// failure-metadata record), persisting that record, and — on final
    /// failure only — persisting a visible chat message so the user can
    /// inspect what the model actually produced, not just a transient error
    /// toast (Q6's explicit UX intent: "user can inspect the failed response
    /// and adjust next prompt/direction to avoid repeated failure").
    async fn reject_and_record(
        &mut self,
        session_id: SessionId,
        messages: &mut Vec<ChatMessage>,
        failed_content: String,
        reason: String,
        conv_store: &dyn ConversationStore,
    ) -> Result<TurnOutcome, String> {
        let retry_count_before = self.retry_count;
        let outcome = self.reject(messages, failed_content.clone(), reason.clone());

        if let TurnOutcome::Failed { .. } = &outcome {
            self.store
                .record_compliance_failure(ComplianceFailureRecord {
                    session_id,
                    rule: classify_rejection_rule(&reason),
                    reason: reason.clone(),
                    retry_count: retry_count_before + 1,
                    failed_content: failed_content.clone(),
                    timestamp: chrono::Utc::now(),
                })
                .await;

            // Reset so a subsequent turn on the same session (a fresh
            // adapter instance, since it's constructed per-turn) doesn't
            // inherit a stale count — defensive, not currently reachable
            // since this adapter instance is discarded after this call.
            self.retry_count = 0;

            let visible = format!(
                "**Squire compliance failure — turn closed without a stored response**\n\n\
                 Reason: {reason}\n\n\
                 The model's final (rejected) response is shown below for reference. \
                 Consider adjusting your next message to avoid the same issue.\n\n\
                 ---\n{failed_content}"
            );
            conv_store
                .append_message(NewMessage {
                    session_id,
                    role: MessageRole::Assistant,
                    content: visible,
                    thinking_content: None,
                })
                .await
                .map_err(|e| e.to_string())?;
        }

        Ok(outcome)
    }
}

/// Best-effort classification of a free-text rejection reason into a short,
/// stable rule id for the structured failure record (Q6). `reason` strings
/// come from `validate_squire_response`'s spec-table wording (§8.3) plus two
/// adapter-level cases (malformed JSON, ask_user-loop gap) that aren't part
/// of that table. Falls back to "other" for anything unrecognized so this
/// stays forward-compatible if the reason wording changes.
pub(crate) fn classify_rejection_rule(reason: &str) -> String {
    if reason.contains("not valid Squire protocol JSON") {
        "malformed_json".to_string()
    } else if reason.contains("ask_user and content cannot coexist") {
        "ask_user_content_conflict".to_string()
    } else if reason.contains("empty close response") {
        "empty_close_response".to_string()
    } else if reason.starts_with("undisplayable token") {
        "undisplayable_token".to_string()
    } else if reason.starts_with("undisplayable span reference") {
        "undisplayable_span_reference".to_string()
    } else if reason.starts_with("malformed sigil") {
        "malformed_sigil".to_string()
    } else if reason.starts_with("unclosed") {
        "unclosed_span".to_string()
    } else if reason.contains("preserved token does not exist") {
        "preserve_token_unknown".to_string()
    } else if reason.contains("relationship references unknown token") {
        "relationship_unknown_token".to_string()
    } else if reason.contains("non-invocable token") {
        "non_invocable_token".to_string()
    } else if reason.contains("unrecognized section") {
        "unrecognized_section".to_string()
    } else {
        "other".to_string()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// ContextManagerAdapter impl
// ═══════════════════════════════════════════════════════════════════════

#[async_trait]
impl ContextManagerAdapter for SquireContextAdapter {
    async fn build_turn_input(
        &mut self,
        session: &SessionWithMessages,
        _base_tools: &[ToolDefinition],
    ) -> Result<TurnInput, String> {
        let user_text = session
            .messages
            .iter()
            .rev()
            .find(|m| matches!(m.role, MessageRole::User))
            .map(|m| m.content.clone())
            .unwrap_or_default();

        let current_turn = self.store.current_turn(session.session.id).await;

        // Spec §9.1 step 2 / §4.3 / §3.1: auto-chunk the user's input into
        // USR_T{turn}_{NNN} system_referential tokens before the bootstrap
        // vector search below, so this turn's own input is immediately
        // discoverable in the same turn it arrived (see decisions.md).
        // Chunks now carry §^chunk_{i}§^ bookmark markers so the AI can
        // create referential tokens via new_tokens with a `ranges` entry.
        let _chunk_ids = ingest_user_input_chunks(&user_text, current_turn, self.store.as_ref(), session.session.id).await;

        // Reconstruct the user request text with §^bookmark§^ bare bookmarks
        // at each chunk boundary, so the AI can see which spans it can
        // reference via new_tokens with a `ranges` entry.  The matching
        // USR_T tokens in expanded_tokens carry the same bookmark markers
        // in their full_desc, so the bookmark names are correlated.
        //
        // NOTE: No §! references here — those are for tokens the AI itself
        // created.  USR_T tokens are system-manufactured; the AI should
        // create its own referential tokens using ranges pointing to the
        // bookmarks it sees in this text.
        let inline_sigil_text: String = {
            let chunks = crate::agent::squire::ingestion::chunk_user_input(&user_text);
            if chunks.is_empty() {
                user_text.clone()
            } else {
                chunks
                    .into_iter()
                    .enumerate()
                    .map(|(i, chunk)| format!("§^chunk_{}§^{}", i, chunk))
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        };

        // Save the user request text for Phase 2 token generation.
        // The orchestrator uses this to feed the original request together
        // with the Phase 1 response into the Phase 2 LLM call.
        self.user_request_text = inline_sigil_text.clone();

        let preserved = self.store.preserved_tokens(session.session.id).await;

        // User-request semantic bootstrap prefetch (global configurable):
        // search each resource class independently so high-density categories
        // like memory do not crowd out workflow/tool/skill candidates.
        let memory_prefetched = self
            .store
            .explore_memory("memory", &user_text, 1, self.prefetch.memory_top_k, current_turn, session.session.id)
            .await;
        let workflow_prefetched = self
            .store
            .explore_memory("workflow", &user_text, 1, self.prefetch.workflow_top_k, current_turn, session.session.id)
            .await;
        let tool_prefetched = self
            .store
            .explore_memory("tool", &user_text, 1, self.prefetch.tool_top_k, current_turn, session.session.id)
            .await;
        let skill_prefetched = self
            .store
            .explore_memory("skill", &user_text, 1, self.prefetch.skill_top_k, current_turn, session.session.id)
            .await;

        // Merge all sources (preserved + semantic prefetch) into one
        // deduplicated token list. Preserved tokens take priority, and
        // duplicates across prefetch sources or within a single source
        // (e.g. the same workflow returned twice) are removed.
        // Prefetched tokens below the min_score threshold are discarded —
        // irrelevant matches waste context and confuse the AI.
        let min_score = self.prefetch.min_score;
        let mut seen: HashSet<String> =
            preserved.iter().map(|t| t.token_id.clone()).collect();
        let preserved_ids: HashSet<String> = seen.clone();
        let mut all_tokens: Vec<_> = preserved.into_iter().collect();
        for token in memory_prefetched
            .into_iter()
            .chain(workflow_prefetched.into_iter())
            .chain(tool_prefetched.into_iter())
            .chain(skill_prefetched.into_iter())
        {
            if token.score < min_score {
                continue;
            }
            if seen.insert(token.token_id.clone()) {
                all_tokens.push(token);
            }
        }

        // Classification policy for expanded (full_desc) vs. short form:
        //   - Preserved tokens — expanded, because the AI chose to keep them
        //   - User/referential chunks (USR_T*, RESP_T*) — expanded, because
        //     a referential token is meaningless without its content
        //   - Prefetched bootstrap tokens — short form only; the AI uses
        //     token_to_detail when it needs the full description
        let mut expanded_tokens: Vec<serde_json::Value> = Vec::new();
        let mut short_tokens: Vec<serde_json::Value> = Vec::new();
        for token in &all_tokens {
            let is_preserved = preserved_ids.contains(&token.token_id);
            let is_referential = token.token_id.starts_with("USR_T")
                || token.token_id.starts_with("RESP_T");

            if is_preserved || is_referential {
                let detail = self.store.token_detail(&token.token_id).await;
                expanded_tokens.push(serde_json::json!({
                    "token_id": token.token_id,
                    "full_desc": detail.and_then(|d| d.full_desc).unwrap_or_default(),
                }));
            } else {
                short_tokens.push(serde_json::json!({
                    "token_id": token.token_id,
                    "short_desc": token.short_desc,
                }));
            }
        }

        let active_process_state =
            self.store.compute_active_process_state(session.session.id).await;

        let context = serde_json::json!({
            "expanded_tokens": expanded_tokens,
            "tokens": short_tokens,
            "active_process_state": active_process_state,
        });

        let system_content = format!(
            "{}\n\n--- Context for this turn ---\n{}",
            crate::agent::squire_prompts::system_prompt(),
            serde_json::to_string(&context).map_err(|e| e.to_string())?
        );

        let user_message = serde_json::json!({
            "user_request": inline_sigil_text,
        });

        let messages = vec![
            ChatMessage {
                role: ChatRole::System,
                content: system_content,
                tool_call_id: None,
                tool_calls: None,
                reasoning_content: None,
            },
            ChatMessage {
                role: ChatRole::User,
                content: serde_json::to_string(&user_message).map_err(|e| e.to_string())?,
                tool_call_id: None,
                tool_calls: None,
                reasoning_content: None,
            },
        ];

        // Q5: Squire mode exposes only the two built-in Squire protocol
        // tools (explore, token_to_detail) as direct ToolDefinitions.
        // External tools — MCP, built-in agent tools — are NOT injected
        // into ChatRequest.tools. The AI discovers them through
        // explore(resource_type="tool_skill") and calls them via the
        // invoke(token_id, params) tool.
        Ok(TurnInput {
            messages,
            tools: built_in_tool_definitions(),
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
        _thinking: Option<String>,
        messages: &mut Vec<ChatMessage>,
        store: &dyn ConversationStore,
    ) -> Result<TurnOutcome, String> {
        let parsed = match detect_and_parse(&assistant_content) {
            Ok(r) => r,
            Err(e) => {
                return self
                    .reject_and_record(session_id, messages, assistant_content, e, store)
                    .await;
            }
        };

        // ── ask_user handling (common to both phases) ──
        if !parsed.ask_user.is_empty() {
            if !parsed.content.is_empty() {
                return self
                    .reject_and_record(
                        session_id,
                        messages,
                        assistant_content,
                        "ask_user and content cannot coexist".to_string(),
                        store,
                    )
                    .await;
            }
            return Ok(TurnOutcome::AskUser {
                question: parsed.ask_user.clone(),
            });
        }

        match self.phase {
            SquirePhase::Phase1 => {
                // If the response already has §# sections (new_tokens,
                // relationships, preserve), it's a combined single-pass
                // response — process content via Phase 1 logic, then
                // immediately feed into Phase 2 token logic in the same
                // call. This keeps backwards compatibility with test
                // harnesses and bootstrapping scenarios.
                let has_phase2_sections = !parsed.new_tokens.is_empty()
                    || !parsed.relationships.is_empty()
                    || !parsed.preserve.is_empty();
                if has_phase2_sections {
                    self.finalize_combined(session_id, parsed, assistant_content, _thinking, messages, store).await
                } else {
                    self.finalize_phase1(session_id, parsed, assistant_content, _thinking, messages, store).await
                }
            }
            SquirePhase::Phase2 => self.finalize_phase2(session_id, parsed, assistant_content, _thinking, messages, store).await,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Phase 1 — explore + generate response (lightweight)
// ═══════════════════════════════════════════════════════════════════════

impl SquireContextAdapter {
    async fn finalize_phase1(
        &mut self,
        session_id: SessionId,
        parsed: super::types::SquireResponse,
        assistant_content: String,
        _thinking: Option<String>,
        messages: &mut Vec<ChatMessage>,
        store: &dyn ConversationStore,
    ) -> Result<TurnOutcome, String> {
        // Lightweight validation: Phase 1 only generates content with sigils,
        // not §# sections (those are for Phase 2).
        if parsed.content.is_empty() {
            return self
                .reject_and_record(
                    session_id,
                    messages,
                    assistant_content,
                    "empty close response".to_string(),
                    store,
                )
                .await;
        }
        if let Err(e) = check_malformed_sigils(&parsed.content) {
            return self
                .reject_and_record(
                    session_id,
                    messages,
                    assistant_content,
                    e.reason,
                    store,
                )
                .await;
        }
        let (_, unclosed) = extract_spans(&parsed.content);
        if let Some(token_id) = unclosed {
            return self
                .reject_and_record(
                    session_id,
                    messages,
                    assistant_content,
                    format!("unclosed §^ span {}", token_id),
                    store,
                )
                .await;
        }
        // §! inline refs must resolve against the store (Phase 1 has no
        // new_tokens to define them inline).
        for token_id in extract_inline_refs(&parsed.content) {
            if !self.store.token_exists(&token_id).await {
                return self
                    .reject_and_record(
                        session_id,
                        messages,
                        assistant_content,
                        format!("undisplayable token §!{}", token_id),
                        store,
                    )
                    .await;
            }
        }

        self.retry_count = 0;
        let turn = self.store.current_turn(session_id).await;

        // Hit-count fidelity: every unique §!-referenced token that already
        // exists in the store gets exactly one hit credited (deduplicated).
        let mut deduped = HashSet::new();
        for token_id in extract_inline_refs(&parsed.content) {
            deduped.insert(token_id);
        }
        for token_id in &deduped {
            if self.store.token_exists(token_id).await {
                self.store.record_hit(token_id).await;
            }
        }

        // Raw partition: persist the text outside every closed §^ span.
        let residual = unmarked_residual(&parsed.content);
        if !residual.is_empty() {
            self.store.record_raw_output(session_id, turn, residual).await;
        }

        self.store.increment_turn(session_id).await;

        // Chunk the Phase 1 response into RESP_T tokens for bookmark
        // resolution in Phase 2.
        ingest_response_chunks(&parsed.content, turn, self.store.as_ref(), session_id).await;

        let display_content = self.expand_for_display(&parsed.content).await;
        if !display_content.is_empty() {
            store
                .append_message(NewMessage {
                    session_id,
                    role: MessageRole::Assistant,
                    content: display_content,
                    thinking_content: _thinking,
                })
                .await
                .map_err(|e| e.to_string())?;
        }

        Ok(TurnOutcome::Phase2 {
            phase1_content: assistant_content,
            user_request: self.user_request_text.clone(),
        })
    }

    /// Combined single-pass: process Phase 1 content logic (hit counts,
    /// raw partition, ingest chunks, display), then Phase 2 token/rel
    /// logic in the same call. Used when a single response has both
    /// content text AND §# token sections (backwards-compatible mode).
    async fn finalize_combined(
        &mut self,
        session_id: SessionId,
        mut parsed: super::types::SquireResponse,
        assistant_content: String,
        _thinking: Option<String>,
        messages: &mut Vec<ChatMessage>,
        store: &dyn ConversationStore,
    ) -> Result<TurnOutcome, String> {
        // ── Phase 1 content pass ──
        if parsed.content.is_empty() {
            return self
                .reject_and_record(
                    session_id,
                    messages,
                    assistant_content,
                    "empty close response".to_string(),
                    store,
                )
                .await;
        }
        if let Err(e) = check_malformed_sigils(&parsed.content) {
            return self
                .reject_and_record(session_id, messages, assistant_content, e.reason, store)
                .await;
        }
        let (_, unclosed) = extract_spans(&parsed.content);
        if let Some(token_id) = unclosed {
            return self
                .reject_and_record(
                    session_id,
                    messages,
                    assistant_content,
                    format!("unclosed §^ span {}", token_id),
                    store,
                )
                .await;
        }

        self.retry_count = 0;
        let turn = self.store.current_turn(session_id).await;
        let (spans, _) = extract_spans(&parsed.content);

        for token_id in extract_inline_refs(&parsed.content) {
            if self.store.token_exists(&token_id).await {
                self.store.record_hit(&token_id).await;
            }
        }

        let residual = unmarked_residual(&parsed.content);
        if !residual.is_empty() {
            self.store.record_raw_output(session_id, turn, residual).await;
        }

        // ── Phase 2 token/rel pass ──
        let _defining_now: HashSet<String> =
            parsed.new_tokens.iter().map(|t| t.id.clone()).collect();

        // Resolve range specs with soft-fail.
        for token in parsed.new_tokens.iter_mut() {
            if let Some(ref desc) = token.full_desc {
                if is_range_spec(desc) {
                    if let Some(spec) = parse_range_spec(desc) {
                        if let Ok(ranges) = resolve_range_spec(&spec, self.store.as_ref()).await {
                            token.ranges = ranges;
                            token.full_desc = None;
                        } else {
                            token.full_desc = None; // unresolvable, store without ranges
                        }
                    }
                }
            }
            let mut to_upsert = token.clone();
            if spans.iter().any(|(id, _)| id == &to_upsert.id) {
                to_upsert.token_type = "referential".to_string();
            }
            if to_upsert.full_desc.is_none() {
                if let Some((_, span_text)) = spans.iter().find(|(id, _)| id == &to_upsert.id) {
                    to_upsert.full_desc = Some(span_text.clone());
                }
            }
            self.store.upsert_token(to_upsert, turn, session_id).await;
        }

        for rel in &parsed.relationships {
            self.store.add_relationship(rel.clone()).await;
        }

        let mut always_preserve: Vec<String> = parsed.preserve.clone();
        for id in self.store.list_token_ids_by_session(session_id).await {
            if always_preserve.contains(&id) {
                continue;
            }
            if id.starts_with("USR_") || id.starts_with("RESP_") {
                always_preserve.push(id);
            }
        }
        self.store
            .set_preserve_list(session_id, always_preserve)
            .await;
        self.store.increment_turn(session_id).await;

        ingest_response_chunks(&parsed.content, turn, self.store.as_ref(), session_id).await;

        let display_content = self.expand_for_display(&parsed.content).await;
        if !display_content.is_empty() {
            store
                .append_message(NewMessage {
                    session_id,
                    role: MessageRole::Assistant,
                    content: display_content,
                    thinking_content: _thinking,
                })
                .await
                .map_err(|e| e.to_string())?;
        }

        Ok(TurnOutcome::Done)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Phase 2 — token/relationship generation (full processing)
// ═══════════════════════════════════════════════════════════════════════

impl SquireContextAdapter {
    async fn finalize_phase2(
        &mut self,
        session_id: SessionId,
        mut parsed: super::types::SquireResponse,
        assistant_content: String,
        _thinking: Option<String>,
        messages: &mut Vec<ChatMessage>,
        store: &dyn ConversationStore,
    ) -> Result<TurnOutcome, String> {
        // ═══════════════════════════════════════════════════════════════
        // Hard validation gates — fundamental protocol violations that
        // always reject the entire response.
        // ═══════════════════════════════════════════════════════════════

        // Phase 2 must not produce content text (only §# sections).
        if !parsed.content.is_empty() {
            return self
                .reject_and_record(
                    session_id,
                    messages,
                    assistant_content,
                    "Phase 2 must not produce content text".to_string(),
                    store,
                )
                .await;
        }
        if let Err(e) = check_malformed_sigils(&parsed.content) {
            return self
                .reject_and_record(session_id, messages, assistant_content, e.reason, store)
                .await;
        }
        let (_, unclosed) = extract_spans(&parsed.content);
        if let Some(tid) = unclosed {
            return self
                .reject_and_record(
                    session_id,
                    messages,
                    assistant_content,
                    format!("unclosed §^ span {}", tid),
                    store,
                )
                .await;
        }

        self.retry_count = 0;
        let turn = self.store.current_turn(session_id).await;
        let (spans, _) = extract_spans(&parsed.content);

        // ═══════════════════════════════════════════════════════════════
        // Build known-token set — tokens being defined in THIS response
        // plus all tokens already in the store (Phase 1 RESP_T chunks,
        // context tokens, memories, etc.).
        // ═══════════════════════════════════════════════════════════════
        let mut defining_now: HashSet<String> = HashSet::new();
        for token in &parsed.new_tokens {
            defining_now.insert(token.id.clone());
        }

        // ═══════════════════════════════════════════════════════════════
        // Upsert new tokens — always valid. These are self-contained
        // definitions that don't depend on other tokens' existence.
        // Range-spec resolution is soft-fail per token: if the source
        // chunk or bookmark doesn't exist yet, the token is still stored
        // (without resolved ranges) and the issue is noted for retry.
        // ═══════════════════════════════════════════════════════════════
        let mut range_issues: Vec<String> = Vec::new();
        for token in parsed.new_tokens.iter_mut() {
            // Soft-fail range resolution (if full_desc is a range spec)
            if let Some(ref desc) = token.full_desc {
                if is_range_spec(desc) {
                    match parse_range_spec(desc) {
                        Some(spec) => match resolve_range_spec(&spec, self.store.as_ref()).await {
                            Ok(ranges) => {
                                token.ranges = ranges;
                                token.full_desc = None;
                            }
                            Err(e) => {
                                range_issues.push(format!("{}: {}", token.id, e));
                                // Token still stored, just without resolved ranges.
                                token.full_desc = None; // clear the range spec from full_desc
                            }
                        },
                        None => {
                            range_issues.push(format!("{}: invalid range spec syntax", token.id));
                            // Keep full_desc as-is; the model can fix syntax next retry.
                        }
                    }
                }
            }

            // Default-typing and span-content fill
            let mut to_upsert = token.clone();
            if spans.iter().any(|(id, _)| id == &to_upsert.id) {
                to_upsert.token_type = "referential".to_string();
            }
            if to_upsert.full_desc.is_none() {
                if let Some((_, span_text)) = spans.iter().find(|(id, _)| id == &to_upsert.id) {
                    to_upsert.full_desc = Some(span_text.clone());
                }
            }
            self.store.upsert_token(to_upsert, turn, session_id).await;
        }

        // ═══════════════════════════════════════════════════════════════
        // Classify relationships — valid ones are saved immediately.
        // Invalid ones (unknown subject/object) are collected for retry.
        // ═══════════════════════════════════════════════════════════════
        let mut valid_rels: Vec<super::types::Relationship> = Vec::new();
        let mut invalid_rels: Vec<String> = Vec::new();
        for rel in &parsed.relationships {
            let subj_known = defining_now.contains(&rel.subject)
                || self.store.token_exists(&rel.subject).await;
            let obj_known = defining_now.contains(&rel.object)
                || self.store.token_exists(&rel.object).await;
            if subj_known && obj_known {
                valid_rels.push(rel.clone());
            } else {
                let mut details = Vec::new();
                if !subj_known {
                    details.push(format!("subject '{}' unknown", rel.subject));
                }
                if !obj_known {
                    details.push(format!("object '{}' unknown", rel.object));
                }
                invalid_rels.push(format!(
                    "{}|{}|{} ({})",
                    rel.subject,
                    rel.predicate,
                    rel.object,
                    details.join(", ")
                ));
            }
        }
        for rel in &valid_rels {
            self.store.add_relationship(rel.clone()).await;
        }

        // ═══════════════════════════════════════════════════════════════
        // Classify preserve entries — save valid ones, skip invalid.
        // ═══════════════════════════════════════════════════════════════
        let mut valid_preserve: Vec<String> = Vec::new();
        let mut invalid_preserve: Vec<String> = Vec::new();
        for id in &parsed.preserve {
            let exists = defining_now.contains(id) || self.store.token_exists(id).await;
            if exists {
                valid_preserve.push(id.clone());
            } else {
                invalid_preserve.push(id.clone());
            }
        }

        // Merge preserved + mandatory USR_/RESP_ tokens.
        let mut always_preserve: Vec<String> = valid_preserve.clone();
        for id in self.store.list_token_ids_by_session(session_id).await {
            if always_preserve.contains(&id) {
                continue;
            }
            if id.starts_with("USR_") || id.starts_with("RESP_") {
                always_preserve.push(id);
            }
        }
        self.store
            .set_preserve_list(session_id, always_preserve)
            .await;
        self.store.increment_turn(session_id).await;

        // ═══════════════════════════════════════════════════════════════
        // If any issues remain, return Retry with a focused message.
        // Valid tokens/relationships have already been saved — the model
        // only needs to fix what's left.
        // ═══════════════════════════════════════════════════════════════
        let mut issues: Vec<String> = Vec::new();
        if !range_issues.is_empty() {
            issues.push(format!(
                "Range resolution failed for these tokens (stored without ranges, \
                 fix or remove the range specs): {}",
                range_issues.join("; ")
            ));
        }
        if !invalid_rels.is_empty() {
            issues.push(format!(
                "Invalid relationships (already saved valid ones; fix or remove these): {}",
                invalid_rels.join("; ")
            ));
        }
        if !invalid_preserve.is_empty() {
            issues.push(format!(
                "Unknown preserved tokens (already saved valid ones; fix or remove these): {}",
                invalid_preserve.join(", ")
            ));
        }

        if !issues.is_empty() {
            let retry_msg = format!(
                "Partial acceptance: valid tokens and relationships were saved. \
                 Fix these remaining issues:\n{}",
                issues.join("\n")
            );
            messages.push(ChatMessage {
                role: ChatRole::User,
                content: retry_msg,
                tool_call_id: None,
                tool_calls: None,
                reasoning_content: None,
            });
            return Ok(TurnOutcome::Retry);
        }

        // ═══════════════════════════════════════════════════════════════
        // Everything valid — finalize normally.
        // ═══════════════════════════════════════════════════════════════
        ingest_response_chunks(&parsed.content, turn, self.store.as_ref(), session_id).await;

        let display_content = self.expand_for_display(&parsed.content).await;
        if !display_content.is_empty() {
            store
                .append_message(NewMessage {
                    session_id,
                    role: MessageRole::Assistant,
                    content: display_content,
                    thinking_content: _thinking,
                })
                .await
                .map_err(|e| e.to_string())?;
        }

        Ok(TurnOutcome::Done)
    }
}
