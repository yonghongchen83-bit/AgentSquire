//! `SquireStore` trait — the storage contract that `LanceDbSquireStore`
//! implements against a real LanceDB backend — plus backend-agnostic
//! graph-traversal/ranking helpers shared by all implementations.

use std::collections::{HashMap, HashSet};

use async_trait::async_trait;

use crate::types::{
    ActiveProcessState, ComplianceFailureRecord, NewTokenSpec, Relationship, TokenDetail,
    TokenRange, TokenSummary, ToolEndpoint,
};
use crate::types::SessionId;

// ─────────────────────────── Storage contract (trait) ───────────────────────────

/// Contract that LanceDB (and in-memory test-double) implementations of
/// Squire storage satisfy. Everything here is scoped to what
/// `SquireContextAdapter` and the built-in tools need to function.
#[async_trait]
pub trait SquireStore: Send + Sync {
    async fn token_exists(&self, token_id: &str) -> bool;
    async fn upsert_token(&self, token: NewTokenSpec, creation_turn: u64, session_id: SessionId);
    async fn insert_relationship(&self, rel: Relationship);
    async fn set_preserve_list(&self, session_id: SessionId, tokens: Vec<String>);
    async fn preserved_tokens(&self, session_id: SessionId) -> Vec<TokenSummary>;
    async fn explore_memory(
        &self,
        resource_type: &str,
        query: &str,
        num_hops: u32,
        max_results: u32,
        current_turn: u64,
        session_id: SessionId,
        // Which embedding vector to search against: `"content"` (default) or `"tag"`.
        // Spec §2/§3 — tag search uses the token's `tags`-derived embedding for
        // cleaner semantic matching of structured, self-describing content.
        vector: &str,
    ) -> Vec<TokenSummary>;
    async fn token_detail(&self, token_id: &str) -> Option<TokenDetail>;
    async fn current_turn(&self, session_id: SessionId) -> u64;
    async fn increment_turn(&self, session_id: SessionId);
    async fn record_hit(&self, token_id: &str);
    async fn record_compliance_failure(&self, record: ComplianceFailureRecord);
    async fn clear_all_preserve_lists(&self);
    async fn record_raw_output(&self, session_id: SessionId, turn: u64, content: String);

    /// Insert a relationship, auto-mirroring `HasParent` ↔ `Contains`.
    /// If the predicate is `HasParent`, a `Contains` edge (subject↔object
    /// swapped) is inserted automatically. `Contains` edges must not be
    /// inserted directly — this method ignores them.
    async fn add_relationship(&self, rel: Relationship);

    /// Get all children of a token via `HasParent` edges.
    async fn get_children(&self, token_id: &str) -> Vec<TokenSummary>;

    /// Get the parent of a token via `HasParent` edge (if any).
    async fn get_parent(&self, token_id: &str) -> Option<TokenSummary>;

    /// Walk up the `HasParent` chain up to `max_depth` steps.
    /// Returns ancestors closest to the token first (parent, grandparent, …).
    async fn get_ancestors(&self, token_id: &str, max_depth: u32) -> Vec<TokenSummary>;

    /// Get the `token_type` string for a token.
    async fn get_type(&self, token_id: &str) -> Option<String>;

    /// Find all tokens whose `token_type` matches the given type string.
    async fn get_instances(&self, token_type: &str) -> Vec<TokenSummary>;

    /// Walk up the `HasParent` chain to find the root ancestor.
    /// Useful for todo/decision tree root discovery.
    async fn get_root(&self, token_id: &str) -> Option<TokenSummary>;

    /// List all token IDs in the store.
    async fn list_token_ids(&self) -> Vec<String>;

    /// List token IDs filtered by session. Default impl returns all IDs
    /// (backward-compatible for test doubles). Production impls should
    /// return only tokens whose `session_id` matches the given session
    /// **or** `SessionId::nil()` (global).
    async fn list_token_ids_by_session(&self, session_id: SessionId) -> Vec<String> {
        let _ = session_id;
        self.list_token_ids().await
    }

    /// Query relationships with optional filtering by subject, predicate,
    /// and/or object. Any `None` filter matches all values.
    async fn get_relationships(
        &self,
        subject: Option<&str>,
        predicate: Option<&str>,
        object: Option<&str>,
    ) -> Vec<Relationship>;

