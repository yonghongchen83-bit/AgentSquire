//! `DecisionTreeTool` — manages decision trees using the Squire token store.
//!
//! Each decision is a `CONCEPT_DT_<slug>` token. Options within a tree are
//! connected via `considers` / `selects` relationships:
//!
//! - **Root** → `CONCEPT_DT_<slug>` — a decision that needs to be made.
//! - **Options** → `CONCEPT_DT_<slug>` — connected via `considers` from the root.
//! - **Active path** → `selects` — the currently chosen option.
//! - **Evidence** → `considers`/`selects` → `CONCEPT_Assumption_<slug>` (via `drivenBy`)
//! - **Validation** → assumption `confirmedBy` / `invalidatedBy` evidence tokens.
//! - **Resolution** → `resolves` — marks the leaf that solved the original problem.
//!
//! Status is **fully computed** — no stored status field.

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashSet;
use std::sync::Arc;

use super::squire::SquireStore;
use super::{Tool, ToolResult};
use crate::storage::conversation_store::SessionId;

// ─── Slug derivation ────────────────────────────────────────────────

/// Generate `CONCEPT_DT_<slug>`, auto-deduplicating with `-2`, `-3`, ...
async fn dt_slug(title: &str, store: &dyn SquireStore) -> String {
    let slug = slugify(title);
    let base = format!("CONCEPT_DT_{}", slug);
    if !store.token_exists(&base).await {
        return base;
    }
    let mut i = 2;
    loop {
        let candidate = format!("CONCEPT_DT_{}-{}", slug, i);
        if !store.token_exists(&candidate).await {
            return candidate;
        }
        i += 1;
    }
}

/// Generate `CONCEPT_Assumption_<slug>`, auto-deduplicating.
async fn assumption_slug(title: &str, store: &dyn SquireStore) -> String {
    let slug = slugify(title);
    let base = format!("CONCEPT_Assumption_{}", slug);
    if !store.token_exists(&base).await {
        return base;
    }
    let mut i = 2;
    loop {
        let candidate = format!("CONCEPT_Assumption_{}-{}", slug, i);
        if !store.token_exists(&candidate).await {
            return candidate;
        }
        i += 1;
    }
}

fn slugify(title: &str) -> String {
    let mut slug = String::with_capacity(title.len());
    for ch in title.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            slug.push(ch);
        } else if ch.is_ascii_whitespace() || ch == '-' {
            slug.push('_');
        }
    }
    let s = slug.trim_matches('_').to_string();
    if s.is_empty() {
        "untitled".to_string()
    } else {
        s
    }
}

// ─── Tree-building helpers ──────────────────────────────────────────

/// Load all DT-related relationships into indexed sets for fast traversal.
async fn load_dt_indices(
    store: &dyn SquireStore,
) -> (
    Vec<String>,                // all DT root IDs (no incoming considers/selects)
    Vec<String>,                // all DT token IDs
    HashSet<String>,            // subjects with considers
    HashSet<String>,            // subjects with selects
    HashSet<String>,            // subjects with confirmedBy
    HashSet<String>,            // subjects with invalidatedBy
    HashSet<String>,            // subjects with abandoned
    HashSet<String>,            // subjects with resolves
    Vec<String>,                // subjects with drivenBy → assumption mapping
) {
    let rels = store.get_relationships(None, None, None).await;
    let all_ids = store.list_token_ids().await;

    use squire_store::predicates;

    let dt_token_ids: Vec<String> = all_ids
        .into_iter()
        .filter(|id| id.starts_with("CONCEPT_DT_"))
        .collect();

    let mut incoming: HashSet<String> = HashSet::new();
    let mut considers_set: HashSet<String> = HashSet::new();
    let mut selects_set: HashSet<String> = HashSet::new();
    let mut confirmed_set: HashSet<String> = HashSet::new();
    let mut invalidated_set: HashSet<String> = HashSet::new();
    let mut abandoned_set: HashSet<String> = HashSet::new();
    let mut resolves_set: HashSet<String> = HashSet::new();
    let mut driven_by_assumptions: Vec<String> = Vec::new();

    for rel in &rels {
        match rel.predicate.as_str() {
            predicates::CONSIDERS | predicates::SELECTS => {
                incoming.insert(rel.object.clone());
                if rel.predicate == predicates::CONSIDERS {
                    considers_set.insert(rel.subject.clone());
                } else {
                    selects_set.insert(rel.subject.clone());
                }
            }
            predicates::CONFIRMED_BY => {
                confirmed_set.insert(rel.subject.clone());
            }
            predicates::INVALIDATED_BY => {
                invalidated_set.insert(rel.subject.clone());
            }
            predicates::ABANDONED => {
                abandoned_set.insert(rel.subject.clone());
            }
            predicates::RESOLVES => {
                resolves_set.insert(rel.subject.clone());
            }
            _ => {}
        }
        if rel.predicate == predicates::DRIVEN_BY {
            driven_by_assumptions.push(rel.subject.clone());
        }
    }

    let roots: Vec<String> = dt_token_ids
        .iter()
        .filter(|id| !incoming.contains(*id))
        .cloned()
        .collect();

    (
        roots,
        dt_token_ids,
        considers_set,
        selects_set,
        confirmed_set,
        invalidated_set,
        abandoned_set,
        resolves_set,
        driven_by_assumptions,
    )
}

