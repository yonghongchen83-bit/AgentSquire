use async_trait::async_trait;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use super::squire::SquireStore;
use super::{Tool, ToolResult};
use crate::storage::conversation_store::SessionId;

// ─── Slug generation ────────────────────────────────────────────────

/// Derive a `TODO_<slug>` token ID from a title. Auto-deduplicates by
/// appending `-2`, `-3`, etc. when the slug already exists in the store.
async fn todo_slug(title: &str, store: &dyn SquireStore) -> String {
    let slug = slugify(title);
    let base = format!("TODO_{}", slug);
    if !store.token_exists(&base).await {
        return base;
    }
    let mut i = 2;
    loop {
        let candidate = format!("TODO_{}-{}", slug, i);
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
        // skip other chars
    }
    let s = slug.trim_matches('_').to_string();
    if s.is_empty() {
        return "untitled".to_string();
    }
    s
}

// ─── Core helpers (shared by all operations) ────────────────────────

/// Compute the done status for a todo node: `true` if it has a `marked_done`
/// relationship *or* all its subtask children are done.
fn is_todo_done(
    id: &str,
    marked_done: &HashSet<String>,
    outgoing_subtask: &HashMap<String, Vec<String>>,
) -> bool {
    if marked_done.contains(id) {
        return true;
    }
    let Some(children) = outgoing_subtask.get(id) else {
        return false;
    };
    children
        .iter()
        .all(|c| is_todo_done(c, marked_done, outgoing_subtask))
}

/// Recursively check that all descendants are done (used to enforce the
/// children-must-be-done-first constraint when marking a node done).
fn all_descendants_todo_done(
    id: &str,
    marked_done: &HashSet<String>,
    outgoing_subtask: &HashMap<String, Vec<String>>,
) -> bool {
    let Some(children) = outgoing_subtask.get(id) else {
        return true;
    };
    for c in children {
        if !is_todo_done(c, marked_done, outgoing_subtask) {
            return false;
        }
        if !all_descendants_todo_done(c, marked_done, outgoing_subtask) {
            return false;
        }
    }
    true
}

/// Collect all descendant ids (recursive subtask walk).
fn collect_descendant_ids(
    id: &str,
    outgoing_subtask: &HashMap<String, Vec<String>>,
) -> HashSet<String> {
    let mut set = HashSet::new();
    let mut stack = vec![id.to_string()];
    while let Some(cur) = stack.pop() {
        if !set.insert(cur.clone()) {
            continue;
        }
        if let Some(children) = outgoing_subtask.get(&cur) {
            for c in children {
                stack.push(c.clone());
            }
        }
    }
    set
}

/// Build the tree JSON for a list of root IDs, using store data.
fn build_tree_json<'a>(
    ids: &'a [String],
    store: &'a dyn SquireStore,
    outgoing_subtask: &'a HashMap<String, Vec<String>>,
    marked_done: &'a HashSet<String>,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Vec<Value>> + Send + 'a>> {
    Box::pin(async move {
    let mut result = Vec::new();
    for id in ids {
        let detail = store.token_detail(id).await;
        let Some(d) = detail else { continue };
        let done = is_todo_done(id, marked_done, outgoing_subtask);
        let status_str = if done { "done" } else { "in_progress" };
        let children_ids = outgoing_subtask.get(id).cloned().unwrap_or_default();
        let children = build_tree_json(&children_ids, store, outgoing_subtask, marked_done).await;
        result.push(serde_json::json!({
            "id": id,
            "title": d.short_desc,
            "status": status_str,
            "children": children,
        }));
    }
    result
})}

pub struct TodoTreeTool {
    store_path: String,
    store: Option<Arc<dyn SquireStore>>,
    session_id: Option<SessionId>,
}

impl TodoTreeTool {
    /// Build a todo tool scoped to a specific conversation (legacy JSON-file path).
    pub fn for_session(session_id: &str) -> Self {
        Self {
            store_path: todo_store_path(session_id),
            store: None,
            session_id: None,
        }
    }

    /// Build a todo tool pointing at an explicit store path (used by tests).
    pub fn with_store_path(path: impl Into<String>) -> Self {
        Self {
            store_path: path.into(),
            store: None,
            session_id: None,
        }
    }

    /// Build a todo tool backed by the Squire store (token-driven mode).
    pub fn for_store(store: Arc<dyn SquireStore>, session_id: SessionId) -> Self {
        Self {
            store_path: String::new(),
            store: Some(store),
            session_id: Some(session_id),
        }
    }

    fn use_store(&self) -> bool {
        self.store.is_some()
    }
}