    /// Compute the active process state for Squire bootstrap injection.
    /// Scans TODO_ and CONCEPT_DT_ tokens, walks their relationship trees,
    /// and returns the current frontier of incomplete work and the most
    /// recent unresolved decision.
    ///
    /// Default implementation uses `list_token_ids` + `get_relationships`.
    /// Implementations only override for custom behaviour.
    async fn compute_active_process_state(&self, session_id: SessionId) -> ActiveProcessState {
        let all_rels = self.get_relationships(None, None, None).await;
        let all_ids = self.list_token_ids_by_session(session_id).await;

        // --- index relationships ---
        let mut outgoing_subtask: HashMap<String, Vec<String>> = HashMap::new();
        let mut incoming_dt_edge: HashSet<String> = HashSet::new();
        let mut marked_done: HashSet<String> = HashSet::new();
        let mut resolved_set: HashSet<String> = HashSet::new();

        for rel in &all_rels {
            if rel.predicate == crate::types::predicates::SUBTASK {
                outgoing_subtask
                    .entry(rel.subject.clone())
                    .or_default()
                    .push(rel.object.clone());
            }
            if rel.predicate == crate::types::predicates::CONSIDERS
                || rel.predicate == crate::types::predicates::SELECTS
            {
                incoming_dt_edge.insert(rel.object.clone());
            }
            if rel.predicate == crate::types::predicates::MARKED_DONE {
                marked_done.insert(rel.subject.clone());
            }
            if rel.predicate == crate::types::predicates::RESOLVES {
                resolved_set.insert(rel.subject.clone());
            }
        }

        // filter token ids by prefix
        let todo_ids: Vec<&str> = all_ids
            .iter()
            .map(|s| s.as_str())
            .filter(|id| id.starts_with("TODO_"))
            .collect();
        let dt_ids: Vec<&str> = all_ids
            .iter()
            .map(|s| s.as_str())
            .filter(|id| id.starts_with("CONCEPT_DT_"))
            .collect();

        // ---- helper: is a TODO node done? ----
        fn is_done(
            id: &str,
            outgoing: &HashMap<String, Vec<String>>,
            done_set: &HashSet<String>,
        ) -> bool {
            if done_set.contains(id) {
                return true;
            }
            let Some(children) = outgoing.get(id) else {
                return false;
            };
            children.iter().all(|c| is_done(c, outgoing, done_set))
        }

        // ---- helper: collect open leaves (incomplete frontier) ----
        fn open_leaves<'a>(
            id: &'a str,
            outgoing: &'a HashMap<String, Vec<String>>,
            done_set: &HashSet<String>,
            result: &mut Vec<&'a str>,
        ) {
            if done_set.contains(id) {
                return;
            }
            let Some(children) = outgoing.get(id) else {
                result.push(id);
                return;
            };
            if children.is_empty() {
                result.push(id);
                return;
            }
            for c in children {
                open_leaves(c, outgoing, done_set, result);
            }
        }

        // ---- TODO root discovery ----
        let todo_roots: Vec<&str> = todo_ids
            .iter()
            .copied()
            .filter(|id| {
                // root = no incoming subtask edge
                !all_rels.iter().any(|r| {
                    r.predicate == crate::types::predicates::SUBTASK && r.object == *id
                })
            })
            .collect();

        let mut todo_root: Option<String> = None;
        let mut open_leaves_out: Vec<String> = Vec::new();

        for root in &todo_roots {
            if !is_done(root, &outgoing_subtask, &marked_done) {
                todo_root = Some((*root).to_string());
                let mut leaves: Vec<&str> = Vec::new();
                open_leaves(root, &outgoing_subtask, &marked_done, &mut leaves);
                open_leaves_out = leaves.into_iter().map(|s| s.to_string()).collect();
                break;
            }
        }

        // ---- DT root discovery ----
        let dt_roots: Vec<&str> = dt_ids
            .iter()
            .copied()
            .filter(|id| !incoming_dt_edge.contains::<str>(id))
            .collect();

        let mut dt_root: Option<String> = None;
        let mut last_decision: Option<crate::types::LastDecision> = None;

