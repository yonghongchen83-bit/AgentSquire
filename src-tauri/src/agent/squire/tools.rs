//! The five built-in Squire protocol tools: `explore`, `token_to_detail`,
//! `rdf`, `batch`, and `invoke`.
//!
//! `explore`, `rdf`, `token_to_detail`, and `batch` are the discovery
//! surface. `batch` composes explore/rdf/token_to_detail into pipelines
//! (`|`) and parallel groups (`&`/`;`) — spec §3 batch composition
//! syntax — reducing round trips while counting as one call.
//! `invoke` proxies a call to a tool/skill discovered via `explore()`,
//! identified by its `token_id`.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use async_trait::async_trait;
use serde_json::Value;

use super::SquireStore;
use super::types::TokenSummary;
use crate::agent::{Tool, ToolResult};
use crate::llm::provider::ToolDefinition;
use crate::storage::conversation_store::SessionId;

/// Default per-turn cap on discovery-tool calls (explore + rdf + token_to_detail).
/// Spec §3: "small, e.g. 2–3".
pub const DEFAULT_BATCH_CAP: u32 = 3;

/// Shared batch counter: check-and-increment. Returns `Some(ToolResult)` if
/// the cap has been exceeded (the caller should return this error immediately);
/// returns `None` if the call is allowed to proceed.
fn check_batch_cap(counter: &AtomicU32, cap: u32, _tool_name: &str) -> Option<ToolResult> {
    let count = counter.fetch_add(1, Ordering::Relaxed);
    if count >= cap {
        // Rollback so repeated over-cap calls all see the exceeded state
        // without saturating the counter.
        counter.store(cap, Ordering::Relaxed);
        return Some(ToolResult {
            call_id: String::new(),
            output: format!(
                "Batch retrieval cap ({}) exceeded. You have already made {} discovery calls \
                 (explore/rdf/token_to_detail) this turn. Respond with the information you have, \
                 or use invoke() on tools you have already discovered.",
                cap, count
            ),
            is_error: true,
        });
    }
    None
}

// ─────────────────────────── Tool definitions ───────────────────────────

pub fn built_in_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "explore".to_string(),
            description: "Search Squire memory and registered resources by semantic similarity, optionally expanding via graph traversal. Use vector='tag' to search against author-curated tags (best for structured content like workflows/skills/tools), vector='content' to search against full prose (default, best for freeform text).".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "resource_type": {
                        "type": "string",
                        "enum": ["workflow", "tool", "skill", "tool_skill", "memory", "concept", "referential", "all"]
                    },
                    "query": {"type": "string"},
                    "num_hops": {"type": "integer", "minimum": 0},
                    "max_results": {"type": "integer", "minimum": 1},
                    "vector": {
                        "type": "string",
                        "enum": ["content", "tag"],
                        "description": "Which embedding to search: 'content' (default) for prose body, 'tag' for author-curated keyword vector. Tag search is best for structured, self-describing content like workflows, skills, and tools."
                    }
                },
                "required": ["resource_type", "query"]
            }),
        },
        ToolDefinition {
            name: "token_to_detail".to_string(),
            description: "Retrieve the short or full description of a specific Squire token.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "token_id": {"type": "string"},
                    "detail_level": {"type": "string", "enum": ["short", "full"]}
                },
                "required": ["token_id", "detail_level"]
            }),
        },
        ToolDefinition {
            name: "rdf".to_string(),
            description: "Walk relationship (triplet) edges outward from a token, returning tokens discovered by graph traversal. Use after explore() to expand context around a seed token. Does NOT reason about which edges matter — that judgment belongs to the AI.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "token_id": {
                        "type": "string",
                        "description": "The token to start traversal from"
                    },
                    "hops": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 5,
                        "description": "Number of graph hops to walk outward"
                    },
                    "max_results": {
                        "type": "integer",
                        "minimum": 1,
                        "description": "Max tokens to return (default 20)"
                    }
                },
                "required": ["token_id", "hops"]
            }),
        },
        ToolDefinition {
            name: "invoke".to_string(),
            description: "Execute a tool or skill by its token_id (discovered via explore(resource_type='tool_skill', ...)). The result is returned as if the tool was called directly.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "token_id": {
                        "type": "string",
                        "description": "The token_id of the tool or skill to invoke, as returned by explore()"
                    },
                    "params": {
                        "type": "object",
                        "description": "Parameters to pass to the tool, matching its input schema"
                    }
                },
                "required": ["token_id", "params"]
            }),
        },
        ToolDefinition {
            name: "batch".to_string(),
            description: "Compose explore/rdf/token_to_detail calls with pipe (|) and parallel (& or ;) operators. Piping explore results into rdf saves round trips and counts as one batch call. Example: explore(memory, 'rust', 1, 10) | rdf(2)".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "expression": {
                        "type": "string",
                        "description": "Batch expression using | (pipe), & or ; or newline (parallel). E.g. 'explore(memory, rust, 1, 10) | rdf(2)' or 'explore(memory, X, 1, 5) & explore(workflow, Y, 0, 3)'"
                    }
                },
                "required": ["expression"]
            }),
        },
    ]
}