impl Default for TodoTreeTool {
    fn default() -> Self {
        Self::for_session("default")
    }
}

// ─── Token-backed helpers ───────────────────────────────────────────

/// Load all relationships and index them for todo-tree operations.
async fn load_todo_indices(
    store: &dyn SquireStore,
) -> (
    Vec<String>,                              // root_ids
    HashMap<String, Vec<String>>,              // outgoing_subtask
    HashMap<String, Vec<String>>,              // incoming_subtask (child→[parents])
    HashSet<String>,                           // marked_done
) {
    let rels = store.get_relationships(None, None, None).await;
    let all_ids = store.list_token_ids().await;

    log::info!(
        "[todo_tree] load_todo_indices: {} rels, {} token IDs, TODO_ tokens: {}",
        rels.len(),
        all_ids.len(),
        all_ids.iter().filter(|id| id.starts_with("TODO_")).count(),
    );

    let mut outgoing_subtask: HashMap<String, Vec<String>> = HashMap::new();
    let mut incoming_subtask: HashMap<String, Vec<String>> = HashMap::new();
    let mut marked_done: HashSet<String> = HashSet::new();

    for rel in &rels {
        if rel.predicate == squire_store::predicates::SUBTASK {
            outgoing_subtask
                .entry(rel.subject.clone())
                .or_default()
                .push(rel.object.clone());
            incoming_subtask
                .entry(rel.object.clone())
                .or_default()
                .push(rel.subject.clone());
        }
        if rel.predicate == squire_store::predicates::MARKED_DONE {
            marked_done.insert(rel.subject.clone());
        }
    }

    let todo_ids: Vec<String> = all_ids
        .into_iter()
        .filter(|id| id.starts_with("TODO_"))
        .collect();

    let root_ids: Vec<String> = todo_ids
        .into_iter()
        .filter(|id| !incoming_subtask.contains_key(id))
        .collect();

    (root_ids, outgoing_subtask, incoming_subtask, marked_done)
}