        for root in &dt_roots {
            if resolved_set.contains::<str>(root) {
                continue;
            }
            dt_root = Some((*root).to_string());

            // Walk the decision tree to find the deepest selected node.
            let mut current = *root;
            loop {
                let next: Vec<&str> = all_rels
                    .iter()
                    .filter(|r| {
                        r.subject == current
                            && r.predicate == crate::types::predicates::SELECTS
                    })
                    .map(|r| r.object.as_str())
                    .collect();
                if next.is_empty() {
                    break;
                }
                current = next[0];
            }

            // Determine status of leaf
            let status = if all_rels.iter().any(|r| {
                r.subject == current && r.predicate == crate::types::predicates::CONFIRMED_BY
            }) {
                "confirmed".to_string()
            } else if all_rels.iter().any(|r| {
                r.subject == current && r.predicate == crate::types::predicates::INVALIDATED_BY
            }) {
                "invalidated".to_string()
            } else if all_rels
                .iter()
                .any(|r| r.subject == current && r.predicate == crate::types::predicates::ABANDONED)
            {
                "abandoned".to_string()
            } else {
                "under_consideration".to_string()
            };

            // Find associated assumption (if any)
            let assumption_id: String = all_rels
                .iter()
                .find(|r| {
                    r.predicate == crate::types::predicates::DRIVEN_BY
                        && r.object.starts_with("CONCEPT_Assumption_")
                })
                .map(|r| r.object.clone())
                .unwrap_or_default();

            last_decision = Some(crate::types::LastDecision {
                node: current.to_string(),
                assumption: assumption_id,
                status,
            });
            break;
        }

        ActiveProcessState {
            todo_root,
            open_leaves: open_leaves_out,
            dt_root,
            last_decision,
        }
    }
}

// ─────────────────────────── Helpers ───────────────────────────

#[derive(Default)]
pub struct StoredToken {
    pub token_type: String,
    pub short_desc: String,
    pub full_desc: Option<String>,
    pub creation_turn: u64,
    pub accumulated_hits: u64,
    pub endpoint: Option<ToolEndpoint>,
    pub ranges: Vec<TokenRange>,
    pub session_id: SessionId,
    /// Free-text, author-curated keywords (spec §2).
    pub tags: Vec<String>,
    /// Structured key/value metadata (spec §2).
    pub properties: std::collections::HashMap<String, String>,
}

/// `effective_priority = accumulated_hits - (current_turn - creation_turn)`
/// (spec §3.3, implemented literally as signed arithmetic since the result
/// is explicitly allowed to go negative for stale, never-referenced tokens).
pub fn effective_priority(accumulated_hits: u64, current_turn: u64, creation_turn: u64) -> i64 {
    accumulated_hits as i64 - (current_turn as i64 - creation_turn as i64)
}

/// Sort a candidate list by score descending (spec §6.1), breaking
/// near-ties (within `SCORE_TIE_EPSILON`) by `effective_priority` descending.
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
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        }
    });
}

/// One node's minimal traversal-relevant shape, backend-agnostic.
pub struct TraversalNode {
    pub token_id: String,
    pub token_type: String,
    pub short_desc: String,
    pub tags: Vec<String>,
    pub properties: std::collections::HashMap<String, String>,
}

/// Backend-agnostic BFS: given the directly-matched hop-0 tokens (with their
/// query-similarity scores) and the full relationship edge list, walks the
/// graph outward up to `num_hops` hops and returns newly-discovered tokens.
pub fn traverse_relationships(
    direct: &[(String, f32)],
    edges: &[(String, String)],
    num_hops: u32,
    all_nodes: &HashMap<String, TraversalNode>,
    type_matches: impl Fn(&str) -> bool,
) -> Vec<TokenSummary> {
    if num_hops == 0 || direct.is_empty() {
        return Vec::new();
    }

    let mut adjacency: HashMap<&str, Vec<&str>> = HashMap::new();
    for (s, o) in edges {
        adjacency.entry(s.as_str()).or_default().push(o.as_str());
        adjacency.entry(o.as_str()).or_default().push(s.as_str());
    }

    let direct_ids: HashSet<&str> = direct.iter().map(|(id, _)| id.as_str()).collect();
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
                    continue;
                }
                if discovered.contains_key(neighbor) {
                    continue;
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
                tags: node.tags.clone(),
                properties: node.properties.clone(),
            })
        })
        .collect()
}