// ─────────────────────────── Explore tool ───────────────────────────

pub struct SquireExploreTool {
    pub store: Arc<dyn SquireStore>,
    /// Snapshot of tool definitions taken at construction time. Used instead of
    /// holding a reference to the dispatch registry to avoid Arc cycles.
    pub tool_defs: Vec<ToolDefinition>,
    /// Needed to look up the requesting session's turn count for
    /// `effective_priority` ranking (spec §3.3) — not exposed to the model
    /// as a tool argument (see decisions.md: the model has no legitimate
    /// reason to know its own session id).
    pub session_id: SessionId,
    /// Shared per-turn batch counter (explore + rdf + token_to_detail).
    pub batch_counter: Arc<AtomicU32>,
    /// Maximum discovery calls allowed this turn.
    pub batch_cap: u32,
}

#[async_trait]
impl Tool for SquireExploreTool {
    fn name(&self) -> &str {
        "explore"
    }
    fn description(&self) -> &str {
        "Search Squire memory and registered resources by semantic similarity."
    }
    fn input_schema(&self) -> Value {
        built_in_tool_definitions()[0].input_schema.clone()
    }
    async fn execute(&self, call_id: &str, args: Value) -> ToolResult {
        if let Some(err) = check_batch_cap(&self.batch_counter, self.batch_cap, "explore") {
            return err;
        }
        let resource_type = args
            .get("resource_type")
            .and_then(|v| v.as_str())
            .unwrap_or("all")
            .to_string();
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let num_hops = args.get("num_hops").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let max_results = args
            .get("max_results")
            .and_then(|v| v.as_u64())
            .unwrap_or(10) as u32;
        let vector = args
            .get("vector")
            .and_then(|v| v.as_str())
            .unwrap_or("content");
        let current_turn = self.store.current_turn(self.session_id).await;

        // ── Token-ID resolution fallback ──────────────────────────────────
        // If the AI passes a raw token_id as the query (e.g. it sees §!Foo
        // and calls explore("Foo")), resolve the token first and use its
        // content for semantic search.  A random token_id string has no
        // semantic relation to the actual content, so embedding search
        // against the raw key would always miss.
        //
        // CRITICAL: Only reach for token_detail() when the query actually
        // LOOKS like a token_id (contains _T\d).  This avoids an expensive
        // LanceDB query on every natural-language explore() — the common
        // case by far.
        let looks_like_token_id = query.contains("_T") && query.bytes().any(|b| b.is_ascii_digit());
        let effective_query = if looks_like_token_id && resource_type != "tool" {
            self.store
                .token_detail(&query)
                .await
                .map(|d| d.full_desc.unwrap_or(d.short_desc))
                .unwrap_or(query.clone())
        } else {
            query.clone()
        };

        // "tool"/"tool_skill" are served from the real (full) tool registry —
        // this is the Squire-as-gateway discovery surface, not memory search.
        let results = if matches!(resource_type.as_str(), "tool" | "tool_skill") {
            let ql = query.to_lowercase();
            // obs-3: capture near-misses (tools that did NOT match the naive
            // substring filter) for the retrieval trace. Pure observation —
            // does not change which tools are returned. Only computed when
            // tracing is on so the release path stays allocation-free.
            let tracing = squire_store::trace::trace_enabled();
            let mut tool_near_misses: Vec<serde_json::Value> = Vec::new();
            let mut tool_results: Vec<TokenSummary> = Vec::new();
            for d in self.tool_defs.iter() {
                let matched = ql.is_empty()
                    || d.name.to_lowercase().contains(&ql)
                    || d.description.to_lowercase().contains(&ql);
                if matched && (tool_results.len() as u32) < max_results {
                    tool_results.push(TokenSummary {
                        token_id: d.name.clone(),
                        token_type: "tool".to_string(),
                        score: 1.0,
                        short_desc: d.description.clone(),
                        accumulated_hits: 0,
                        hop_distance: 0,
                        via_token_id: None, tags: vec![], properties: std::collections::HashMap::new(),
                    });
                } else if tracing {
                    // Either it didn't match the substring filter, or it
                    // matched but was truncated past max_results — both are
                    // near-misses from the caller's perspective.
                    tool_near_misses.push(serde_json::json!({
                        "token_id": d.name,
                        "token_type": "tool",
                        "score": if matched { 1.0 } else { 0.0 },
                        "included": false,
                    }));
                }
            }
            if resource_type == "tool_skill" {
                let skills = self
                    .store
                    .explore_memory("skill", &effective_query, num_hops, max_results, current_turn, self.session_id, vector)
                    .await;
                tool_results.extend(skills);
            }
            if tracing {
                let results_json: Vec<serde_json::Value> = tool_results
                    .iter()
                    .map(|t| {
                        serde_json::json!({
                            "token_id": t.token_id,
                            "token_type": t.token_type,
                            "score": t.score,
                            "included": true,
                        })
                    })
                    .collect();
                tool_near_misses.truncate(20);
                let payload = serde_json::json!({
                    "branch": "tool_registry_substring",
                    "resource_type": resource_type,
                    "query": query,
                    "num_hops": num_hops,
                    "max_results": max_results,
                    // Tools are served from the live registry by a naive
                    // substring filter, NOT semantic embedding — flag this so
                    // trace consumers don't confuse it with the store branch.
                    "embedding_backend": "none-substring-match",
                    "scoring_note": "substring-not-semantic; score fixed at 1.0 for matches",
                    "results": results_json,
                    "near_misses": tool_near_misses,
                });
                squire_store::trace::trace_explore(
                    current_turn,
                    Some(call_id.to_string()),
                    payload,
                );
            }
            tool_results
        } else {
            self.store
                .explore_memory(&resource_type, &effective_query, num_hops, max_results, current_turn, self.session_id, vector)
                .await
        };

        ToolResult {
            call_id: call_id.to_string(),
            output: serde_json::to_string(&results).unwrap_or_else(|_| "[]".to_string()),
            is_error: false,
        }
    }
}

