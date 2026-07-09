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
    extract_inline_refs, extract_spans, resolve_ranges,
    strip_span_markers, take_token_id, unmarked_residual, validate_squire_response,
};
use super::SquireStore;
use super::tools::built_in_tool_definitions;
use super::types::{ComplianceFailureRecord, SquireResponse};
use crate::agent::context_adapter::{ContextManagerAdapter, TurnInput, TurnOutcome};
use crate::agent::ToolResult;
use crate::commands::utils::clean_deepseek_json;
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
// Adapter struct
// ═══════════════════════════════════════════════════════════════════════

pub struct SquireContextAdapter {
    pub(crate) store: Arc<dyn SquireStore>,
    prefetch: SquirePrefetchConfig,
    max_retries: u32,
    retry_count: u32,
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
        }
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
    } else if reason.starts_with("unclosed") {
        "unclosed_span".to_string()
    } else if reason.contains("non-invocable token") {
        "non_invocable_token".to_string()
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
        let cleaned = clean_deepseek_json(assistant_content.trim());
        let parsed: SquireResponse = match serde_json::from_str(&cleaned) {
            Ok(r) => r,
            Err(e) => {
                return self
                    .reject_and_record(
                        session_id,
                        messages,
                        assistant_content,
                        format!("response is not valid Squire protocol JSON: {}", e),
                        store,
                    )
                    .await;
            }
        };

        if !parsed.ask_user.is_empty() {
            // Spec §9.3's response-field AskUser loop: a populated `ask_user`
            // with no `content` is a valid, expected turn state, not a
            // protocol violation — surface it to orchestration as
            // `TurnOutcome::AskUser` so it can pause the turn, round-trip the
            // question to the user via IPC, and resume with the answer
            // appended to `messages` (see `ask-user-loop/decisions.md`).
            // `content` is guaranteed empty here since `ask_user`+`content`
            // mutual exclusion would otherwise apply — but this branch runs
            // before `validate_squire_response`, so a model that populates
            // both isn't rejected via this path; it still needs to fail per
            // spec §8.3. Check for that malformed combination explicitly.
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

        let known: HashSet<String> = {
            let mut set = HashSet::new();
            for token_id in extract_inline_refs(&parsed.content) {
                if self.store.token_exists(&token_id).await {
                    set.insert(token_id);
                }
            }
            set
        };

        if let Err(failure) = validate_squire_response(&parsed, |id| known.contains(id)) {
            return self
                .reject_and_record(session_id, messages, assistant_content, failure.reason, store)
                .await;
        }

        self.retry_count = 0;
        let turn = self.store.current_turn(session_id).await;
        let (spans, _) = extract_spans(&parsed.content);

        // Hit-count fidelity (spec §3.3, events "Token appears in explore()
        // results that AI acts on" [second disjunct: "...or references in
        // output", per §6.1's gloss] and "§! reference found in a chunk
        // loaded into context"): every token in `known` already existed in
        // the store *before* this turn's new_tokens upsert loop below runs
        // (that's exactly what `token_exists`-filtering computed `known`
        // means) and is `§!`-referenced in this compliant response's
        // content, which is unambiguously "loaded into context" via
        // `expand_for_display` immediately below. A token that is instead
        // newly defined *and* cited in this same turn is deliberately
        // excluded here — it already receives its one hit from
        // `upsert_token`'s "regardless" +1 (event 4) below, so crediting it
        // again here would double-count a single citation. See
        // decisions.md for the full operationalization and the deliberately
        // deferred nested chunk-citing-chunk case.
        for token_id in &known {
            self.store.record_hit(token_id).await;
        }

        // Raw partition (spec §4.1/§4.3/§9.4 step 4): persist the unmarked
        // residual of this compliant response — the text outside every
        // closed §^ span, i.e. content the AI produced but did not promote
        // into a structured memory token. Only on the compliant path (a
        // rejected response never reaches this point; reject_and_record
        // already gives it a complete structured audit trail via
        // record_compliance_failure) and only when there's something left
        // to store (a fully §^-spanned response has nothing outside its
        // spans — see raw-partition-storage/decisions.md).
        let residual = unmarked_residual(&parsed.content);
        if !residual.is_empty() {
            self.store.record_raw_output(session_id, turn, residual).await;
        }

        for token in &parsed.new_tokens {
            let mut token = token.clone();
            // Default-typing (spec v3): tokens from §^ spans are
            // "referential"; all others without an explicit type remain
            // "concept" (handled by serde default on NewTokenSpec).
            if spans.iter().any(|(id, _)| id == &token.id) {
                token.token_type = "referential".to_string();
            }
            if token.full_desc.is_none() {
                if let Some((_, span_text)) = spans.iter().find(|(id, _)| id == &token.id) {
                    token.full_desc = Some(span_text.clone());
                }
            }
            self.store.upsert_token(token, turn, session_id).await;
        }
        for rel in &parsed.relationships {
            self.store.add_relationship(rel.clone()).await;
        }
        self.store
            .set_preserve_list(session_id, parsed.preserve.clone())
            .await;
        self.store.increment_turn(session_id).await;

        // Chunk the model's response into RESP_T{turn}_{NNN} tokens for
        // future bookmark/referential-token resolution.
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
