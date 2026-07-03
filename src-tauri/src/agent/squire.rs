//! Squire context mode: `SquireContextAdapter` plus the three built-in tools
//! (`explore`, `token_to_detail`, `invoke`) it exposes as the model's entire
//! tool surface (Q5 — strict gateway boundary).
//!
//! Scope for this node (see `.AiControl/root/Squire/squire-adapter`):
//! adapter control flow, strict tool-surface enforcement, and the protocol
//! validation gates that drive retry/compliance-failure classification (Q6).
//! The `SquireStore` trait below is the storage contract `squire-storage`
//! implements for real (LanceDB-backed); `InMemorySquireStore` here is a
//! non-persistent stand-in so this node is testable end-to-end without it.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::llm::provider::{ChatMessage, ChatRole, ToolCall, ToolDefinition};
use crate::storage::conversation_store::{
    ConversationStore, MessageRole, NewMessage, SessionId, SessionWithMessages,
};

use super::context_adapter::{ContextManagerAdapter, TurnInput, TurnOutcome};
use super::{Tool, ToolDanger, ToolRegistry, ToolResult};

// ─────────────────────────── Storage contract ───────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenSummary {
    pub token_id: String,
    #[serde(rename = "type")]
    pub token_type: String,
    pub score: f32,
    pub short_desc: String,
    /// Hit-count bookkeeping (spec §3.2/§3.3) — strictly additive, never
    /// decremented. Exposed here so callers/tests can see the raw count
    /// alongside the derived `effective_priority` used for ranking.
    #[serde(default)]
    pub accumulated_hits: u64,
    /// Graph-traversal provenance (spec §4.2/§6.1/§7.1): 0 for tokens that
    /// directly matched the vector/type filter, N for tokens discovered by
    /// walking the relationship graph N hops out from a direct match.
    #[serde(default)]
    pub hop_distance: u32,
    /// For traversal-discovered tokens (`hop_distance > 0`), the direct-match
    /// token that led to this token's discovery (the BFS parent). `None` for
    /// direct matches themselves.
    #[serde(default)]
    pub via_token_id: Option<String>,
}

