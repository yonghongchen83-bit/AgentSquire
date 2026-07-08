//! `InMemorySquireStore` test double for the `SquireStore` trait.
//!
//! The trait itself and the backend-agnostic helpers are now in the
//! `squire-store` crate. This module only keeps the in-memory
//! implementation used in tests and development.

use std::collections::HashMap;
use tokio::sync::Mutex;

use async_trait::async_trait;

use squire_store::{
    effective_priority, sort_by_score_then_priority, predicates, ComplianceFailureRecord,
    NewTokenSpec, RawPartitionRecord, Relationship, SquireStore, StoredToken, TokenDetail,
    TokenSummary, TraversalNode, traverse_relationships,
};
use crate::storage::conversation_store::SessionId;

// ═══════════════════════════════════════════════════════════════════════
// In-memory store (test double)
// ═══════════════════════════════════════════════════════════════════════

/// Non-persistent stand-in for the LanceDB-backed store. State lives only
/// for the lifetime of the process.
#[derive(Default)]
pub struct InMemorySquireStore {
    pub(crate) tokens: Mutex<HashMap<String, StoredToken>>,
    pub(crate) relationships: Mutex<Vec<Relationship>>,
    pub(crate) preserve_lists: Mutex<HashMap<SessionId, Vec<String>>>,
    pub(crate) turns: Mutex<HashMap<SessionId, u64>>,
    pub(crate) compliance_failures: Mutex<Vec<ComplianceFailureRecord>>,
    pub(crate) raw_partition: Mutex<Vec<RawPartitionRecord>>,
}

// ═══════════════════════════════════════════════════════════════════════
// In-memory store (test double)
// ═══════════════════════════════════════════════════════════════════════

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

    async fn upsert_token(&self, token: NewTokenSpec, creation_turn: u64, session_id: SessionId) {
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
                ranges: token.ranges.clone(),
                session_id,
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
        session_id: SessionId,
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

        // Session filter: include tokens whose session_id is the current
        // session OR nil (global). Tokens from OTHER sessions are excluded.
        let nil = uuid::Uuid::nil();
        let session_matches = |t: &StoredToken| {
            t.session_id == session_id || t.session_id == nil
        };

        let mut direct: Vec<TokenSummary> = tokens
            .iter()
            .filter(|(_, t)| type_matches(&t.token_type))
            .filter(|(_, t)| session_matches(t))
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
                t.accumulated_hits =
                    tokens.get(&t.token_id).map(|s| s.accumulated_hits).unwrap_or(0);
            }
            direct.extend(expanded);
        }

        let priorities: HashMap<String, i64> = direct
            .iter()
            .filter_map(|t| {
                tokens.get(&t.token_id).map(|stored| {
                    (
                        t.token_id.clone(),
                        effective_priority(
                            stored.accumulated_hits,
                            current_turn,
                            stored.creation_turn,
                        ),
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
            ranges: t.ranges.clone(),
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

    async fn get_relationships(
        &self,
        subject: Option<&str>,
        predicate: Option<&str>,
        object: Option<&str>,
    ) -> Vec<Relationship> {
        let rels = self.relationships.lock().await;
        rels.iter()
            .filter(|r| {
                subject.map_or(true, |s| r.subject == s)
                    && predicate.map_or(true, |p| r.predicate == p)
                    && object.map_or(true, |o| r.object == o)
            })
            .cloned()
            .collect()
    }

    async fn list_token_ids(&self) -> Vec<String> {
        self.tokens.lock().await.keys().cloned().collect()
    }

    // ── Tree API convenience methods ──

    async fn add_relationship(&self, rel: Relationship) {
        if rel.predicate == predicates::CONTAINS {
            return;
        }
        self.insert_relationship(rel.clone()).await;
        if rel.predicate == predicates::HAS_PARENT {
            self.insert_relationship(Relationship {
                subject: rel.object.clone(),
                predicate: predicates::CONTAINS.to_string(),
                object: rel.subject.clone(),
            })
            .await;
        }
    }

    async fn get_children(&self, token_id: &str) -> Vec<TokenSummary> {
        let rels = self
            .get_relationships(Some(token_id), Some(predicates::HAS_PARENT), None)
            .await;
        let tokens = self.tokens.lock().await;
        rels.iter()
            .filter_map(|rel| {
                tokens.get(&rel.object).map(|t| TokenSummary {
                    token_id: rel.object.clone(),
                    token_type: t.token_type.clone(),
                    score: 0.0,
                    short_desc: t.short_desc.clone(),
                    accumulated_hits: t.accumulated_hits,
                    hop_distance: 0,
                    via_token_id: None,
                })
            })
            .collect()
    }

    async fn get_parent(&self, token_id: &str) -> Option<TokenSummary> {
        let rels = self
            .get_relationships(None, Some(predicates::HAS_PARENT), Some(token_id))
            .await;
        let parent_id = rels.first()?.subject.clone();
        let tokens = self.tokens.lock().await;
        tokens.get(&parent_id).map(|t| TokenSummary {
            token_id: parent_id,
            token_type: t.token_type.clone(),
            score: 0.0,
            short_desc: t.short_desc.clone(),
            accumulated_hits: t.accumulated_hits,
            hop_distance: 0,
            via_token_id: None,
        })
    }

    async fn get_ancestors(&self, token_id: &str, max_depth: u32) -> Vec<TokenSummary> {
        let mut ancestors = Vec::new();
        let mut current = token_id.to_string();
        for _ in 0..max_depth {
            let rels = self
                .get_relationships(None, Some(predicates::HAS_PARENT), Some(&current))
                .await;
            let Some(parent_rel) = rels.first() else {
                break;
            };
            current = parent_rel.subject.clone();
            let tokens = self.tokens.lock().await;
            if let Some(t) = tokens.get(&current) {
                ancestors.push(TokenSummary {
                    token_id: current.clone(),
                    token_type: t.token_type.clone(),
                    score: 0.0,
                    short_desc: t.short_desc.clone(),
                    accumulated_hits: t.accumulated_hits,
                    hop_distance: 0,
                    via_token_id: None,
                });
            }
        }
        ancestors
    }

    async fn get_type(&self, token_id: &str) -> Option<String> {
        self.tokens
            .lock()
            .await
            .get(token_id)
            .map(|t| t.token_type.clone())
    }

    async fn get_instances(&self, token_type: &str) -> Vec<TokenSummary> {
        let tokens = self.tokens.lock().await;
        tokens
            .iter()
            .filter(|(_, t)| t.token_type == token_type)
            .map(|(id, t)| TokenSummary {
                token_id: id.clone(),
                token_type: t.token_type.clone(),
                score: 0.0,
                short_desc: t.short_desc.clone(),
                accumulated_hits: t.accumulated_hits,
                hop_distance: 0,
                via_token_id: None,
            })
            .collect()
    }

    async fn get_root(&self, token_id: &str) -> Option<TokenSummary> {
        let mut current = token_id.to_string();
        loop {
            let rels = self
                .get_relationships(None, Some(predicates::HAS_PARENT), Some(&current))
                .await;
            let Some(parent_rel) = rels.first() else {
                let tokens = self.tokens.lock().await;
                return tokens.get(&current).map(|t| TokenSummary {
                    token_id: current,
                    token_type: t.token_type.clone(),
                    score: 0.0,
                    short_desc: t.short_desc.clone(),
                    accumulated_hits: t.accumulated_hits,
                    hop_distance: 0,
                    via_token_id: None,
                });
            };
            current = parent_rel.subject.clone();
        }
    }
}