// ─────────────────────────── Token-to-detail tool ───────────────────────────

pub struct SquireTokenToDetailTool {
    pub store: Arc<dyn SquireStore>,
    /// Shared per-turn batch counter (explore + rdf + token_to_detail).
    pub batch_counter: Arc<AtomicU32>,
    /// Maximum discovery calls allowed this turn.
    pub batch_cap: u32,
}

#[async_trait]
impl Tool for SquireTokenToDetailTool {
    fn name(&self) -> &str {
        "token_to_detail"
    }
    fn description(&self) -> &str {
        "Retrieve the short or full description of a specific Squire token."
    }
    fn input_schema(&self) -> Value {
        built_in_tool_definitions()[1].input_schema.clone()
    }
    async fn execute(&self, call_id: &str, args: Value) -> ToolResult {
        if let Some(err) = check_batch_cap(&self.batch_counter, self.batch_cap, "token_to_detail") {
            return err;
        }
        let token_id = args.get("token_id").and_then(|v| v.as_str()).unwrap_or("");
        if token_id.is_empty() {
            return ToolResult {
                call_id: call_id.to_string(),
                output: "Missing required argument: token_id".to_string(),
                is_error: true,
            };
        }
        let detail_level = args
            .get("detail_level")
            .and_then(|v| v.as_str())
            .unwrap_or("short");

        match self.store.token_detail(token_id).await {
            Some(detail) => {
                // Spec §6.2: "accumulated_hits increments by 1 on each call."
                // Also covers §5.1/§3.3's "chunk loaded into context" event
                // for this token itself (the requested token's own detail
                // body is what's being loaded here). `hit-count-fidelity`
                // added the complementary finalize_turn wiring for §!
                // references appearing in the AI's own response content —
                // see this file's `finalize_turn` and decisions.md for the
                // one still-deferred residual (a §! reference nested inside
                // *this* full_desc body pointing at a third token is not
                // itself scanned/credited here).
                self.store.record_hit(token_id).await;
                let output = if detail_level == "full" {
                    detail.full_desc.unwrap_or(detail.short_desc)
                } else {
                    detail.short_desc
                };
                ToolResult {
                    call_id: call_id.to_string(),
                    output,
                    is_error: false,
                }
            }
            None => ToolResult {
                call_id: call_id.to_string(),
                output: format!("Unknown token: {}", token_id),
                is_error: true,
            },
        }
    }
}