/// Enough connection/dispatch info to re-invoke a tool purely from stored
/// metadata, without it being present in the current turn's live
/// `ToolRegistry` (`token-detail-endpoint`: the endpoint-carrying `TokenDetail`
/// extension `squire-storage/decisions.md` originally flagged as a second,
/// separate "full cutover" piece beyond ingestion itself). Only an `Mcp`
/// variant exists — local/built-in tools are registered unconditionally on
/// every turn (`ToolRegistry::new()`, no config/enablement gate), so a
/// built-in token can never actually be "ingested but not currently live";
/// see decisions.md's proportionality assessment for the full reasoning.
///
/// SECURITY: `McpServerConfig` can carry `env`/`headers`, which may include
/// secrets (e.g. an API key for an authenticated MCP server). This type must
/// never be exposed to the model — `SquireTokenToDetailTool::execute` only
/// ever reads `TokenDetail::short_desc`/`full_desc`, never re-serializes the
/// whole struct; keep it that way.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ToolEndpoint {
    Mcp {
        server: crate::state::config::McpServerConfig,
        remote_name: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenDetail {
    pub short_desc: String,
    pub full_desc: Option<String>,
    /// See `ToolEndpoint` — `None` for every non-tool token, for local-builtin
    /// tool tokens, and for MCP tool tokens ingested before this field
    /// existed (self-healing: the very next per-turn re-ingestion backfills
    /// it for any MCP tool that's live again that turn).
    #[serde(default)]
    pub endpoint: Option<ToolEndpoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NewTokenSpec {
    pub id: String,
    #[serde(rename = "type")]
    pub token_type: String,
    pub short_desc: String,
    #[serde(default)]
    pub full_desc: Option<String>,
    /// See `ToolEndpoint`/`TokenDetail::endpoint`.
    #[serde(default)]
    pub endpoint: Option<ToolEndpoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Relationship {
    pub subject: String,
    pub predicate: String,
    pub object: String,
}

/// Structured diagnostic record for a compliance failure that exhausted the
/// retry budget (Q6: "store structured failure metadata (rule violated,
/// validator reason, retry count, timestamp) for debugging"). `rule` and
/// `reason` are currently the same string — `validate_squire_response` and
/// the JSON-parse/ask_user-gap paths in `finalize_turn` don't yet classify
/// failures into a separate rule-id taxonomy vs. free-text reason, so `rule`
/// carries a short machine-stable category and `reason` the full message.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ComplianceFailureRecord {
    pub session_id: SessionId,
    pub rule: String,
    pub reason: String,
    pub retry_count: u32,
    pub failed_content: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Raw-partition audit-log record (spec §4.1/§4.3/§9.4 step 4/§11): the
/// unmarked residual of a compliant turn's AI output — the portion of
/// `content` that was NOT enclosed in a `§^...§^` span, i.e. content the AI
/// produced but did not promote into a structured, addressable memory token.
/// Append-only, write-only from the model's perspective — see
/// `raw-partition-storage/decisions.md` for why no `SquireStore` method
/// reads this back.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawPartitionRecord {
    pub session_id: SessionId,
    pub turn: u64,
    pub content: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Contract `squire-storage` implements against LanceDB (Q4). Everything here
/// is scoped to what `SquireContextAdapter` and the built-in tools need to
/// function; graph traversal depth (`num_hops`) is accepted but the
/// in-memory stand-in below does not perform real traversal.
#[async_trait]
pub trait SquireStore: Send + Sync {
    async fn token_exists(&self, token_id: &str) -> bool;
    async fn upsert_token(&self, token: NewTokenSpec, creation_turn: u64);
    async fn insert_relationship(&self, rel: Relationship);
    async fn set_preserve_list(&self, session_id: SessionId, tokens: Vec<String>);
    async fn preserved_tokens(&self, session_id: SessionId) -> Vec<TokenSummary>;
    /// `current_turn` is the requesting session's turn count, used only to
    /// compute `effective_priority` (spec §3.3:
    /// `accumulated_hits - (current_turn - creation_turn)`) for ranking —
    /// it does not otherwise affect which tokens match.
    async fn explore_memory(
        &self,
        resource_type: &str,
        query: &str,
        num_hops: u32,
        max_results: u32,
        current_turn: u64,
    ) -> Vec<TokenSummary>;
    async fn token_detail(&self, token_id: &str) -> Option<TokenDetail>;
    /// Current turn number for a session (0 before the first close).
    async fn current_turn(&self, session_id: SessionId) -> u64;
    /// Advance the turn counter — called once per turn close (spec §9.4 step 9).
    async fn increment_turn(&self, session_id: SessionId);
    /// Increment a token's `accumulated_hits` by 1 (spec §3.3's hit-count
    /// events; wired at the `token_to_detail` call site per §6.2, the
    /// preserve-list bootstrap-load path, and — since `hit-count-fidelity` —
    /// at `finalize_turn` for every already-existing token `§!`-referenced
    /// in a compliant response's content, per §3.3's table). No-op if the
    /// token doesn't exist.
    async fn record_hit(&self, token_id: &str);
    /// Persist a compliance-failure diagnostic (Q6). Append-only, debugging
    /// aid only — never read back to drive runtime behavior.
    async fn record_compliance_failure(&self, record: ComplianceFailureRecord);
    /// Clear every session's preserve-list carryover (Q7: "restart clears
    /// pending preserve carryover state" — preserved_tokens is an ephemeral
    /// next-turn-only handoff, not long-lived continuity state). Called once
    /// at app startup, before any session resumes activity.
    async fn clear_all_preserve_lists(&self);
    /// Persists the unmarked residual of a compliant turn's AI output — the
    /// portion of `content` that was NOT enclosed in a `§^...§^` span (spec
    /// §4.1/§4.3: "if the AI does not mark a span, it is stored only in the
    /// raw partition"). Append-only, debugging/audit aid only — no
    /// `SquireStore` method reads this back (spec: "Explore() does not
    /// search this partition by default"; no other read mechanism is
    /// described anywhere in the protocol). Callers should skip this call
    /// entirely when there is nothing to store rather than persist an empty
    /// row (see `finalize_turn`'s call site).
    async fn record_raw_output(&self, session_id: SessionId, turn: u64, content: String);
}

#[derive(Default)]
struct StoredToken {
    token_type: String,
    short_desc: String,
    full_desc: Option<String>,
    creation_turn: u64,
    accumulated_hits: u64,
    endpoint: Option<ToolEndpoint>,
}

/// `effective_priority = accumulated_hits - (current_turn - creation_turn)`
/// (spec §3.3, implemented literally as signed arithmetic since the result
/// is explicitly allowed to go negative for stale, never-referenced tokens).
pub fn effective_priority(accumulated_hits: u64, current_turn: u64, creation_turn: u64) -> i64 {
    accumulated_hits as i64 - (current_turn as i64 - creation_turn as i64)
}

/// Sort a candidate list by score descending (spec §6.1), breaking
/// near-ties (within `SCORE_TIE_EPSILON`) by `effective_priority` descending.
/// Exact float equality is unlikely for real cosine-similarity scores, so an
/// epsilon bucket is used rather than requiring bit-exact ties — otherwise
/// `effective_priority` would practically never be consulted.
const SCORE_TIE_EPSILON: f32 = 1e-6;

pub fn sort_by_score_then_priority(
    results: &mut [TokenSummary],
    priorities: &HashMap<String, i64>,
) {
    results.sort_by(|a, b| {
        if (a.score - b.score).abs() <= SCORE_TIE_EPSILON {
            let pa = priorities.get(&a.token_id).copied().unwrap_or(0);
            let pb = priorities.get(&b.token_id).copied().unwrap_or(0);
            pb.cmp(&pa)
        } else {
            b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal)
        }
    });
}

/// One node's minimal traversal-relevant shape, backend-agnostic, used by
/// the shared BFS helper below (spec §4.2/§6.1/§7.1: graph traversal over
/// the relationship triplet store, undirected for reachability purposes —
/// see `decisions.md` for why subject/object are treated symmetrically).
pub struct TraversalNode {
    pub token_id: String,
    pub token_type: String,
    pub short_desc: String,
}

/// Backend-agnostic BFS: given the directly-matched hop-0 tokens (with their
/// query-similarity scores) and the full relationship edge list, walks the
/// graph outward up to `num_hops` hops and returns newly-discovered tokens
/// (i.e. not already present in `direct_ids`) with hop-distance/provenance
/// metadata and a decayed score (`base_score * 0.5^hop_distance`, where
/// `base_score` is the originating hop-0 match's score — spec §7.3: a
/// graph-connected token "might not score well on raw vector similarity
/// alone", so it isn't given a fabricated similarity score of its own).
pub fn traverse_relationships(
    direct: &[(String, f32)],
    edges: &[(String, String)], // (subject, object) pairs, undirected for traversal
    num_hops: u32,
    all_nodes: &HashMap<String, TraversalNode>,
    type_matches: impl Fn(&str) -> bool,
) -> Vec<TokenSummary> {
    if num_hops == 0 || direct.is_empty() {
        return Vec::new();
    }

    // Undirected adjacency map.
    let mut adjacency: HashMap<&str, Vec<&str>> = HashMap::new();
    for (s, o) in edges {
        adjacency.entry(s.as_str()).or_default().push(o.as_str());
        adjacency.entry(o.as_str()).or_default().push(s.as_str());
    }

    let direct_ids: HashSet<&str> = direct.iter().map(|(id, _)| id.as_str()).collect();
    // discovered: token_id -> (hop_distance, via_token_id, base_score)
    let mut discovered: HashMap<String, (u32, String, f32)> = HashMap::new();
    let mut frontier: Vec<(String, String, f32)> = direct
        .iter()
        .map(|(id, score)| (id.clone(), id.clone(), *score))
        .collect();

    for hop in 1..=num_hops {
        let mut next_frontier = Vec::new();
        for (node_id, origin_id, base_score) in &frontier {
            let Some(neighbors) = adjacency.get(node_id.as_str()) else {
                continue;
            };
            for &neighbor in neighbors {
                if direct_ids.contains(neighbor) {
                    continue; // already a direct match, not a traversal discovery
                }
                if discovered.contains_key(neighbor) {
                    continue; // keep the shortest-hop-distance record already found
                }
                discovered.insert(
                    neighbor.to_string(),
                    (hop, origin_id.clone(), *base_score),
                );
                next_frontier.push((neighbor.to_string(), origin_id.clone(), *base_score));
            }
        }
        frontier = next_frontier;
        if frontier.is_empty() {
            break;
        }
    }

    discovered
        .into_iter()
        .filter_map(|(token_id, (hop_distance, via_token_id, base_score))| {
            let node = all_nodes.get(&token_id)?;
            if !type_matches(&node.token_type) {
                return None;
            }
            Some(TokenSummary {
                token_id: node.token_id.clone(),
                token_type: node.token_type.clone(),
                score: base_score * 0.5f32.powi(hop_distance as i32),
                short_desc: node.short_desc.clone(),
                accumulated_hits: 0,
                hop_distance,
                via_token_id: Some(via_token_id),
            })
        })
        .collect()
}

/// Non-persistent stand-in for the LanceDB-backed store `squire-storage`
/// will deliver. State lives only for the lifetime of the process.
#[derive(Default)]
pub struct InMemorySquireStore {
    tokens: Mutex<HashMap<String, StoredToken>>,
    relationships: Mutex<Vec<Relationship>>,
    preserve_lists: Mutex<HashMap<SessionId, Vec<String>>>,
    turns: Mutex<HashMap<SessionId, u64>>,
    compliance_failures: Mutex<Vec<ComplianceFailureRecord>>,
    raw_partition: Mutex<Vec<RawPartitionRecord>>,
}

impl InMemorySquireStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Test-harness-only accessor for the raw-partition audit log — mirrors
    /// how existing tests already inspect a `RecordingStore`'s appended
    /// messages directly. Not part of the `SquireStore` trait (deliberately
    /// — see `raw-partition-storage/decisions.md`'s "no read-back" design).
    #[cfg(test)]
    pub async fn raw_partition_records(&self) -> Vec<RawPartitionRecord> {
        self.raw_partition.lock().await.clone()
    }
}

#[async_trait]
impl SquireStore for InMemorySquireStore {
    async fn token_exists(&self, token_id: &str) -> bool {
        self.tokens.lock().await.contains_key(token_id)
    }

    async fn upsert_token(&self, token: NewTokenSpec, creation_turn: u64) {
        let mut tokens = self.tokens.lock().await;
        tokens
            .entry(token.id.clone())
            .and_modify(|t| {
                t.short_desc = token.short_desc.clone();
                if token.full_desc.is_some() {
                    t.full_desc = token.full_desc.clone();
                }
                if token.endpoint.is_some() {
                    t.endpoint = token.endpoint.clone();
                }
                // Spec §9.4 step 5 / §5.2: accumulated_hits increments on
                // every upsert "regardless" — both the new_tokens-at-close
                // path and the §^-span-reuse-of-existing-token path funnel
                // through this same call.
                t.accumulated_hits += 1;
            })
            .or_insert(StoredToken {
                token_type: token.token_type.clone(),
                short_desc: token.short_desc.clone(),
                full_desc: token.full_desc.clone(),
                creation_turn,
                accumulated_hits: 1,
                endpoint: token.endpoint.clone(),
            });
    }

    async fn insert_relationship(&self, rel: Relationship) {
        self.relationships.lock().await.push(rel);
    }

    async fn set_preserve_list(&self, session_id: SessionId, tokens: Vec<String>) {
        self.preserve_lists.lock().await.insert(session_id, tokens);
    }

    async fn preserved_tokens(&self, session_id: SessionId) -> Vec<TokenSummary> {
        let ids = self
            .preserve_lists
            .lock()
            .await
            .get(&session_id)
            .cloned()
            .unwrap_or_default();
        let mut tokens = self.tokens.lock().await;
        let mut out = Vec::new();
        for id in ids {
            if let Some(t) = tokens.get_mut(&id) {
                // Spec §3.3: "Token in preserve list loaded at turn open" +1.
                t.accumulated_hits += 1;
                out.push(TokenSummary {
                    token_id: id.clone(),
                    token_type: t.token_type.clone(),
                    score: 0.0,
                    short_desc: t.short_desc.clone(),
                    accumulated_hits: t.accumulated_hits,
                    hop_distance: 0,
                    via_token_id: None,
                });
            }
        }
        out
    }

    async fn explore_memory(
        &self,
        resource_type: &str,
        query: &str,
        num_hops: u32,
        max_results: u32,
        current_turn: u64,
    ) -> Vec<TokenSummary> {
        let q = query.to_lowercase();
        let tokens = self.tokens.lock().await;
        let type_matches = |t: &str| {
            resource_type == "all"
                || t == resource_type
                || (resource_type == "memory"
                    && (t == "concept" || t == "referential" || t == "system_referential"))
                || (resource_type == "tool_skill" && t == "skill")
        };
        let mut direct: Vec<TokenSummary> = tokens
            .iter()
            .filter(|(_, t)| type_matches(&t.token_type))
            .filter(|(id, t)| {
                q.is_empty()
                    || id.to_lowercase().contains(&q)
                    || t.short_desc.to_lowercase().contains(&q)
            })
            .map(|(id, t)| TokenSummary {
                token_id: id.clone(),
                token_type: t.token_type.clone(),
                score: 1.0,
                short_desc: t.short_desc.clone(),
                accumulated_hits: t.accumulated_hits,
                hop_distance: 0,
                via_token_id: None,
            })
            .collect();

        // Graph traversal (spec §4.2/§6.1/§7.1): expand outward from the
        // direct matches up to num_hops, over *all* tokens (traversal
        // discovery isn't itself query-filtered — only type-filtered, since
        // a connected token might not match the query text at all, per
        // §7.3), regardless of the query-text filter applied to direct hits.
        if num_hops > 0 && !direct.is_empty() {
            let all_nodes: HashMap<String, TraversalNode> = tokens
                .iter()
                .map(|(id, t)| {
                    (
                        id.clone(),
                        TraversalNode {
                            token_id: id.clone(),
                            token_type: t.token_type.clone(),
                            short_desc: t.short_desc.clone(),
                        },
                    )
                })
                .collect();
            let relationships = self.relationships.lock().await;
            let edges: Vec<(String, String)> = relationships
                .iter()
                .map(|r| (r.subject.clone(), r.object.clone()))
                .collect();
            drop(relationships);
            let direct_scores: Vec<(String, f32)> =
                direct.iter().map(|t| (t.token_id.clone(), t.score)).collect();
            let mut expanded = traverse_relationships(
                &direct_scores,
                &edges,
                num_hops,
                &all_nodes,
                type_matches,
            );
            for t in &mut expanded {
                t.accumulated_hits = tokens.get(&t.token_id).map(|s| s.accumulated_hits).unwrap_or(0);
            }
            direct.extend(expanded);
        }

        let priorities: HashMap<String, i64> = direct
            .iter()
            .filter_map(|t| {
                tokens.get(&t.token_id).map(|stored| {
                    (
                        t.token_id.clone(),
                        effective_priority(stored.accumulated_hits, current_turn, stored.creation_turn),
                    )
                })
            })
            .collect();
        sort_by_score_then_priority(&mut direct, &priorities);
        direct.truncate(max_results.max(1) as usize);
        direct
    }

    async fn token_detail(&self, token_id: &str) -> Option<TokenDetail> {
        self.tokens.lock().await.get(token_id).map(|t| TokenDetail {
            short_desc: t.short_desc.clone(),
            full_desc: t.full_desc.clone(),
            endpoint: t.endpoint.clone(),
        })
    }

    async fn current_turn(&self, session_id: SessionId) -> u64 {
        *self.turns.lock().await.get(&session_id).unwrap_or(&0)
    }

    async fn increment_turn(&self, session_id: SessionId) {
        let mut turns = self.turns.lock().await;
        *turns.entry(session_id).or_insert(0) += 1;
    }

    async fn record_compliance_failure(&self, record: ComplianceFailureRecord) {
        self.compliance_failures.lock().await.push(record);
    }

    async fn clear_all_preserve_lists(&self) {
        self.preserve_lists.lock().await.clear();
    }

    async fn record_hit(&self, token_id: &str) {
        if let Some(t) = self.tokens.lock().await.get_mut(token_id) {
            t.accumulated_hits += 1;
        }
    }

    async fn record_raw_output(&self, session_id: SessionId, turn: u64, content: String) {
        self.raw_partition.lock().await.push(RawPartitionRecord {
            session_id,
            turn,
            content,
            timestamp: chrono::Utc::now(),
        });
    }
}

// ─────────────────────────── Protocol (spec §8) ───────────────────────────

#[derive(Debug, Clone, Deserialize, Default, PartialEq)]
#[serde(default)]
pub struct SquireResponse {
    pub ask_user: String,
    pub content: String,
    pub preserve: Vec<String>,
    pub new_tokens: Vec<NewTokenSpec>,
    pub relationships: Vec<Relationship>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ComplianceFailure {
    pub reason: String,
}

/// Terminated by whitespace or the next `§`, per spec §5.1/§5.2.
fn take_token_id(s: &str) -> String {
    s.chars().take_while(|c| !c.is_whitespace()).collect()
}

/// `§!TokenID` occurrences in `content` (spec §5.1).
fn extract_inline_refs(content: &str) -> Vec<String> {
    content
        .split('§')
        .skip(1)
        .filter_map(|part| part.strip_prefix('!'))
        .map(take_token_id)
        .filter(|id| !id.is_empty())
        .collect()
}

/// `§^TokenID content §^` spans (spec §5.2). Returns the closed spans found
/// and, if the content ends mid-span, the token id of the unclosed one.
fn extract_spans(content: &str) -> (Vec<(String, String)>, Option<String>) {
    let parts: Vec<&str> = content.split("§^").collect();
    let mut spans = Vec::new();
    let mut unclosed = None;
    let mut i = 1;
    while i < parts.len() {
        let opening = parts[i];
        let token_id = take_token_id(opening);
        if token_id.is_empty() {
            // Bare `§^` with nothing pending open — not a valid open tag; skip.
            i += 1;
            continue;
        }
        let rest = &opening[token_id.len()..];
        if i + 1 < parts.len() {
            spans.push((token_id, rest.trim().to_string()));
            i += 2;
        } else {
            unclosed = Some(token_id);
            i += 1;
        }
    }
    (spans, unclosed)
}

fn strip_span_markers(content: &str) -> String {
    let parts: Vec<&str> = content.split("§^").collect();
    let mut out = String::new();
    out.push_str(parts[0]);
    let mut i = 1;
    while i < parts.len() {
        let opening = parts[i];
        let token_id = take_token_id(opening);
        if token_id.is_empty() {
            i += 1;
            continue;
        }
        out.push_str(opening[token_id.len()..].trim());
        i += 1;
        if i < parts.len() {
            out.push_str(parts[i]);
            i += 1;
        }
    }
    out
}

/// Raw-partition extraction (spec §4.1/§4.3: "if the AI does not mark a
/// span, it is stored only in the raw partition"). Returns the portion of
/// `content` that falls OUTSIDE every closed `§^...§^` span — the text the
/// AI produced but did not promote into a structured, addressable memory
/// token. A close sibling of `strip_span_markers` (same `split("§^")`
/// traversal shape), but where that function *keeps* span bodies (for clean
/// display prose) and discards only the markers, this function discards the
/// span bodies too, keeping only the text outside them. Segments are joined
/// with a single space and the result is trimmed, so a response that is
/// entirely one closed span (nothing before or after it) correctly yields
/// an empty string — see `finalize_turn`'s call site for why callers should
/// skip persisting an empty result rather than write a pointless empty row.
fn unmarked_residual(content: &str) -> String {
    let parts: Vec<&str> = content.split("§^").collect();
    let mut segments: Vec<&str> = vec![parts[0]];
    let mut i = 1;
    while i < parts.len() {
        let opening = parts[i];
        let token_id = take_token_id(opening);
        if token_id.is_empty() {
            // Bare `§^` with nothing pending open — not a valid open tag;
            // its trailing text (up to the next marker, if any) is outside
            // any span, so it counts as unmarked residual.
            segments.push(opening);
            i += 1;
            continue;
        }
        // `opening[token_id.len()..]` is the span body (if closed) — never
        // pushed to `segments`, since it belongs to the structured
        // partition, not the raw one.
        i += 1;
        if i < parts.len() {
            // parts[i] is the text after this span's closing `§^` marker,
            // up to the next marker (or end of content) — outside any span.
            segments.push(parts[i]);
            i += 1;
        }
        // If `i >= parts.len()` here, the span was unclosed — in practice
        // unreachable at finalize_turn's call site, since
        // validate_squire_response already rejects unclosed spans before
        // this function is ever called on a compliant response.
    }
    segments
        .into_iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Validity rules from spec §8.3 (excluding the `invoke()` non-invocable-token
/// rule, which is checked at call time inside `SquireInvokeTool`).
pub fn validate_squire_response(
    resp: &SquireResponse,
    token_known: impl Fn(&str) -> bool,
) -> Result<(), ComplianceFailure> {
    if !resp.ask_user.is_empty() && !resp.content.is_empty() {
        return Err(ComplianceFailure {
            reason: "ask_user and content cannot coexist".to_string(),
        });
    }

    if !resp.ask_user.is_empty() {
        return Ok(());
    }

    if resp.content.is_empty()
        && resp.new_tokens.is_empty()
        && resp.relationships.is_empty()
        && resp.preserve.is_empty()
    {
        return Err(ComplianceFailure {
            reason: "empty close response".to_string(),
        });
    }

    for token_id in extract_inline_refs(&resp.content) {
        let defined_inline = resp.new_tokens.iter().any(|t| t.id == token_id);
        if !defined_inline && !token_known(&token_id) {
            return Err(ComplianceFailure {
                reason: format!("undisplayable token §!{}", token_id),
            });
        }
    }

    let (_, unclosed) = extract_spans(&resp.content);
    if let Some(token_id) = unclosed {
        return Err(ComplianceFailure {
            reason: format!("unclosed §^ span {}", token_id),
        });
    }

    Ok(())
}

// ─────────────────────────── Tool-token ingestion (ss-9) ───────────────────────────

/// Deterministic token id for a tool discovered via the live `ToolRegistry`:
/// the registry name itself, unprefixed. Local built-ins have fixed,
/// hardcoded names; MCP tools get a stable `mcp_{server_id}_{tool_id}` local
/// name assigned once per discovery pass by `streaming_cmd.rs`'s existing
/// sanitization scheme — as long as neither a server's configured id nor a
/// remote tool's advertised name changes, this is stable across repeated
/// ingestion calls (see decisions.md's "Token ID scheme" section for why an
/// unprefixed id, matching the registry name exactly, is required for
/// `SquireInvokeTool`'s registry-primary lookup to stay consistent with a
/// token discovered via `explore(resource_type="tool_skill")`).
pub fn tool_token_id(registry_name: &str) -> String {
    registry_name.to_string()
}

/// Full description for an ingested tool token, matching spec §3.1's
/// type-enforced format for tools ("Standard MCP tool schema — name,
/// description, input schema") and, byte-for-byte, the same JSON shape
/// `SquireTokenToDetailTool::execute`'s existing `detail_level == "full"`
/// branch already returns for registry-sourced tools — so a caller sees the
/// same "full tool description" shape whether it came from the live
/// registry or from an ingested store row.
fn tool_full_desc(def: &ToolDefinition) -> String {
    serde_json::json!({
        "name": def.name,
        "description": def.description,
        "input_schema": def.input_schema,
    })
    .to_string()
}

/// Ingests the app's real tool registry into `store` as `tool`-typed tokens
/// (ss-9: `squire-storage/todo.json`'s flagged follow-up — "a write path
/// that turns MCP/local tool discovery into persisted, invocable SquireStore
/// token rows"). Backend-agnostic: calls only `SquireStore::upsert_token`, so
/// it works unmodified against both `InMemorySquireStore` and
/// `LanceDbSquireStore`, and against any future implementation of the trait.
///
/// `creation_turn` is passed as `0` for every tool — tool discovery is not
/// scoped to any one session's turn counter (see decisions.md's
/// "`creation_turn`" section for the full rationale); `upsert_token`'s
/// existing "preserve creation_turn on update" semantics mean this only
/// matters for a token's very first ingestion.
///
/// Intended call site: once per turn, immediately after `ToolRegistry` is
/// fully assembled (local built-ins + MCP discovery) in
/// `commands::streaming_cmd::send_message_impl` — the one point in the
/// codebase where "the full, current set of available tools" is known, for
/// both Legacy and Squire mode sessions (see decisions.md's "Trigger point"
/// section for why this is the correct, and only real, trigger point today).
///
/// `endpoints` (added by `token-detail-endpoint`): an optional side-channel
/// map from a tool's registry name (the same name `tool_token_id` uses as
/// the token id) to the `ToolEndpoint` needed to re-dispatch it purely from
/// stored metadata. `ToolDefinition` itself erases MCP-vs-local origin once a
/// tool is registered, so callers that know the origin (today, only
/// `streaming_cmd.rs`'s MCP-discovery loop, which still holds the
/// `McpServerConfig`/remote tool name at registration time) pass it here
/// instead. Absent from the map (or an empty/`None` map, e.g. every existing
/// caller/test predating this node) means `endpoint: None` is written —
/// exactly today's pre-`token-detail-endpoint` behavior — so this parameter
/// is purely additive. See decisions.md's "Ingestion call-site threading"
/// section for why this isn't instead recovered by parsing the registry name.
pub async fn ingest_tool_registry(
    registry: &ToolRegistry,
    store: &dyn SquireStore,
    endpoints: &HashMap<String, ToolEndpoint>,
) {
    for def in registry.definitions() {
        store
            .upsert_token(
                NewTokenSpec {
                    id: tool_token_id(&def.name),
                    token_type: "tool".to_string(),
                    short_desc: def.description.clone(),
                    full_desc: Some(tool_full_desc(&def)),
                    endpoint: endpoints.get(&def.name).cloned(),
                },
                0,
            )
            .await;
    }
}

// ─────────────────────────── User-input chunking (spec §3.1/§4.3/§9.1/§11) ───────────────────────────

/// Soft size cap (characters) above which a paragraph is further split on
/// sentence boundaries. Not a spec-derived value — spec §15's configuration
/// table has no chunk-size constant — chosen as a documented judgment call;
/// see decisions.md's "(2) What 'chunk' means" section for the rationale.
const CHUNK_SOFT_LIMIT_CHARS: usize = 400;

/// Splits `text` into "natural language structure" chunks (spec §4.3's exact
/// phrase; decisions.md's "(2) What 'chunk' means" documents the judgment
/// call this resolves to): first by blank-line paragraph boundaries, then,
/// for any paragraph longer than `CHUNK_SOFT_LIMIT_CHARS`, further split on
/// sentence-ending punctuation (`.`/`!`/`?` followed by whitespace or
/// end-of-string). A short single-paragraph message (the common chat-message
/// case) produces exactly one chunk. Never returns an empty chunk string;
/// returns an empty `Vec` only for whitespace-only/empty input.
pub fn chunk_user_input(text: &str) -> Vec<String> {
    let paragraphs: Vec<&str> = text
        .split("\n\n")
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .collect();

    let mut chunks = Vec::new();
    for para in paragraphs {
        if para.len() <= CHUNK_SOFT_LIMIT_CHARS {
            chunks.push(para.to_string());
            continue;
        }
        chunks.extend(split_into_sentences(para));
    }
    chunks
}

/// Sentence-boundary split used only for paragraphs exceeding
/// `CHUNK_SOFT_LIMIT_CHARS` (see `chunk_user_input`). Splits after a
/// `.`/`!`/`?` that is followed by whitespace or end-of-string. Known,
/// accepted imprecision: does not special-case abbreviations ("Dr.") or
/// decimal numbers ("3.14") — see decisions.md.
fn split_into_sentences(paragraph: &str) -> Vec<String> {
    let mut sentences = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = paragraph.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        current.push(c);
        if matches!(c, '.' | '!' | '?') {
            let next_is_boundary = chars.get(i + 1).map(|n| n.is_whitespace()).unwrap_or(true);
            if next_is_boundary {
                let trimmed = current.trim();
                if !trimmed.is_empty() {
                    sentences.push(trimmed.to_string());
                }
                current = String::new();
            }
        }
        i += 1;
    }
    let trimmed = current.trim();
    if !trimmed.is_empty() {
        sentences.push(trimmed.to_string());
    }
    if sentences.is_empty() {
        vec![paragraph.to_string()]
    } else {
        sentences
    }
}

/// First sentence of `chunk` (spec §3.1's worked example field comment:
/// "short_desc: (first sentence of the chunk)") — up to the first
/// `.`/`!`/`?` followed by whitespace/end-of-string, or a newline, or the
/// whole chunk if none is found.
fn first_sentence(chunk: &str) -> String {
    let bytes_end = chunk
        .char_indices()
        .find(|(idx, c)| {
            if *c == '\n' {
                return true;
            }
            if matches!(c, '.' | '!' | '?') {
                let rest = &chunk[idx + c.len_utf8()..];
                return rest.chars().next().map(|n| n.is_whitespace()).unwrap_or(true);
            }
            false
        })
        .map(|(idx, c)| idx + c.len_utf8());

    match bytes_end {
        Some(end) => chunk[..end].trim().to_string(),
        None => chunk.trim().to_string(),
    }
}

/// Ingests one turn's user input as `USR_T{turn}_{NNN}`-id
/// `system_referential` tokens (spec §3.1/§4.3/§9.1 step 2/§11 — "the
/// user-input auto-chunking gap"). Backend-agnostic: calls only
/// `SquireStore::upsert_token`, matching `ingest_tool_registry`'s shape.
/// `turn` is the current (about-to-open) turn number, encoded directly in
/// each chunk's id and passed as `creation_turn` — see decisions.md's
/// "(3) Token ID scheme" section for why the sequence resets per turn rather
/// than running as a session-lifetime monotonic counter.
///
/// Intended call site: `SquireContextAdapter::build_turn_input`, immediately
/// after reading the latest user message and *before* the bootstrap
/// `explore_memory` call, so a turn's own freshly-chunked input is
/// bootstrap-discoverable within that same turn (spec §9.1's numbered
/// sequence: step 2 chunking precedes step 3 vector search).
pub async fn ingest_user_input_chunks(text: &str, turn: u64, store: &dyn SquireStore) {
    for (i, chunk) in chunk_user_input(text).into_iter().enumerate() {
        let id = format!("USR_T{}_{:03}", turn, i + 1);
        store
            .upsert_token(
                NewTokenSpec {
                    id,
                    token_type: "system_referential".to_string(),
                    short_desc: first_sentence(&chunk),
                    full_desc: Some(chunk),
                    endpoint: None,
                },
                turn,
            )
            .await;
    }
}

// ─────────────────────────── Built-in tools (spec §6) ───────────────────────────

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
            description: "Invoke a tool through the Squire as the sole gateway. token_id must be a discovered tool token.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "token_id": {"type": "string"},
                    "params": {"type": "object"}
                },
                "required": ["token_id", "params"]
            }),
        },
    ]
}