/// Get the children for a DT node (via considers + selects).
fn dt_children(
    parent: &str,
    rels: &[squire_store::Relationship],
) -> Vec<String> {
    rels.iter()
        .filter(|r| {
            r.subject == parent
                && (r.predicate == squire_store::predicates::CONSIDERS
                    || r.predicate == squire_store::predicates::SELECTS)
        })
        .map(|r| r.object.clone())
        .collect()
}

/// Build tree JSON for a DT node recursively.
fn dt_build_tree<'a>(
    id: &'a str,
    rels: &'a [squire_store::Relationship],
    store: &'a dyn SquireStore,
    selects_set: &'a HashSet<String>,
    confirmed_set: &'a HashSet<String>,
    invalidated_set: &'a HashSet<String>,
    abandoned_set: &'a HashSet<String>,
    resolves_set: &'a HashSet<String>,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Value> + Send + 'a>> {
    Box::pin(async move {
        let detail = store.token_detail(id).await;
        let title = detail
            .map(|d| d.short_desc)
            .unwrap_or_else(|| id.to_string());

        let children = dt_children(id, rels);
        let is_active = selects_set.contains(id);

        let status = if resolves_set.contains(id) {
            "resolved"
        } else if confirmed_set.contains(id) {
            "confirmed"
        } else if invalidated_set.contains(id) {
            "invalidated"
        } else if abandoned_set.contains(id) {
            "abandoned"
        } else if is_active {
            "active"
        } else {
            "considered"
        };

        let mut child_values = Vec::new();
        for c in children {
            child_values.push(
                dt_build_tree(
                    &c, rels, store, selects_set, confirmed_set,
                    invalidated_set, abandoned_set, resolves_set,
                )
                .await,
            );
        }

        serde_json::json!({
            "id": id,
            "title": title,
            "status": status,
            "children": child_values,
        })
    })
}

// ─── DecisionTreeTool ───────────────────────────────────────────────

pub struct DecisionTreeTool {
    store: Arc<dyn SquireStore>,
    session_id: SessionId,
}

impl DecisionTreeTool {
    pub fn new(store: Arc<dyn SquireStore>, session_id: SessionId) -> Self {
        Self { store, session_id }
    }
}

#[async_trait]
impl Tool for DecisionTreeTool {
    fn name(&self) -> &str {
        "decision_tree"
    }