/// Check that marking `id` as done is valid (all children + descendants done).
fn check_done_constraints(
    id: &str,
    marked_done: &HashSet<String>,
    outgoing_subtask: &HashMap<String, Vec<String>>,
) -> Result<(), String> {
    let Some(children) = outgoing_subtask.get(id) else {
        return Ok(());
    };
    for c in children {
        if !is_todo_done(c, marked_done, outgoing_subtask) {
            return Err(format!(
                "Cannot mark '{}' as done: child '{}' is not done",
                id, c
            ));
        }
    }
    if !all_descendants_todo_done(id, marked_done, outgoing_subtask) {
        return Err(format!(
            "Cannot mark '{}' as done: not all descendants are done",
            id
        ));
    }
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// Legacy JSON-file store (kept for backward compat / tests)
// ═══════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TodoStatus {
    Todo,
    InProgress,
    Done,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct TodoNode {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) status: TodoStatus,
    pub(crate) children: Vec<String>,
    pub(crate) parent: Option<String>,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct TodoStore {
    pub(crate) nodes: HashMap<String, TodoNode>,
    pub(crate) root_items: Vec<String>,
}

impl TodoStore {
    pub(crate) fn load(path: &str) -> Self {
        match crate::fs::ops::read_file(path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    fn save(&self, path: &str) -> Result<(), String> {
        let content =
            serde_json::to_string_pretty(self).map_err(|e| format!("serialization: {}", e))?;
        crate::fs::ops::write_file(path, &content).map_err(|e| format!("write: {}", e))
    }

    fn all_descendants_done(&self, id: &str) -> bool {
        let node = match self.nodes.get(id) {
            Some(n) => n,
            None => return false,
        };
        for child_id in &node.children {
            let child = match self.nodes.get(child_id) {
                Some(c) => c,
                None => continue,
            };
            if child.status != TodoStatus::Done {
                return false;
            }
            if !self.all_descendants_done(child_id) {
                return false;
            }
        }
        true
    }

    fn collect_descendants(&self, id: &str) -> HashSet<String> {
        let mut set = HashSet::new();
        let mut stack = vec![id.to_string()];
        while let Some(current) = stack.pop() {
            if !set.insert(current.clone()) {
                continue;
            }
            if let Some(node) = self.nodes.get(&current) {
                for child in &node.children {
                    stack.push(child.clone());
                }
            }
        }
        set
    }

    pub(crate) fn add_node(
        &mut self,
        id: String,
        title: String,
        parent_id: Option<String>,
    ) -> Result<(), String> {
        let now = chrono::Utc::now().to_rfc3339();

        if self.nodes.contains_key(&id) {
            return Err(format!("Node already exists: {}", id));
        }

        if let Some(ref pid) = parent_id {
            if !self.nodes.contains_key(pid) {
                return Err(format!("Parent node not found: {}", pid));
            }
        }

        let node = TodoNode {
            id: id.clone(),
            title,
            status: TodoStatus::Todo,
            children: Vec::new(),
            parent: parent_id.clone(),
            created_at: now.clone(),
            updated_at: now.clone(),
        };

        if let Some(ref pid) = parent_id {
            if let Some(parent) = self.nodes.get_mut(pid) {
                parent.children.push(id.clone());
                parent.updated_at = now;
            }
        } else {
            self.root_items.push(id.clone());
        }

        self.nodes.insert(id, node);
        Ok(())
    }

    pub(crate) fn update_status(&mut self, id: &str, new_status: TodoStatus) -> Result<(), String> {
        let node = self
            .nodes
            .get(id)
            .ok_or_else(|| format!("Node not found: {}", id))?;

        if new_status == TodoStatus::Done && node.status != TodoStatus::Done {
            for child_id in &node.children {
                let child = self
                    .nodes
                    .get(child_id)
                    .ok_or_else(|| format!("Child node not found: {}", child_id))?;
                if child.status != TodoStatus::Done {
                    return Err(format!(
                        "Cannot mark '{}' as done: child '{}' is {:?}",
                        node.title, child.title, child.status
                    ));
                }
            }
            if !self.all_descendants_done(id) {
                return Err(format!(
                    "Cannot mark '{}' as done: not all descendants are done",
                    node.title
                ));
            }
        }

        let node = self.nodes.get_mut(id).unwrap();
        node.status = new_status;
        node.updated_at = chrono::Utc::now().to_rfc3339();
        Ok(())
    }

    pub(crate) fn build_tree_json(&self, ids: &[String]) -> Vec<Value> {
        ids.iter()
            .filter_map(|id| {
                let node = self.nodes.get(id)?;
                let status_str = match node.status {
                    TodoStatus::Todo => "todo",
                    TodoStatus::InProgress => "in_progress",
                    TodoStatus::Done => "done",
                };
                Some(serde_json::json!({
                    "id": node.id,
                    "title": node.title,
                    "status": status_str,
                    "children": self.build_tree_json(&node.children),
                }))
            })
            .collect()
    }

    pub(crate) fn remove_node(&mut self, id: &str) -> Result<Vec<String>, String> {
        let descendants = self.collect_descendants(id);

        let parent_id = self.nodes.get(id).and_then(|n| n.parent.clone());
        if let Some(ref pid) = parent_id {
            if let Some(parent) = self.nodes.get_mut(pid) {
                parent.children.retain(|c| c != id);
            }
        } else {
            self.root_items.retain(|r| r != id);
        }

        for did in &descendants {
            self.nodes.remove(did);
        }

        Ok(descendants.into_iter().collect())
    }
}

impl Default for TodoStore {
    fn default() -> Self {
        Self {
            nodes: HashMap::new(),
            root_items: Vec::new(),
        }
    }
}

/// Resolve the on-disk store path for a session's todo tree. Rooted under
/// `config_dir()/todo-trees/<session_id>.json` (same app-config root as the DB
/// and provider-wire log) so per-conversation todo state never lands in the
/// user's project/CWD — writing it into the workspace root churned the dev
/// server's file watcher and was scoped as a single global file shared across
/// every session. See `UI_Business_Test` node notes.
pub fn todo_store_path(session_id: &str) -> String {
    crate::state::config::config_dir()
        .join("todo-trees")
        .join(format!("{}.json", session_id))
        .to_string_lossy()
        .into_owned()
}

// ═══════════════════════════════════════════════════════════════════════
// Tool impl
// ═══════════════════════════════════════════════════════════════════════

#[async_trait]
impl Tool for TodoTreeTool {
    fn name(&self) -> &str {
        "todo_tree"
    }

    fn description(&self) -> &str {
        "Manage a hierarchical todo tree. Items can have parent-child relationships. An item can only be marked 'done' when all its children are also 'done'."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": ["create", "bulk", "list", "update", "get", "delete"],
                    "description": "Operation to perform"
                },
                "id": {
                    "type": "string",
                    "description": "Item ID (required for update/get/delete)"
                },
                "title": {
                    "type": "string",
                    "description": "Title of the todo item (required for create)"
                },
                "parent_id": {
                    "type": "string",
                    "description": "Parent item ID to nest under (optional for create)"
                },
                "status": {
                    "type": "string",
                    "enum": ["todo", "in_progress", "done"],
                    "description": "New status (required for update)"
                },
                "operations": {
                    "type": "array",
                    "description": "For bulk: pack multiple single operations in order. Each item supports operation=create|list|update|get|delete with the same fields as single calls.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "operation": {
                                "type": "string",
                                "enum": ["create", "list", "update", "get", "delete"]
                            },
                            "id": { "type": "string" },
                            "title": { "type": "string" },
                            "parent_id": { "type": "string" },
                            "status": {
                                "type": "string",
                                "enum": ["todo", "in_progress", "done"]
                            }
                        },
                        "required": ["operation"]
                    }
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

        if self.use_store() {
            self.execute_token(call_id, operation, &args).await
        } else {
            log::info!("[todo_tree] using LEGACY path for operation={}", operation);
            self.execute_legacy(call_id, operation, &args).await
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Token-driven execution (Squire store)
// ═══════════════════════════════════════════════════════════════════════

impl TodoTreeTool {
    async fn execute_token(&self, call_id: &str, operation: &str, args: &Value) -> ToolResult {
        log::info!("[todo_tree] using TOKEN path for operation={}", operation);
        let store = match self.store.as_ref() {
            Some(s) => s.as_ref(),
            None => {
                return ToolResult {
                    call_id: call_id.to_string(),
                    output: "Internal error: store not available".to_string(),
                    is_error: true,
                }
            }
        };

        match operation {
            "create" => self.token_create(call_id, args, store).await,
            "list" => self.token_list(call_id, store).await,
            "update" => self.token_update(call_id, args, store).await,
            "get" => self.token_get(call_id, args, store).await,
            "delete" => self.token_delete(call_id, args, store).await,
            "bulk" => self.token_bulk(call_id, args, store).await,
            other => ToolResult {
                call_id: call_id.to_string(),
                output: format!(
                    "Unknown operation: {}. Use create, bulk, list, update, get, or delete.",
                    other
                ),
                is_error: true,
            },
        }
    }

    async fn token_create(&self, call_id: &str, args: &Value, store: &dyn SquireStore) -> ToolResult {
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

        let parent_id = args.get("parent_id").and_then(|v| v.as_str());
        let id = todo_slug(title_raw, store).await;

        let turn = self
            .session_id
            .map(|sid| store.current_turn(sid))
            .unwrap_or(Box::pin(std::future::ready(0)))
            .await;

        store
            .upsert_token(
                squire_store::NewTokenSpec {
                    id: id.clone(),
                    token_type: "todo".to_string(),
                    short_desc: title_raw.to_string(),
                    full_desc: None,
                    endpoint: None,
                    ranges: vec![],
                },
                turn,
            )
            .await;

        if let Some(pid) = parent_id {
            store
                .insert_relationship(squire_store::Relationship {
                    subject: pid.to_string(),
                    predicate: squire_store::predicates::SUBTASK.to_string(),
                    object: id.clone(),
                })
                .await;
        }

        ToolResult {
            call_id: call_id.to_string(),
            output: format!("Created todo item: {} (id: {})", title_raw, id),
            is_error: false,
        }
    }

    async fn token_list(&self, call_id: &str, store: &dyn SquireStore) -> ToolResult {
        let (root_ids, outgoing, _, marked_done) = load_todo_indices(store).await;
        log::info!(
            "[todo_tree] token_list: {} roots, outgoing keys: {:?}",
            root_ids.len(),
            outgoing.keys().collect::<Vec<_>>(),
        );
        let items = build_tree_json(&root_ids, store, &outgoing, &marked_done).await;
        let payload = serde_json::json!({
            "_type": "todo_tree",
            "items": items,
        });
        ToolResult {
            call_id: call_id.to_string(),
            output: serde_json::to_string(&payload).unwrap_or_default(),
            is_error: false,
        }
    }

    async fn token_update(&self, call_id: &str, args: &Value, store: &dyn SquireStore) -> ToolResult {
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
        let status_str = match args.get("status").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => {
                return ToolResult {
                    call_id: call_id.to_string(),
                    output: "Missing required argument: status".to_string(),
                    is_error: true,
                }
            }
        };

        let detail = store.token_detail(id).await;
        let title = detail
            .as_ref()
            .map(|d| d.short_desc.clone())
            .unwrap_or_default();

        match status_str {
            "done" => {
                let (_root_ids, outgoing, _incoming, marked_done) =
                    load_todo_indices(store).await;
                if let Err(e) = check_done_constraints(id, &marked_done, &outgoing) {
                    return ToolResult {
                        call_id: call_id.to_string(),
                        output: e,
                        is_error: true,
                    };
                }
                store
                    .insert_relationship(squire_store::Relationship {
                        subject: id.to_string(),
                        predicate: squire_store::predicates::MARKED_DONE.to_string(),
                        object: format!("TRT_{}", id),
                    })
                    .await;
                ToolResult {
                    call_id: call_id.to_string(),
                    output: format!("Updated '{}' ({}) to done", title, id),
                    is_error: false,
                }
            }
            "todo" | "in_progress" => {
                ToolResult {
                    call_id: call_id.to_string(),
                    output: format!(
                        "Status '{}' is computed from the relationship graph. '{}' ({}) is not marked done.",
                        status_str, title, id
                    ),
                    is_error: false,
                }
            }
            _ => ToolResult {
                call_id: call_id.to_string(),
                output: format!(
                    "Invalid status: {}. Use todo, in_progress, or done.",
                    status_str
                ),
                is_error: true,
            },
        }
    }

    async fn token_get(&self, call_id: &str, args: &Value, store: &dyn SquireStore) -> ToolResult {
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

        let detail = store.token_detail(id).await;
        if detail.is_none() {
            return ToolResult {
                call_id: call_id.to_string(),
                output: format!("Node not found: {}", id),
                is_error: true,
            };
        }

        let (_root_ids, outgoing, _incoming, marked_done) = load_todo_indices(store).await;
        let children_ids = outgoing.get(id).cloned().unwrap_or_default();
        let children = build_tree_json(&children_ids, store, &outgoing, &marked_done).await;
        let done = is_todo_done(id, &marked_done, &outgoing);
        let status_str = if done { "done" } else { "in_progress" };

        let payload = serde_json::json!({
            "_type": "todo_tree",
            "items": [{
                "id": id,
                "title": detail.unwrap().short_desc,
                "status": status_str,
                "children": children,
            }],
        });
        ToolResult {
            call_id: call_id.to_string(),
            output: serde_json::to_string(&payload).unwrap_or_default(),
            is_error: false,
        }
    }

    async fn token_delete(&self, call_id: &str, args: &Value, store: &dyn SquireStore) -> ToolResult {
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

        let (_root_ids, outgoing, _incoming, _marked_done) = load_todo_indices(store).await;
        let descendants = collect_descendant_ids(id, &outgoing);

        let all_rels = store.get_relationships(None, None, None).await;
        for rel in &all_rels {
            if rel.predicate == squire_store::predicates::SUBTASK
                && (rel.subject == id
                    || rel.object == id
                    || descendants.contains(&rel.subject)
                    || descendants.contains(&rel.object))
            {
                store
                    .insert_relationship(squire_store::Relationship {
                        subject: rel.subject.clone(),
                        predicate: "deleted".to_string(),
                        object: rel.object.clone(),
                    })
                    .await;
            }
        }

        let count = descendants.len();
        ToolResult {
            call_id: call_id.to_string(),
            output: format!("Deleted '{}' ({}) and {} descendant(s)", id, id, count - 1),
            is_error: false,
        }
    }

    async fn token_bulk(&self, call_id: &str, args: &Value, store: &dyn SquireStore) -> ToolResult {
        let operations = match args.get("operations").and_then(|v| v.as_array()) {
            Some(v) if !v.is_empty() => v,
            _ => {
                return ToolResult {
                    call_id: call_id.to_string(),
                    output: "Missing required argument: operations (non-empty array)".to_string(),
                    is_error: true,
                }
            }
        };

        let mut results: Vec<Value> = Vec::new();

        for (idx, op) in operations.iter().enumerate() {
            let Some(op_name) = op.get("operation").and_then(|v| v.as_str()) else {
                return ToolResult {
                    call_id: call_id.to_string(),
                    output: format!("operations[{}].operation is required", idx),
                    is_error: true,
                };
            };

            match op_name {
                "create" => {
                    let result = self.token_create(call_id, op, store).await;
                    if result.is_error {
                        return result;
                    }
                    results.push(serde_json::json!({
                        "index": idx,
                        "operation": "create",
                        "ok": true,
                    }));
                }
                "list" => {
                    let (root_ids, outgoing, _, marked_done) = load_todo_indices(store).await;
                    let items = build_tree_json(&root_ids, store, &outgoing, &marked_done).await;
                    results.push(serde_json::json!({
                        "index": idx,
                        "operation": "list",
                        "ok": true,
                        "items": items,
                    }));
                }
                "get" => {
                    let Some(id) = op.get("id").and_then(|v| v.as_str()) else {
                        return ToolResult {
                            call_id: call_id.to_string(),
                            output: format!("operations[{}].id is required for get", idx),
                            is_error: true,
                        };
                    };
                    let (_root_ids, outgoing, _incoming, marked_done) =
                        load_todo_indices(store).await;
                    let children_ids = outgoing.get(id).cloned().unwrap_or_default();
                    let children = build_tree_json(&children_ids, store, &outgoing, &marked_done).await;
                    let done = is_todo_done(id, &marked_done, &outgoing);
                    let status_str = if done { "done" } else { "in_progress" };
                    let detail = store.token_detail(id).await;
                    results.push(serde_json::json!({
                        "index": idx,
                        "operation": "get",
                        "ok": true,
                        "item": {
                            "id": id,
                            "title": detail.map(|d| d.short_desc).unwrap_or_default(),
                            "status": status_str,
                            "children": children,
                        }
                    }));
                }
                "update" => {
                    let result = self.token_update(call_id, op, store).await;
                    if result.is_error {
                        return result;
                    }
                    results.push(serde_json::json!({
                        "index": idx,
                        "operation": "update",
                        "ok": true,
                    }));
                }
                "delete" => {
                    let result = self.token_delete(call_id, op, store).await;
                    if result.is_error {
                        return result;
                    }
                    results.push(serde_json::json!({
                        "index": idx,
                        "operation": "delete",
                        "ok": true,
                    }));
                }
                other => {
                    return ToolResult {
                        call_id: call_id.to_string(),
                        output: format!(
                            "operations[{}].operation unsupported: {}. Use create, list, update, get, or delete.",
                            idx, other
                        ),
                        is_error: true,
                    }
                }
            }
        }

        ToolResult {
            call_id: call_id.to_string(),
            output: serde_json::json!({
                "_type": "todo_tree_bulk",
                "results": results,
            })
            .to_string(),
            is_error: false,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Legacy JSON-file execution
// ═══════════════════════════════════════════════════════════════════════

impl TodoTreeTool {
    fn store_path_for(&self, args: &Value) -> String {
        args.get("path")
            .and_then(|v| v.as_str())
            .unwrap_or(self.store_path.as_str())
            .to_string()
    }

    async fn execute_legacy(&self, call_id: &str, operation: &str, args: &Value) -> ToolResult {
        let store_path = self.store_path_for(args);

        match operation {
            "create" => {
                let title = match args.get("title").and_then(|v| v.as_str()) {
                    Some(t) => t.trim(),
                    None => {
                        return ToolResult {
                            call_id: call_id.to_string(),
                            output: "Missing required argument: title".to_string(),
                            is_error: true,
                        }
                    }
                };
                if title.is_empty() {
                    return ToolResult {
                        call_id: call_id.to_string(),
                        output: "Title cannot be empty".to_string(),
                        is_error: true,
                    };
                }
                let parent_id = args.get("parent_id").and_then(|v| v.as_str());

                let mut store = TodoStore::load(&store_path);
                let id = uuid::Uuid::new_v4().to_string();

                if let Err(e) =
                    store.add_node(id.clone(), title.to_string(), parent_id.map(|s| s.to_string()))
                {
                    return ToolResult {
                        call_id: call_id.to_string(),
                        output: e,
                        is_error: true,
                    };
                }
                if let Err(e) = store.save(&store_path) {
                    return ToolResult {
                        call_id: call_id.to_string(),
                        output: format!("Failed to save: {}", e),
                        is_error: true,
                    };
                }
                ToolResult {
                    call_id: call_id.to_string(),
                    output: format!("Created todo item: {} (id: {})", title, id),
                    is_error: false,
                }
            }

            "bulk" => {
                let operations = match args.get("operations").and_then(|v| v.as_array()) {
                    Some(v) if !v.is_empty() => v,
                    _ => {
                        return ToolResult {
                            call_id: call_id.to_string(),
                            output: "Missing required argument: operations (non-empty array)"
                                .to_string(),
                            is_error: true,
                        }
                    }
                };

                let mut store = TodoStore::load(&store_path);
                let mut mutated = false;
                let mut results: Vec<Value> = Vec::new();

                for (idx, op) in operations.iter().enumerate() {
                    let Some(op_name) = op.get("operation").and_then(|v| v.as_str()) else {
                        return ToolResult {
                            call_id: call_id.to_string(),
                            output: format!("operations[{}].operation is required", idx),
                            is_error: true,
                        };
                    };

                    match op_name {
                        "create" => {
                            let Some(title_raw) = op.get("title").and_then(|v| v.as_str()) else {
                                return ToolResult {
                                    call_id: call_id.to_string(),
                                    output: format!(
                                        "operations[{}].title is required for create",
                                        idx
                                    ),
                                    is_error: true,
                                };
                            };
                            let title = title_raw.trim();
                            if title.is_empty() {
                                return ToolResult {
                                    call_id: call_id.to_string(),
                                    output: format!("operations[{}].title cannot be empty", idx),
                                    is_error: true,
                                };
                            }

                            let parent_id = op
                                .get("parent_id")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string());
                            let id = uuid::Uuid::new_v4().to_string();
                            if let Err(e) =
                                store.add_node(id.clone(), title.to_string(), parent_id.clone())
                            {
                                return ToolResult {
                                    call_id: call_id.to_string(),
                                    output: format!(
                                        "bulk failed at operations[{}] create: {}",
                                        idx, e
                                    ),
                                    is_error: true,
                                };
                            }
                            mutated = true;
                            results.push(serde_json::json!({
                                "index": idx,
                                "operation": "create",
                                "ok": true,
                                "id": id,
                                "title": title,
                                "parent_id": parent_id,
                            }));
                        }
                        "list" => {
                            let items = store.build_tree_json(&store.root_items);
                            results.push(serde_json::json!({
                                "index": idx,
                                "operation": "list",
                                "ok": true,
                                "items": items,
                            }));
                        }
                        "get" => {
                            let Some(id) = op.get("id").and_then(|v| v.as_str()) else {
                                return ToolResult {
                                    call_id: call_id.to_string(),
                                    output: format!("operations[{}].id is required for get", idx),
                                    is_error: true,
                                };
                            };
                            let Some(node) = store.nodes.get(id) else {
                                return ToolResult {
                                    call_id: call_id.to_string(),
                                    output: format!(
                                        "bulk failed at operations[{}] get: Node not found: {}",
                                        idx, id
                                    ),
                                    is_error: true,
                                };
                            };
                            let status_str = match node.status {
                                TodoStatus::Todo => "todo",
                                TodoStatus::InProgress => "in_progress",
                                TodoStatus::Done => "done",
                            };
                            let children = store.build_tree_json(&node.children);
                            results.push(serde_json::json!({
                                "index": idx,
                                "operation": "get",
                                "ok": true,
                                "item": {
                                    "id": node.id,
                                    "title": node.title,
                                    "status": status_str,
                                    "children": children,
                                }
                            }));
                        }
                        "update" => {
                            let Some(id) = op.get("id").and_then(|v| v.as_str()) else {
                                return ToolResult {
                                    call_id: call_id.to_string(),
                                    output: format!("operations[{}].id is required for update", idx),
                                    is_error: true,
                                };
                            };
                            let Some(status_raw) = op.get("status").and_then(|v| v.as_str())
                            else {
                                return ToolResult {
                                    call_id: call_id.to_string(),
                                    output: format!(
                                        "operations[{}].status is required for update",
                                        idx
                                    ),
                                    is_error: true,
                                };
                            };
                            let new_status = match status_raw {
                                "todo" => TodoStatus::Todo,
                                "in_progress" => TodoStatus::InProgress,
                                "done" => TodoStatus::Done,
                                _ => {
                                    return ToolResult {
                                        call_id: call_id.to_string(),
                                        output: format!(
                                            "operations[{}].status invalid: {}. Use todo, in_progress, or done.",
                                            idx, status_raw
                                        ),
                                        is_error: true,
                                    }
                                }
                            };
                            if let Err(e) = store.update_status(id, new_status.clone()) {
                                return ToolResult {
                                    call_id: call_id.to_string(),
                                    output: format!(
                                        "bulk failed at operations[{}] update: {}",
                                        idx, e
                                    ),
                                    is_error: true,
                                };
                            }
                            mutated = true;
                            results.push(serde_json::json!({
                                "index": idx,
                                "operation": "update",
                                "ok": true,
                                "id": id,
                                "status": status_raw,
                            }));
                        }
                        "delete" => {
                            let Some(id) = op.get("id").and_then(|v| v.as_str()) else {
                                return ToolResult {
                                    call_id: call_id.to_string(),
                                    output: format!("operations[{}].id is required for delete", idx),
                                    is_error: true,
                                };
                            };
                            let removed = match store.remove_node(id) {
                                Ok(v) => v,
                                Err(e) => {
                                    return ToolResult {
                                        call_id: call_id.to_string(),
                                        output: format!(
                                            "bulk failed at operations[{}] delete: {}",
                                            idx, e
                                        ),
                                        is_error: true,
                                    }
                                }
                            };
                            mutated = true;
                            results.push(serde_json::json!({
                                "index": idx,
                                "operation": "delete",
                                "ok": true,
                                "id": id,
                                "removed_count": removed.len(),
                            }));
                        }
                        other => {
                            return ToolResult {
                                call_id: call_id.to_string(),
                                output: format!(
                                    "operations[{}].operation unsupported: {}. Use create, list, update, get, or delete.",
                                    idx, other
                                ),
                                is_error: true,
                            }
                        }
                    }
                }

                if mutated {
                    if let Err(e) = store.save(&store_path) {
                        return ToolResult {
                            call_id: call_id.to_string(),
                            output: format!("Failed to save: {}", e),
                            is_error: true,
                        };
                    }
                }

                ToolResult {
                    call_id: call_id.to_string(),
                    output: serde_json::json!({
                        "_type": "todo_tree_bulk",
                        "results": results,
                    })
                    .to_string(),
                    is_error: false,
                }
            }

            "list" => {
                let store = TodoStore::load(&store_path);
                let items = store.build_tree_json(&store.root_items);
                let payload = serde_json::json!({
                    "_type": "todo_tree",
                    "items": items,
                });
                ToolResult {
                    call_id: call_id.to_string(),
                    output: serde_json::to_string(&payload).unwrap_or_default(),
                    is_error: false,
                }
            }

            "update" => {
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
                let status_str = match args.get("status").and_then(|v| v.as_str()) {
                    Some(s) => s,
                    None => {
                        return ToolResult {
                            call_id: call_id.to_string(),
                            output: "Missing required argument: status".to_string(),
                            is_error: true,
                        }
                    }
                };
                let new_status = match status_str {
                    "todo" => TodoStatus::Todo,
                    "in_progress" => TodoStatus::InProgress,
                    "done" => TodoStatus::Done,
                    _ => {
                        return ToolResult {
                            call_id: call_id.to_string(),
                            output: format!(
                                "Invalid status: {}. Use todo, in_progress, or done.",
                                status_str
                            ),
                            is_error: true,
                        }
                    }
                };

                let mut store = TodoStore::load(&store_path);
                let node_title = store
                    .nodes
                    .get(id)
                    .map(|n| n.title.clone())
                    .unwrap_or_default();

                if let Err(e) = store.update_status(id, new_status) {
                    return ToolResult {
                        call_id: call_id.to_string(),
                        output: e,
                        is_error: true,
                    };
                }
                if let Err(e) = store.save(&store_path) {
                    return ToolResult {
                        call_id: call_id.to_string(),
                        output: format!("Failed to save: {}", e),
                        is_error: true,
                    };
                }
                ToolResult {
                    call_id: call_id.to_string(),
                    output: format!("Updated '{}' ({}) to {}", node_title, id, status_str),
                    is_error: false,
                }
            }

            "get" => {
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
                let store = TodoStore::load(&store_path);
                let node = match store.nodes.get(id) {
                    Some(n) => n,
                    None => {
                        return ToolResult {
                            call_id: call_id.to_string(),
                            output: format!("Node not found: {}", id),
                            is_error: true,
                        }
                    }
                };
                let status_str = match node.status {
                    TodoStatus::Todo => "todo",
                    TodoStatus::InProgress => "in_progress",
                    TodoStatus::Done => "done",
                };
                let children = store.build_tree_json(&node.children);
                let payload = serde_json::json!({
                    "_type": "todo_tree",
                    "items": [{
                        "id": node.id,
                        "title": node.title,
                        "status": status_str,
                        "children": children,
                    }],
                });
                ToolResult {
                    call_id: call_id.to_string(),
                    output: serde_json::to_string(&payload).unwrap_or_default(),
                    is_error: false,
                }
            }

            "delete" => {
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
                let mut store = TodoStore::load(&store_path);
                let title = store
                    .nodes
                    .get(id)
                    .map(|n| n.title.clone())
                    .unwrap_or_default();
                match store.remove_node(id) {
                    Ok(removed) => {
                        if let Err(e) = store.save(&store_path) {
                            return ToolResult {
                                call_id: call_id.to_string(),
                                output: format!("Failed to save: {}", e),
                                is_error: true,
                            };
                        }
                        let count = removed.len();
                        ToolResult {
                            call_id: call_id.to_string(),
                            output: format!(
                                "Deleted '{}' ({}) and {} descendant(s)",
                                title,
                                id,
                                count - 1
                            ),
                            is_error: false,
                        }
                    }
                    Err(e) => ToolResult {
                        call_id: call_id.to_string(),
                        output: e,
                        is_error: true,
                    },
                }
            }

            other => ToolResult {
                call_id: call_id.to_string(),
                output: format!(
                    "Unknown operation: {}. Use create, bulk, list, update, get, or delete.",
                    other
                ),
                is_error: true,
            },
        }
    }
}