pub struct SquireExploreTool {
    pub store: Arc<dyn SquireStore>,
    pub tool_registry: Arc<ToolRegistry>,
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
            let mut tool_results: Vec<TokenSummary> = self
                .tool_registry
                .definitions()
                .into_iter()
                .filter(|d| {
                    ql.is_empty()
                        || d.name.to_lowercase().contains(&ql)
                        || d.description.to_lowercase().contains(&ql)
                })
                .take(max_results as usize)
                .map(|d| TokenSummary {
                    token_id: d.name.clone(),
                    token_type: "tool".to_string(),
                    score: 1.0,
                    short_desc: d.description.clone(),
                    accumulated_hits: 0,
                    hop_distance: 0,
                    via_token_id: None,
                })
                .collect();
            if resource_type == "tool_skill" {
                let skills = self
                    .store
                    .explore_memory("skill", &query, num_hops, max_results, current_turn)
                    .await;
                tool_results.extend(skills);
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

pub struct SquireTokenToDetailTool {
    pub store: Arc<dyn SquireStore>,
    pub tool_registry: Arc<ToolRegistry>,
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

        if let Some(tool) = self.tool_registry.get(token_id) {
            let output = if detail_level == "full" {
                serde_json::json!({
                    "name": tool.name(),
                    "description": tool.description(),
                    "input_schema": tool.input_schema(),
                })
                .to_string()
            } else {
                tool.description().to_string()
            };
            return ToolResult {
                call_id: call_id.to_string(),
                output,
                is_error: false,
            };
        }

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

pub struct SquireInvokeTool {
    pub tool_registry: Arc<ToolRegistry>,
    /// squire-storage's real token store. Consulted so `invoke` can resolve
    /// tokens the model discovered via `explore(resource_type="tool"/
    /// "tool_skill")` even if they aren't present in `tool_registry` under
    /// that exact name this turn — e.g. an MCP-sourced tool ingested
    /// (`tool-token-ingestion`) in a previous turn whose server isn't
    /// connected/enabled this turn. `tool_registry` remains the
    /// primary/authoritative lookup, tried first, since it's still the
    /// fastest and most current source whenever the tool IS live this turn.
    /// Since `token-detail-endpoint`, a store hit whose `TokenDetail::
    /// endpoint` is `Some(ToolEndpoint::Mcp{..})` is actually dispatched via
    /// `crate::mcp::call_tool` — the same one-off, stateless dispatch
    /// primitive the live registry's own `McpProxyTool::execute` already
    /// uses — rather than only returning a diagnostic. See
    /// `token-detail-endpoint/decisions.md` for the full design and
    /// proportionality assessment.
    pub store: Arc<dyn SquireStore>,
}

#[async_trait]
impl Tool for SquireInvokeTool {
    fn name(&self) -> &str {
        "invoke"
    }
    fn description(&self) -> &str {
        "Invoke a tool through the Squire as the sole gateway."
    }
    fn input_schema(&self) -> Value {
        built_in_tool_definitions()[2].input_schema.clone()
    }
    fn danger(&self) -> ToolDanger {
        // The proxied tool's real danger level isn't known until token_id is
        // read from args, which `danger()` has no access to. Fail safe:
        // every invoke() requires approval until squire-storage's token
        // metadata can carry a per-token danger classification.
        ToolDanger::Destructive
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
        let params = args.get("params").cloned().unwrap_or_else(|| serde_json::json!({}));

        // Registry lookup is primary: it's the fastest, most current source
        // whenever the tool IS live this turn (local + MCP proxies).
        if let Some(tool) = self.tool_registry.get(token_id) {
            return tool.execute(call_id, params).await;
        }

        // Fall back to the store's token_detail so a token the model learned
        // about via `explore(resource_type="tool_skill")` — ingested by
        // `tool-token-ingestion` — doesn't dead-end just because it isn't
        // also a live ToolRegistry entry this turn.
        match self.store.token_detail(token_id).await {
            // token-detail-endpoint: a stored MCP endpoint means this token
            // is genuinely dispatchable purely from persisted metadata —
            // reconnect and forward the call via the same one-off,
            // stateless `crate::mcp::call_tool` primitive McpProxyTool::
            // execute already uses for live MCP tools.
            Some(TokenDetail {
                endpoint: Some(ToolEndpoint::Mcp { server, remote_name }),
                ..
            }) => match crate::mcp::call_tool(server, remote_name, params).await {
                Ok((output, is_error)) => ToolResult {
                    call_id: call_id.to_string(),
                    output,
                    is_error,
                },
                Err(e) => ToolResult {
                    call_id: call_id.to_string(),
                    output: format!(
                        "MCP tool call failed (dispatched from Squire storage; this tool's server was not live in this turn's registry): {}",
                        e
                    ),
                    is_error: true,
                },
            },
            // No stored endpoint: a local-builtin token (never has one, by
            // construction — see ToolEndpoint's doc comment) or an
            // MCP-sourced token ingested before token-detail-endpoint shipped
            // (self-healing: the next per-turn re-ingestion backfills the
            // endpoint if that tool's server is live again).
            Some(detail) => ToolResult {
                call_id: call_id.to_string(),
                output: format!(
                    "Token '{}' is recorded in Squire storage ({}) but has no invocable endpoint bound yet.",
                    token_id, detail.short_desc
                ),
                is_error: true,
            },
            None => ToolResult {
                call_id: call_id.to_string(),
                output: format!("non-invocable token {}", token_id),
                is_error: true,
            },
        }
    }
}

// ─────────────────────────── Adapter ───────────────────────────

const SQUIRE_SYSTEM_PROMPT: &str = r#"You are the Main AI in the Context Squire system. You have no memory between turns other than what the current request provides. Do not assume you remember anything - if it is not in this request, it does not exist in your working context.

You have exactly three built-in tools: explore(resource_type, query, num_hops, max_results), token_to_detail(token_id, detail_level), and invoke(token_id, params). All other capabilities must be discovered via explore(resource_type="tool_skill", ...). You never call external services directly - invoke() is the sole gateway.

Two sigils appear in your output, never visible to the user:
- §!TokenID - inline reference to an existing token, expanded to its short description before display. The token must exist in the store or be defined in this response's new_tokens.
- §^TokenID content §^ - marks a span of your output as a named retrievable memory unit (opened by §^TokenID, closed by bare §^, does not nest). This is the act of memory creation.

Always respond with a single JSON object in exactly this shape (empty fields present as empty string/array, never omitted):
{
  "ask_user": "",
  "content": "",
  "preserve": [],
  "new_tokens": [],
  "relationships": []
}

ask_user: a question for the user. If populated, content must be empty. Ask one focused question you cannot answer yourself via explore()/invoke().
content: your response to the user, may contain §! and §^ markers.
preserve: token IDs to carry forward to next turn's preserved_tokens, bypassing semantic scoring. Underpreserve rather than overpreserve.
new_tokens: definitions for every token you reference via §! that isn't already in the store, and for every §^ span (short_desc required, full_desc optional - the span text is captured automatically).
relationships: directed triples {subject, predicate, object} connecting tokens you create - an unconnected token is nearly unreachable later.

The Squire validates your response and rejects it with a reason if: ask_user and content are both populated; §!TokenID references a token not in the store and not in new_tokens; a §^ span is opened but never closed. On rejection, read the reason, fix only the specific issue, and resubmit."#;

pub struct SquireContextAdapter {
    store: Arc<dyn SquireStore>,
    max_retries: u32,
    retry_count: u32,
}

impl SquireContextAdapter {
    pub fn new(store: Arc<dyn SquireStore>) -> Self {
        Self {
            store,
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
                let short = match self.store.token_detail(&token_id).await {
                    Some(d) => d.short_desc,
                    None => token_id.clone(),
                };
                out.push_str(&short);
                out.push_str(remainder);
            } else {
                out.push('§');
                out.push_str(part);
            }
        }
        out
    }

    /// Records a rejection and decides retry vs. final failure per Q6.
    fn reject(&mut self, messages: &mut Vec<ChatMessage>, failed_content: String, reason: String) -> TurnOutcome {
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
fn classify_rejection_rule(reason: &str) -> String {
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
        ingest_user_input_chunks(&user_text, current_turn, self.store.as_ref()).await;

        let prefetched = self
            .store
            .explore_memory("all", &user_text, 1, 10, current_turn)
            .await;
        let preserved = self.store.preserved_tokens(session.session.id).await;

        let request = serde_json::json!({
            "user_request": user_text,
            "prefetched_tokens": prefetched,
            "preserved_tokens": preserved,
        });

        let messages = vec![
            ChatMessage {
                role: ChatRole::System,
                content: SQUIRE_SYSTEM_PROMPT.to_string(),
                tool_call_id: None,
                tool_calls: None,
                reasoning_content: None,
            },
            ChatMessage {
                role: ChatRole::User,
                content: serde_json::to_string(&request).map_err(|e| e.to_string())?,
                tool_call_id: None,
                tool_calls: None,
                reasoning_content: None,
            },
        ];

        Ok(TurnInput {
            messages,
            tools: built_in_tool_definitions(),
        })
    }

    async fn handle_tool_loop_step(
        &mut self,
        tool_call: &ToolCall,
        result: &ToolResult,
        reasoning: Option<String>,
        messages: &mut Vec<ChatMessage>,
    ) -> Result<(), String> {
        messages.push(ChatMessage {
            role: ChatRole::Assistant,
            content: String::new(),
            tool_call_id: Some(tool_call.id.clone()),
            tool_calls: Some(vec![ToolCall {
                id: tool_call.id.clone(),
                name: tool_call.name.clone(),
                arguments: tool_call.arguments.clone(),
            }]),
            reasoning_content: reasoning,
        });

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
        let parsed: SquireResponse = match serde_json::from_str(assistant_content.trim()) {
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
            if token.full_desc.is_none() {
                if let Some((_, span_text)) = spans.iter().find(|(id, _)| id == &token.id) {
                    token.full_desc = Some(span_text.clone());
                }
            }
            self.store.upsert_token(token, turn).await;
        }
        for rel in &parsed.relationships {
            self.store.insert_relationship(rel.clone()).await;
        }
        self.store
            .set_preserve_list(session_id, parsed.preserve.clone())
            .await;
        self.store.increment_turn(session_id).await;

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

#[cfg(test)]
#[path = "squire_test.rs"]
mod tests;
