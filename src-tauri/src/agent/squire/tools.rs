//! The three built-in Squire protocol tools: `explore`, `token_to_detail`,
//! and `invoke`.
//!
//! `explore` and `token_to_detail` are the discovery surface. `invoke`
//! proxies a call to a tool/skill discovered via `explore()`, identified by
//! its `token_id`. The streaming orchestration in `streaming_cmd.rs`
//! rewrites the `stream-tool-call` event so the frontend sees the real tool
//! name (not "invoke"), keeping the UI consistent with Legacy mode.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;

use super::SquireStore;
use super::types::TokenSummary;
use crate::agent::{Tool, ToolResult};
use crate::llm::provider::ToolDefinition;
use crate::storage::conversation_store::SessionId;

// ─────────────────────────── Tool definitions ───────────────────────────

pub fn built_in_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "explore".to_string(),
            description: "Search Squire memory and registered resources by semantic similarity, optionally expanding via graph traversal.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "resource_type": {
                        "type": "string",
                        "enum": ["workflow", "tool", "skill", "tool_skill", "memory", "concept", "referential", "all"]
                    },
                    "query": {"type": "string"},
                    "num_hops": {"type": "integer", "minimum": 0},
                    "max_results": {"type": "integer", "minimum": 1}
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
        let current_turn = self.store.current_turn(self.session_id).await;

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
                        via_token_id: None,
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
                    .explore_memory("skill", &query, num_hops, max_results, current_turn)
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
                .explore_memory(&resource_type, &query, num_hops, max_results, current_turn)
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

// ─────────────────────────── Invoke tool ───────────────────────────

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
        built_in_tool_definitions()[2].input_schema.clone()
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