// ─────────────────────────── Rdf (graph walk) tool ───────────────────────────

/// `rdf(token_id, hops)` — walk relationship edges outward from a seed token.
///
/// Spec §3: a mechanical, non-judgmental walk of triplet edges. Does not reason
/// about which edges matter — that judgment belongs to the AI. Uses the
/// backend-agnostic `traverse_relationships` helper shared by all store
/// implementations.
pub struct SquireRdfTool {
    pub store: Arc<dyn SquireStore>,
    /// Shared per-turn batch counter (explore + rdf + token_to_detail).
    pub batch_counter: Arc<AtomicU32>,
    /// Maximum discovery calls allowed this turn.
    pub batch_cap: u32,
}

#[async_trait]
impl Tool for SquireRdfTool {
    fn name(&self) -> &str {
        "rdf"
    }
    fn description(&self) -> &str {
        "Walk relationship edges outward from a token to discover related tokens via graph traversal."
    }
    fn input_schema(&self) -> Value {
        built_in_tool_definitions()[2].input_schema.clone()
    }
    async fn execute(&self, call_id: &str, args: Value) -> ToolResult {
        if let Some(err) = check_batch_cap(&self.batch_counter, self.batch_cap, "rdf") {
            return err;
        }
        let token_id = args.get("token_id").and_then(|v| v.as_str()).unwrap_or("");
        if token_id.is_empty() {
            return ToolResult {
                call_id: call_id.to_string(),
                output: "Missing required argument: token_id".to_string(),
                is_error: true,
            };
        }
        let hops = args.get("hops").and_then(|v| v.as_u64()).unwrap_or(1) as u32;
        let max_results = args
            .get("max_results")
            .and_then(|v| v.as_u64())
            .unwrap_or(20) as u32;

        // Verify the seed token exists
        let Some(_seed_token) = self.store.token_detail(token_id).await else {
            return ToolResult {
                call_id: call_id.to_string(),
                output: format!("Unknown seed token: {}", token_id),
                is_error: true,
            };
        };

        // Record hit on the seed token (spec §5.1: loading context = +1 hit)
        self.store.record_hit(token_id).await;

        // Get all relationships in the graph
        let relationships = self.store.get_relationships(None, None, None).await;

        // Build edges: (subject, object) pairs
        let edges: Vec<(String, String)> = relationships
            .iter()
            .map(|r| (r.subject.clone(), r.object.clone()))
            .collect();

        // Build node map for all tokens referenced in the graph, plus the seed
        let all_ids = self.store.list_token_ids().await;
        let mut all_nodes: std::collections::HashMap<String, squire_store::TraversalNode> =
            std::collections::HashMap::new();
        for id in &all_ids {
            if let Some(detail) = self.store.token_detail(id).await {
                all_nodes.insert(
                    id.clone(),
                    squire_store::TraversalNode {
                        token_id: id.clone(),
                        token_type: self
                            .store
                            .get_type(id)
                            .await
                            .unwrap_or_else(|| "unknown".to_string()),
                        short_desc: detail.short_desc.clone(),
                        tags: detail.tags.clone(),
                        properties: detail.properties.clone(),
                    },
                );
            }
        }

        // Seed: single token with score 1.0
        let direct = vec![(token_id.to_string(), 1.0f32)];

        // Walk the graph — pass |_| true because rdf() does not filter by type
        let mut results: Vec<TokenSummary> = squire_store::traverse_relationships(
            &direct,
            &edges,
            hops,
            &all_nodes,
            |_| true,
        );

        // Sort by hop_distance then score, truncate to max_results
        results.sort_by(|a, b| {
            a.hop_distance
                .cmp(&b.hop_distance)
                .then_with(|| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal))
        });
        results.truncate(max_results.max(1) as usize);