    fn description(&self) -> &str {
        "Manage decision trees for structured reasoning. Supports: create_decision, consider, select, confirm, invalidate, abandon, resolve, list, get."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": [
                        "create_decision", "consider", "select", "confirm",
                        "invalidate", "abandon", "resolve", "list", "get"
                    ],
                    "description": "Operation to perform"
                },
                "id": {
                    "type": "string",
                    "description": "Decision/option ID (required for most operations)"
                },
                "title": {
                    "type": "string",
                    "description": "Title for the decision or option (required for create_decision, consider)"
                },
                "parent_id": {
                    "type": "string",
                    "description": "Parent decision ID to nest under (required for consider)"
                },
                "assumption": {
                    "type": "string",
                    "description": "Assumption text (optional for select, required for confirm/invalidate)"
                },
                "evidence": {
                    "type": "string",
                    "description": "Evidence description (optional for confirm/invalidate)"
                }
            },
            "required": ["operation"]
        })
    }

    async fn execute(&self, call_id: &str, args: Value) -> ToolResult {
        let operation = match args.get("operation").and_then(|v| v.as_str()) {
            Some(op) => op,
            None => {
                return ToolResult {
                    call_id: call_id.to_string(),
                    output: "Missing required argument: operation".to_string(),
                    is_error: true,
                }
            }
        };

        match operation {
            "create_decision" => self.op_create_decision(call_id, &args).await,
            "consider" => self.op_consider(call_id, &args).await,
            "select" => self.op_select(call_id, &args).await,
            "confirm" => self.op_confirm(call_id, &args).await,
            "invalidate" => self.op_invalidate(call_id, &args).await,
            "abandon" => self.op_abandon(call_id, &args).await,
            "resolve" => self.op_resolve(call_id, &args).await,
            "list" => self.op_list(call_id).await,
            "get" => self.op_get(call_id, &args).await,
            other => ToolResult {
                call_id: call_id.to_string(),
                output: format!(
                    "Unknown operation: {}. Use create_decision, consider, select, confirm, invalidate, abandon, resolve, list, or get.",
                    other
                ),
                is_error: true,
            },
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Operations
// ═══════════════════════════════════════════════════════════════════════

impl DecisionTreeTool {
    /// Create a new decision tree root: `CONCEPT_DT_<slug>` token.
    async fn op_create_decision(&self, call_id: &str, args: &Value) -> ToolResult {
        let title_raw = match args.get("title").and_then(|v| v.as_str()) {
            Some(t) => t.trim(),
            None => {
                return ToolResult {
                    call_id: call_id.to_string(),
                    output: "Missing required argument: title".to_string(),
                    is_error: true,
                }
            }
        };
        if title_raw.is_empty() {
            return ToolResult {
                call_id: call_id.to_string(),
                output: "Title cannot be empty".to_string(),
                is_error: true,
            };
        }

        let id = dt_slug(title_raw, self.store.as_ref()).await;
        let turn = self.store.current_turn(self.session_id).await;

        self.store
            .upsert_token(
                squire_store::NewTokenSpec {
                    id: id.clone(),
                    token_type: "decision".to_string(),
                    short_desc: title_raw.to_string(),
                    full_desc: None,
                    endpoint: None,
                    ranges: vec![],
                },
                turn,
                self.session_id,
            )
            .await;

        ToolResult {
            call_id: call_id.to_string(),
            output: format!("Created decision: {} (id: {})", title_raw, id),
            is_error: false,
        }
    }

    /// Add an option under a decision: `CONCEPT_DT_<slug>` child via `considers`.
    async fn op_consider(&self, call_id: &str, args: &Value) -> ToolResult {
        let parent_id = match args.get("parent_id").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => {
                return ToolResult {
                    call_id: call_id.to_string(),
                    output: "Missing required argument: parent_id".to_string(),
                    is_error: true,
                }
            }
        };

        let title_raw = match args.get("title").and_then(|v| v.as_str()) {
            Some(t) => t.trim(),
            None => {
                return ToolResult {
                    call_id: call_id.to_string(),
                    output: "Missing required argument: title".to_string(),
                    is_error: true,
                }
            }
        };
        if title_raw.is_empty() {
            return ToolResult {
                call_id: call_id.to_string(),
                output: "Title cannot be empty".to_string(),
                is_error: true,
            };
        }

        let id = dt_slug(title_raw, self.store.as_ref()).await;
        let turn = self.store.current_turn(self.session_id).await;

        self.store
            .upsert_token(
                squire_store::NewTokenSpec {
                    id: id.clone(),
                    token_type: "decision".to_string(),
                    short_desc: title_raw.to_string(),
                    full_desc: None,
                    endpoint: None,
                    ranges: vec![],
                },
                turn,
                self.session_id,
            )
            .await;

        self.store
            .insert_relationship(squire_store::Relationship {
                subject: parent_id.to_string(),
                predicate: squire_store::predicates::CONSIDERS.to_string(),
                object: id.clone(),
            })
            .await;

        ToolResult {
            call_id: call_id.to_string(),
            output: format!("Added option under '{}': {} (id: {})", parent_id, title_raw, id),
            is_error: false,
        }
    }

    /// Select an option as the active path. Optionally link to an assumption.
    async fn op_select(&self, call_id: &str, args: &Value) -> ToolResult {
        let id = match args.get("id").and_then(|v| v.as_str()) {
            Some(i) => i,
            None => {
                return ToolResult {
                    call_id: call_id.to_string(),
                    output: "Missing required argument: id".to_string(),
                    is_error: true,
                }
            }
        };

        // Remove any prior `selects` from the same parent (only one active path).
        // Find the parent via incoming considers/selects.
        let rels = self.store.get_relationships(None, None, None).await;
        let parent: Option<String> = rels.iter().find(|r| {
            r.object == id
                && (r.predicate == squire_store::predicates::CONSIDERS
                    || r.predicate == squire_store::predicates::SELECTS)
        }).map(|r| r.subject.clone());

        self.store
            .insert_relationship(squire_store::Relationship {
                subject: parent.clone().unwrap_or_else(|| id.to_string()),
                predicate: squire_store::predicates::SELECTS.to_string(),
                object: id.to_string(),
            })
            .await;

        // If an assumption was provided, link it.
        if let Some(assumption_text) = args.get("assumption").and_then(|v| v.as_str()) {
            let assumption_text = assumption_text.trim();
            if !assumption_text.is_empty() {
                let ass_id = assumption_slug(assumption_text, self.store.as_ref()).await;
                let turn = self.store.current_turn(self.session_id).await;
                self.store
                    .upsert_token(
                        squire_store::NewTokenSpec {
                            id: ass_id.clone(),
                            token_type: "assumption".to_string(),
                            short_desc: assumption_text.to_string(),
                            full_desc: None,
                            endpoint: None,
                            ranges: vec![],
                        },
                        turn,
                        self.session_id,
                    )
                    .await;
                self.store
                    .insert_relationship(squire_store::Relationship {
                        subject: id.to_string(),
                        predicate: squire_store::predicates::DRIVEN_BY.to_string(),
                        object: ass_id.clone(),
                    })
                    .await;
            }
        }

        ToolResult {
            call_id: call_id.to_string(),
            output: format!("Selected option: {}", id),
            is_error: false,
        }
    }

    /// Confirm an option (the assumption was validated).
    async fn op_confirm(&self, call_id: &str, args: &Value) -> ToolResult {
        let id = match args.get("id").and_then(|v| v.as_str()) {
            Some(i) => i,
            None => {
                return ToolResult {
                    call_id: call_id.to_string(),
                    output: "Missing required argument: id".to_string(),
                    is_error: true,
                }
            }
        };

        let evidence_text = args.get("evidence").and_then(|v| v.as_str()).unwrap_or("confirmed");

        self.store
            .insert_relationship(squire_store::Relationship {
                subject: id.to_string(),
                predicate: squire_store::predicates::CONFIRMED_BY.to_string(),
                object: format!("TRT_evidence_{}", slugify(evidence_text)),
            })
            .await;

        ToolResult {
            call_id: call_id.to_string(),
            output: format!("Confirmed option: {}", id),
            is_error: false,
        }
    }

    /// Invalidate an option (the assumption was disproved).
    async fn op_invalidate(&self, call_id: &str, args: &Value) -> ToolResult {
        let id = match args.get("id").and_then(|v| v.as_str()) {
            Some(i) => i,
            None => {
                return ToolResult {
                    call_id: call_id.to_string(),
                    output: "Missing required argument: id".to_string(),
                    is_error: true,
                }
            }
        };

        let evidence_text = args.get("evidence").and_then(|v| v.as_str()).unwrap_or("invalidated");

        self.store
            .insert_relationship(squire_store::Relationship {
                subject: id.to_string(),
                predicate: squire_store::predicates::INVALIDATED_BY.to_string(),
                object: format!("TRT_evidence_{}", slugify(evidence_text)),
            })
            .await;

        ToolResult {
            call_id: call_id.to_string(),
            output: format!("Invalidated option: {}", id),
            is_error: false,
        }
    }

    /// Abandon an option (no longer being pursued).
    async fn op_abandon(&self, call_id: &str, args: &Value) -> ToolResult {
        let id = match args.get("id").and_then(|v| v.as_str()) {
            Some(i) => i,
            None => {
                return ToolResult {
                    call_id: call_id.to_string(),
                    output: "Missing required argument: id".to_string(),
                    is_error: true,
                }
            }
        };

        let assumption_text = args.get("assumption").and_then(|v| v.as_str()).unwrap_or("");

        self.store
            .insert_relationship(squire_store::Relationship {
                subject: id.to_string(),
                predicate: squire_store::predicates::ABANDONED.to_string(),
                object: if assumption_text.is_empty() {
                    format!("TRT_abandoned_{}", id)
                } else {
                    format!("CONCEPT_Assumption_{}", slugify(assumption_text))
                },
            })
            .await;

        ToolResult {
            call_id: call_id.to_string(),
            output: format!("Abandoned option: {}", id),
            is_error: false,
        }
    }

    /// Mark an option as the resolution.
    async fn op_resolve(&self, call_id: &str, args: &Value) -> ToolResult {
        let id = match args.get("id").and_then(|v| v.as_str()) {
            Some(i) => i,
            None => {
                return ToolResult {
                    call_id: call_id.to_string(),
                    output: "Missing required argument: id".to_string(),
                    is_error: true,
                }
            }
        };

        // Find the root of this tree (walk up via incoming considers/selects).
        let rels = self.store.get_relationships(None, None, None).await;
        let root = find_dt_root(id, &rels);

        self.store
            .insert_relationship(squire_store::Relationship {
                subject: id.to_string(),
                predicate: squire_store::predicates::RESOLVES.to_string(),
                object: root.clone(),
            })
            .await;

        ToolResult {
            call_id: call_id.to_string(),
            output: format!("Resolved decision: {} → {}", root, id),
            is_error: false,
        }
    }

    /// List all decision trees.
    async fn op_list(&self, call_id: &str) -> ToolResult {
        let rels = self.store.get_relationships(None, None, None).await;
        let (roots, _all_ids, _considers_set, selects_set, confirmed_set,
             invalidated_set, abandoned_set, resolves_set, _) =
            load_dt_indices(self.store.as_ref()).await;

        let mut items: Vec<Value> = Vec::new();
        for root in &roots {
            let tree = dt_build_tree(
                root,
                &rels,
                self.store.as_ref(),
                &selects_set,
                &confirmed_set,
                &invalidated_set,
                &abandoned_set,
                &resolves_set,
            )
            .await;
            items.push(tree);
        }

        let payload = serde_json::json!({
            "_type": "decision_tree",
            "items": items,
        });

        ToolResult {
            call_id: call_id.to_string(),
            output: serde_json::to_string(&payload).unwrap_or_default(),
            is_error: false,
        }
    }

    /// Get a single decision tree node and its subtree.
    async fn op_get(&self, call_id: &str, args: &Value) -> ToolResult {
        let id = match args.get("id").and_then(|v| v.as_str()) {
            Some(i) => i,
            None => {
                return ToolResult {
                    call_id: call_id.to_string(),
                    output: "Missing required argument: id".to_string(),
                    is_error: true,
                }
            }
        };

        let detail = self.store.token_detail(id).await;
        if detail.is_none() {
            return ToolResult {
                call_id: call_id.to_string(),
                output: format!("Decision not found: {}", id),
                is_error: true,
            };
        }

        let rels = self.store.get_relationships(None, None, None).await;
        let (_roots, _all_ids, _considers_set, selects_set, confirmed_set,
             invalidated_set, abandoned_set, resolves_set, _) =
            load_dt_indices(self.store.as_ref()).await;

        // Walk up to root so the frontend always gets the complete tree.
        let root = find_dt_root(id, &rels);

        let tree = dt_build_tree(
            &root,
            &rels,
            self.store.as_ref(),
            &selects_set,
            &confirmed_set,
            &invalidated_set,
            &abandoned_set,
            &resolves_set,
        )
        .await;

        let payload = serde_json::json!({
            "_type": "decision_tree",
            "items": [tree],
        });

        ToolResult {
            call_id: call_id.to_string(),
            output: serde_json::to_string(&payload).unwrap_or_default(),
            is_error: false,
        }
    }
}

/// Walk up through `considers`/`selects` from a node to find the tree root.
fn find_dt_root(id: &str, rels: &[squire_store::Relationship]) -> String {
    let mut current = id.to_string();
    loop {
        let parent = rels.iter().find(|r| {
            r.object == current
                && (r.predicate == squire_store::predicates::CONSIDERS
                    || r.predicate == squire_store::predicates::SELECTS)
        });
        match parent {
            Some(p) => current = p.subject.clone(),
            None => return current,
        }
    }
}
