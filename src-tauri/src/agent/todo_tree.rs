use async_trait::async_trait;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

use super::{Tool, ToolResult};

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

pub struct TodoTreeTool {
    /// Absolute path to this session's todo-tree JSON store.
    store_path: String,
}

impl TodoTreeTool {
    /// Build a todo tool scoped to a specific conversation.
    pub fn for_session(session_id: &str) -> Self {
        Self {
            store_path: todo_store_path(session_id),
        }
    }

    /// Build a todo tool pointing at an explicit store path (used by tests).
    pub fn with_store_path(path: impl Into<String>) -> Self {
        Self {
            store_path: path.into(),
        }
    }
}

impl Default for TodoTreeTool {
    /// No session context (e.g. `list_available_tools`, which only reads tool
    /// definitions and never executes). Still kept off the CWD.
    fn default() -> Self {
        Self::for_session("default")
    }
}

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

        // The session-scoped store path is fixed by construction; an explicit
        // `path` arg is honored only as a test/override hook (not advertised in
        // the schema, so the model can never redirect where this is written).
        let store_path = args
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or(self.store_path.as_str());

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

                let mut store = TodoStore::load(store_path);
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
                if let Err(e) = store.save(store_path) {
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

                let mut store = TodoStore::load(store_path);
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
                    if let Err(e) = store.save(store_path) {
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
                let store = TodoStore::load(store_path);
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

                let mut store = TodoStore::load(store_path);
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
                if let Err(e) = store.save(store_path) {
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
                let store = TodoStore::load(store_path);
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
                let mut store = TodoStore::load(store_path);
                let title = store
                    .nodes
                    .get(id)
                    .map(|n| n.title.clone())
                    .unwrap_or_default();
                match store.remove_node(id) {
                    Ok(removed) => {
                        if let Err(e) = store.save(store_path) {
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