        ToolResult {
            call_id: call_id.to_string(),
            output: serde_json::to_string(&results).unwrap_or_else(|_| "[]".to_string()),
            is_error: false,
        }
    }
}

// ─────────────────────────── Invoke tool ───────────────────────────
// ... existing invoke tool code continues ...

/// Squire's `invoke(token_id, params)` tool — a ToolDefinition-only entry point.
///
/// This struct exists so the `invoke` tool has a `ToolDefinition` visible to
/// the AI. The actual execution is handled by `streaming_cmd.rs`'s dispatch
/// loop: when `tc.name == "invoke"`, it extracts `token_id` from `tc.arguments`
/// and dispatches to the real tool from `dispatch_registry`. The frontend
/// receives `stream-tool-call` with the real tool name, not `"invoke"`.
///
/// This `execute()` should never be called — if it is, something went wrong
/// in the orchestration.
pub struct SquireInvokeTool;

#[async_trait]
impl Tool for SquireInvokeTool {
    fn name(&self) -> &str {
        "invoke"
    }

    fn description(&self) -> &str {
        "Execute a tool or skill by its token_id (discovered via explore())."
    }

    fn input_schema(&self) -> Value {
        built_in_tool_definitions()[3].input_schema.clone()
    }

    fn danger(&self) -> crate::agent::ToolDanger {
        crate::agent::ToolDanger::Destructive
    }

    async fn execute(&self, _call_id: &str, _args: Value) -> ToolResult {
        // This should never be reached — the streaming orchestration
        // rewrites "invoke" tool calls before dispatch. If we get here,
        // the rewrite logic failed.
        ToolResult {
            call_id: String::new(),
            output: "Internal error: invoke tool was not redirected by orchestration".to_string(),
            is_error: true,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Batch composition syntax parser + tool (spec §3)
// ═══════════════════════════════════════════════════════════════════════

/// Parsed function call from a batch expression.
#[derive(Debug, Clone)]
pub enum BatchFunc {
    Explore {
        resource_type: String,
        query: String,
        num_hops: u32,
        max_results: u32,
        /// Which embedding to search: "content" (default) or "tag".
        vector: String,
    },
    Rdf {
        hops: u32,
        max_results: u32,
    },
    TokenToDetail {
        token_id: String,
    },
}

/// Parse a batch expression into groups of pipelined function calls.
/// `|` separates pipeline stages; `&`, `;`, and newlines separate groups.
///
/// Examples:
///   `explore(memory, rust, 1, 10) | rdf(2)`
///   `explore(memory, X, 1, 5) & explore(workflow, Y, 0, 3)`
pub fn parse_batch_expr(expr: &str) -> Result<Vec<Vec<BatchFunc>>, String> {
    // Split into groups on &, ;, or newlines (but not inside parens/quotes)
    let groups = split_groups(expr)?;
    let mut result = Vec::new();
    for group in &groups {
        let pipeline = split_pipeline(group)?;
        let mut funcs = Vec::new();
        for stage in &pipeline {
            funcs.push(parse_func_call(stage)?);
        }
        result.push(funcs);
    }
    if result.is_empty() {
        return Err("Empty batch expression".to_string());
    }
    Ok(result)
}

/// Split text into groups on `&`, `;`, or newlines, respecting parens and quotes.
fn split_groups(text: &str) -> Result<Vec<String>, String> {
    let mut groups = Vec::new();
    let mut current = String::new();
    let mut depth = 0i32;
    let mut in_single = false;
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\'' && !in_single {
            in_single = true;
        } else if c == '\'' && in_single {
            in_single = false;
        } else if !in_single {
            if c == '(' { depth += 1; }
            if c == ')' { depth -= 1; if depth < 0 { return Err("Unmatched ')' in batch expression".to_string()); } }
        }
        if depth == 0 && !in_single && (c == '&' || c == ';' || c == '\n') {
            let trimmed = current.trim().to_string();
            if !trimmed.is_empty() {
                groups.push(trimmed);
            }
            current = String::new();
            i += 1;
            continue;
        }
        current.push(c);
        i += 1;
    }
    let trimmed = current.trim().to_string();
    if !trimmed.is_empty() {
        groups.push(trimmed);
    }
    Ok(groups)
}

/// Split a group (no separators) into pipeline stages on `|`.
fn split_pipeline(text: &str) -> Result<Vec<String>, String> {
    let mut stages = Vec::new();
    let mut current = String::new();
    let mut depth = 0i32;
    let mut in_single = false;
    for c in text.chars() {
        if c == '\'' && !in_single {
            in_single = true;
        } else if c == '\'' && in_single {
            in_single = false;
        } else if !in_single {
            if c == '(' { depth += 1; }
            if c == ')' { depth -= 1; if depth < 0 { return Err("Unmatched ')' in pipeline".to_string()); } }
        }
        if depth == 0 && !in_single && c == '|' {
            let trimmed = current.trim().to_string();
            if trimmed.is_empty() { return Err("Empty pipeline stage before '|'".to_string()); }
            stages.push(trimmed);
            current = String::new();
            continue;
        }
        current.push(c);
    }
    let trimmed = current.trim().to_string();
    if !trimmed.is_empty() { stages.push(trimmed); }
    if stages.is_empty() { return Err("Empty pipeline".to_string()); }
    Ok(stages)
}

/// Parse a single function call like `explore(memory, rust, 1, 10)`.
fn parse_func_call(text: &str) -> Result<BatchFunc, String> {
    let text = text.trim();
    let open = text.find('(').ok_or_else(|| format!("Missing '(' in: {}", text))?;
    let close = text.rfind(')').ok_or_else(|| format!("Missing ')' in: {}", text))?;
    let name = text[..open].trim();
    let args_str = text[open + 1..close].trim();
    let args = parse_args(args_str)?;

    match name {
        "explore" => {
            if args.len() < 2 {
                return Err("explore() requires at least 2 args: (resource_type, query, [num_hops], [max_results], [vector])".to_string());
            }
            Ok(BatchFunc::Explore {
                resource_type: args.get(0).cloned().unwrap_or_default(),
                query: args.get(1).cloned().unwrap_or_default(),
                num_hops: args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0),
                max_results: args.get(3).and_then(|s| s.parse().ok()).unwrap_or(10),
                vector: args.get(4).cloned().unwrap_or_else(|| "content".to_string()),
            })
        }
        "rdf" => {
            if args.is_empty() {
                return Err("rdf() requires at least 1 arg: (hops, [max_results])".to_string());
            }
            Ok(BatchFunc::Rdf {
                hops: args.get(0).and_then(|s| s.parse().ok()).unwrap_or(1),
                max_results: args.get(1).and_then(|s| s.parse().ok()).unwrap_or(20),
            })
        }
        "token_to_detail" => {
            if args.is_empty() {
                return Err("token_to_detail() requires token_id".to_string());
            }
            Ok(BatchFunc::TokenToDetail {
                token_id: args[0].clone(),
            })
        }
        other => Err(format!("Unknown batch function: {}", other)),
    }
}

/// Parse comma-separated args, handling single-quoted strings (which may contain commas).
fn parse_args(s: &str) -> Result<Vec<String>, String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_quote = false;
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\'' && !in_quote {
            in_quote = true;
            i += 1;
            continue;
        }
        if c == '\'' && in_quote {
            in_quote = false;
            i += 1;
            continue;
        }
        if c == ',' && !in_quote {
            args.push(current.trim().to_string());
            current = String::new();
            i += 1;
            continue;
        }
        current.push(c);
        i += 1;
    }
    let trimmed = current.trim().to_string();
    if !trimmed.is_empty() { args.push(trimmed); }
    if in_quote { return Err("Unclosed single quote in args".to_string()); }
    Ok(args)
}

// ─────────────────────────── Batch tool ───────────────────────────

pub struct SquireBatchTool {
    pub store: Arc<dyn SquireStore>,
    /// Tool definitions snapshot for explore() in the live registry.
    pub tool_defs: Vec<ToolDefinition>,
    pub session_id: SessionId,
    /// Shared per-turn batch counter (counts as 1 call against the cap).
    pub batch_counter: Arc<AtomicU32>,
    pub batch_cap: u32,
}

#[async_trait]
impl Tool for SquireBatchTool {
    fn name(&self) -> &str { "batch" }
    fn description(&self) -> &str {
        "Compose explore/rdf/token_to_detail calls into one batch expression using | (pipe) and & or ; (parallel)."
    }
    fn input_schema(&self) -> Value {
        built_in_tool_definitions()[4].input_schema.clone()
    }
    async fn execute(&self, call_id: &str, args: Value) -> ToolResult {
        if let Some(err) = check_batch_cap(&self.batch_counter, self.batch_cap, "batch") {
            return err;
        }
        let expr = args.get("expression").and_then(|v| v.as_str()).unwrap_or("");
        if expr.is_empty() {
            return ToolResult { call_id: call_id.to_string(), output: "Missing 'expression' argument".to_string(), is_error: true };
        }

        let groups = match parse_batch_expr(expr) {
            Ok(g) => g,
            Err(e) => return ToolResult { call_id: call_id.to_string(), output: format!("Batch parse error: {}", e), is_error: true },
        };

        let current_turn = self.store.current_turn(self.session_id).await;
        let mut all_results: Vec<TokenSummary> = Vec::new();
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

        // Execute each parallel group independently, merge results
        for group in &groups {
            let group_results = self.exec_pipeline(group, current_turn).await;
            for t in group_results {
                if seen.insert(t.token_id.clone()) {
                    all_results.push(t);
                }
            }
        }

        all_results.truncate(200); // cap at reasonable max
        ToolResult {
            call_id: call_id.to_string(),
            output: serde_json::to_string(&all_results).unwrap_or_else(|_| "[]".to_string()),
            is_error: false,
        }
    }
}

impl SquireBatchTool {
    /// Execute a pipeline: first func produces seed tokens, each subsequent
    /// `| rdf(hops, max)` walks from those seeds.
    async fn exec_pipeline(&self, funcs: &[BatchFunc], current_turn: u64) -> Vec<TokenSummary> {
        if funcs.is_empty() { return vec![]; }

        // Execute first function — produces seed tokens
        let seeds = self.exec_func(&funcs[0], &[], current_turn).await;

        if funcs.len() == 1 {
            return seeds;
        }

        // For each subsequent `| rdf(...)`, walk from all seed tokens
        let mut current_seeds = seeds;
        for func in &funcs[1..] {
            current_seeds = self.exec_func(func, &current_seeds, current_turn).await;
        }
        current_seeds
    }

    /// Execute a single function, optionally using seeds as input for rdf.
    async fn exec_func(&self, func: &BatchFunc, seeds: &[TokenSummary], current_turn: u64) -> Vec<TokenSummary> {
        match func {
            BatchFunc::Explore { resource_type, query, num_hops, max_results, vector } => {
                if matches!(resource_type.as_str(), "tool" | "tool_skill") {
                    // Tool registry path — substring match
                    let ql = query.to_lowercase();
                    let mut results = Vec::new();
                    for d in &self.tool_defs {
                        let matched = ql.is_empty()
                            || d.name.to_lowercase().contains(&ql)
                            || d.description.to_lowercase().contains(&ql);
                        if matched && (results.len() as u32) < *max_results {
                            results.push(TokenSummary {
                                token_id: d.name.clone(), token_type: "tool".to_string(),
                                score: 1.0, short_desc: d.description.clone(),
                                accumulated_hits: 0, hop_distance: 0, via_token_id: None, tags: vec![], properties: std::collections::HashMap::new(),
                            });
                        }
                    }
                    if resource_type == "tool_skill" {
                        let skills = self.store.explore_memory(
                            "skill", query, *num_hops, *max_results, current_turn, self.session_id, vector,
                        ).await;
                        results.extend(skills);
                    }
                    results
                } else {
                    self.store.explore_memory(
                        resource_type, query, *num_hops, *max_results, current_turn, self.session_id, vector,
                    ).await
                }
            }
            BatchFunc::Rdf { hops, max_results } => {
                if seeds.is_empty() { return vec![]; }
                // Build edges and nodes for traversal
                let relationships = self.store.get_relationships(None, None, None).await;
                let edges: Vec<(String, String)> = relationships.iter()
                    .map(|r| (r.subject.clone(), r.object.clone())).collect();
                let all_ids = self.store.list_token_ids().await;
                let mut all_nodes: std::collections::HashMap<String, squire_store::TraversalNode> =
                    std::collections::HashMap::new();
                for id in &all_ids {
                    if let Some(detail) = self.store.token_detail(id).await {
                        all_nodes.insert(id.clone(), squire_store::TraversalNode {
                            token_id: id.clone(),
                            token_type: self.store.get_type(id).await.unwrap_or_else(|| "unknown".to_string()),
                            short_desc: detail.short_desc.clone(),
                            tags: detail.tags.clone(),
                            properties: detail.properties.clone(),
                        });
                    }
                }
                let direct: Vec<(String, f32)> = seeds.iter()
                    .map(|s| (s.token_id.clone(), s.score)).collect();
                let mut results = squire_store::traverse_relationships(
                    &direct, &edges, *hops, &all_nodes, |_| true,
                );
                results.sort_by(|a, b| {
                    a.hop_distance.cmp(&b.hop_distance)
                        .then_with(|| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal))
                });
                results.truncate((*max_results).max(1) as usize);
                results
            }
            BatchFunc::TokenToDetail { token_id } => {
                match self.store.token_detail(token_id).await {
                    Some(detail) => {
                        self.store.record_hit(token_id).await;
                        let desc = detail.full_desc.unwrap_or(detail.short_desc);
                        vec![TokenSummary {
                            token_id: token_id.clone(), token_type: "detail".to_string(),
                            score: 1.0, short_desc: desc, accumulated_hits: 0,
                            hop_distance: 0, via_token_id: None, tags: vec![], properties: std::collections::HashMap::new(),
                        }]
                    }
                    None => vec![],
                }
            }
        }
    }
}
