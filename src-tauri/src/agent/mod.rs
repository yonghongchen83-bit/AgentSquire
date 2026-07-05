use async_trait::async_trait;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tauri::Emitter;
use tauri::Manager;

use crate::state::config::McpServerConfig;

pub mod context_adapter;
pub mod squire;
pub mod squire_skills;
pub mod squire_workflows;

pub use crate::llm::provider::ToolCall;
pub use crate::llm::provider::ToolDefinition;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolResult {
    pub call_id: String,
    pub output: String,
    pub is_error: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum ToolDanger {
    Safe,
    Destructive,
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> Value;
    fn danger(&self) -> ToolDanger {
        ToolDanger::Safe
    }
    async fn execute(&self, call_id: &str, args: Value) -> ToolResult;
}

// ── File Read Tool ──

pub struct FileReadTool;

#[async_trait]
impl Tool for FileReadTool {
    fn name(&self) -> &str {
        "read_file"
    }
    fn description(&self) -> &str {
        "Read the contents of a file at the given path"
    }
    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to read"
                }
            },
            "required": ["path"]
        })
    }
    async fn execute(&self, call_id: &str, args: Value) -> ToolResult {
        let path = match args.get("path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => {
                return ToolResult {
                    call_id: call_id.to_string(),
                    output: "Missing required argument: path".to_string(),
                    is_error: true,
                }
            }
        };
        match crate::fs::ops::read_file(path) {
            Ok(content) => ToolResult {
                call_id: call_id.to_string(),
                output: content,
                is_error: false,
            },
            Err(e) => ToolResult {
                call_id: call_id.to_string(),
                output: e.to_string(),
                is_error: true,
            },
        }
    }
}

// ── File Write Tool (destructive) ──

pub struct FileWriteTool;

#[async_trait]
impl Tool for FileWriteTool {
    fn name(&self) -> &str {
        "write_file"
    }
    fn description(&self) -> &str {
        "Write content to a file. Creates parent directories if needed. Requires user approval."
    }
    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to write the file to"
                },
                "content": {
                    "type": "string",
                    "description": "Content to write to the file"
                }
            },
            "required": ["path", "content"]
        })
    }
    fn danger(&self) -> ToolDanger {
        ToolDanger::Destructive
    }
    async fn execute(&self, call_id: &str, args: Value) -> ToolResult {
        let path = match args.get("path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => {
                return ToolResult {
                    call_id: call_id.to_string(),
                    output: "Missing required argument: path".to_string(),
                    is_error: true,
                }
            }
        };
        let content = match args.get("content").and_then(|v| v.as_str()) {
            Some(c) => c,
            None => {
                return ToolResult {
                    call_id: call_id.to_string(),
                    output: "Missing required argument: content".to_string(),
                    is_error: true,
                }
            }
        };
        match crate::fs::ops::write_file(path, content) {
            Ok(()) => ToolResult {
                call_id: call_id.to_string(),
                output: format!("Successfully wrote {} bytes to {}", content.len(), path),
                is_error: false,
            },
            Err(e) => ToolResult {
                call_id: call_id.to_string(),
                output: e.to_string(),
                is_error: true,
            },
        }
    }
}

// ── Code Search Tool ──

pub struct CodeSearchTool;

#[async_trait]
impl Tool for CodeSearchTool {
    fn name(&self) -> &str {
        "search_code"
    }
    fn description(&self) -> &str {
        "Search code using ripgrep. Returns matching lines with file paths and line numbers."
    }
    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search pattern (plain text or regex)"
                },
                "path": {
                    "type": "string",
                    "description": "Directory to search in (default: current directory)"
                },
                "regex": {
                    "type": "boolean",
                    "description": "Treat query as a regular expression"
                },
                "case_sensitive": {
                    "type": "boolean",
                    "description": "Perform case-sensitive search"
                },
                "glob": {
                    "type": "string",
                    "description": "File glob pattern (e.g. *.rs, src/**/*.ts)"
                }
            },
            "required": ["query"]
        })
    }
    async fn execute(&self, call_id: &str, args: Value) -> ToolResult {
        let query = match args.get("query").and_then(|v| v.as_str()) {
            Some(q) => q,
            None => {
                return ToolResult {
                    call_id: call_id.to_string(),
                    output: "Missing required argument: query".to_string(),
                    is_error: true,
                }
            }
        };
        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
        let regex = args.get("regex").and_then(|v| v.as_bool()).unwrap_or(false);
        let case_sensitive = args
            .get("case_sensitive")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let glob = args
            .get("glob")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let options = crate::search::grep::SearchOptions {
            query: query.to_string(),
            path: path.to_string(),
            regex,
            case_sensitive,
            whole_word: false,
            max_results: Some(50),
            glob,
            context_lines: Some(2),
        };

        match crate::search::grep::search(&options) {
            Ok(matches) => {
                if matches.is_empty() {
                    return ToolResult {
                        call_id: call_id.to_string(),
                        output: "No matches found".to_string(),
                        is_error: false,
                    };
                }
                let mut output = String::new();
                for m in &matches {
                    output.push_str(&format!(
                        "{}:{}:{} {}\n",
                        m.file, m.line_number, m.column, m.content
                    ));
                }
                ToolResult {
                    call_id: call_id.to_string(),
                    output,
                    is_error: false,
                }
            }
            Err(e) => ToolResult {
                call_id: call_id.to_string(),
                output: e.to_string(),
                is_error: true,
            },
        }
    }
}

// ── Terminal Tool (destructive) ──

pub struct TerminalTool;

#[async_trait]
impl Tool for TerminalTool {
    fn name(&self) -> &str {
        "run_terminal"
    }
    fn description(&self) -> &str {
        "Execute a shell command. Returns stdout, stderr, and exit code. Requires user approval."
    }
    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "Command to execute (e.g. 'cargo', 'npm', 'python')"
                },
                "args": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "Command arguments"
                },
                "workdir": {
                    "type": "string",
                    "description": "Working directory for the command"
                }
            },
            "required": ["command"]
        })
    }
    fn danger(&self) -> ToolDanger {
        ToolDanger::Destructive
    }
    async fn execute(&self, call_id: &str, args: Value) -> ToolResult {
        let command = match args.get("command").and_then(|v| v.as_str()) {
            Some(c) => c,
            None => {
                return ToolResult {
                    call_id: call_id.to_string(),
                    output: "Missing required argument: command".to_string(),
                    is_error: true,
                }
            }
        };
        let cmd_args: Vec<String> = args
            .get("args")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        let workdir = args.get("workdir").and_then(|v| v.as_str());

        match crate::shell::exec::execute(command, &cmd_args, workdir) {
            Ok(result) => {
                let mut output = String::new();
                if !result.stdout.is_empty() {
                    output.push_str(&result.stdout);
                }
                if !result.stderr.is_empty() {
                    if !output.is_empty() {
                        output.push('\n');
                    }
                    output.push_str(&format!("stderr:\n{}", result.stderr));
                }
                if !result.success {
                    output.push_str(&format!("\n(exit code: {})", result.exit_code));
                }
                if output.is_empty() {
                    output = format!("Command completed with exit code {}", result.exit_code);
                }
                ToolResult {
                    call_id: call_id.to_string(),
                    output,
                    is_error: !result.success,
                }
            }
            Err(e) => ToolResult {
                call_id: call_id.to_string(),
                output: e.to_string(),
                is_error: true,
            },
        }
    }
}

// ── Git Tool ──

pub struct GitTool;

#[async_trait]
impl Tool for GitTool {
    fn name(&self) -> &str {
        "git"
    }
    fn description(&self) -> &str {
        "Run git operations: status, diff, log, branches. Read-only git operations."
    }
    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": ["status", "diff", "log", "branches"],
                    "description": "Git operation to perform"
                },
                "path": {
                    "type": "string",
                    "description": "Repository path (default: current directory)"
                },
                "max_count": {
                    "type": "number",
                    "description": "Max log entries (for log operation, default: 10)"
                },
                "staged": {
                    "type": "boolean",
                    "description": "Show staged diff (for diff operation)"
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
        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");

        let output = match operation {
            "status" => match crate::fs::git::status(path) {
                Ok(entries) => {
                    if entries.is_empty() {
                        "Clean working tree".to_string()
                    } else {
                        let mut out = String::from("Git Status:\n");
                        for e in &entries {
                            out.push_str(&format!("  {}  {}\n", e.status, e.path));
                        }
                        out
                    }
                }
                Err(e) => {
                    return ToolResult {
                        call_id: call_id.to_string(),
                        output: e.to_string(),
                        is_error: true,
                    }
                }
            },
            "diff" => {
                let staged = args
                    .get("staged")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                match crate::fs::git::diff(path, staged) {
                    Ok(diffs) => {
                        if diffs.is_empty() {
                            "No diffs".to_string()
                        } else {
                            let mut out = String::new();
                            for d in &diffs {
                                out.push_str(&format!("--- {}\n{}", d.path, d.diff));
                            }
                            out
                        }
                    }
                    Err(e) => {
                        return ToolResult {
                            call_id: call_id.to_string(),
                            output: e.to_string(),
                            is_error: true,
                        }
                    }
                }
            }
            "log" => {
                let max_count = args.get("max_count").and_then(|v| v.as_i64()).unwrap_or(10) as i32;
                match crate::fs::git::log(path, max_count) {
                    Ok(entries) => {
                        let mut out = String::new();
                        for e in &entries {
                            let short_hash = e.hash.chars().take(7).collect::<String>();
                            out.push_str(&format!("{} {} ({})\n", short_hash, e.message, e.author));
                        }
                        out
                    }
                    Err(e) => {
                        return ToolResult {
                            call_id: call_id.to_string(),
                            output: e.to_string(),
                            is_error: true,
                        }
                    }
                }
            }
            "branches" => match crate::fs::git::branches(path) {
                Ok(branches) => {
                    let mut out = String::new();
                    for b in &branches {
                        let marker = if b.current { "* " } else { "  " };
                        out.push_str(&format!("{}{}\n", marker, b.name));
                    }
                    out
                }
                Err(e) => {
                    return ToolResult {
                        call_id: call_id.to_string(),
                        output: e.to_string(),
                        is_error: true,
                    }
                }
            },
            other => {
                return ToolResult {
                    call_id: call_id.to_string(),
                    output: format!(
                        "Unknown git operation: {}. Use status, diff, log, or branches.",
                        other
                    ),
                    is_error: true,
                }
            }
        };

        ToolResult {
            call_id: call_id.to_string(),
            output,
            is_error: false,
        }
    }
}

// ── Web Fetch Tool ──

pub struct WebFetchTool;

#[async_trait]
impl Tool for WebFetchTool {
    fn name(&self) -> &str {
        "web_fetch"
    }
    fn description(&self) -> &str {
        "Fetch a web page and return its HTML content. Useful for reading documentation, checking APIs, or scraping web content."
    }
    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "URL to fetch"
                },
                "max_length": {
                    "type": "number",
                    "description": "Maximum number of characters to return (default: 10000)"
                }
            },
            "required": ["url"]
        })
    }
    async fn execute(&self, call_id: &str, args: Value) -> ToolResult {
        let url = match args.get("url").and_then(|v| v.as_str()) {
            Some(u) => u,
            None => {
                return ToolResult {
                    call_id: call_id.to_string(),
                    output: "Missing required argument: url".to_string(),
                    is_error: true,
                }
            }
        };
        let max_length = args
            .get("max_length")
            .and_then(|v| v.as_u64())
            .unwrap_or(10000) as usize;

        let client = match reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .timeout(std::time::Duration::from_secs(30))
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                return ToolResult {
                    call_id: call_id.to_string(),
                    output: format!("Failed to create HTTP client: {}", e),
                    is_error: true,
                }
            }
        };

        match client.get(url).send().await {
            Ok(response) => {
                let status = response.status();
                if !status.is_success() {
                    return ToolResult {
                        call_id: call_id.to_string(),
                        output: format!("HTTP {} {}: {}", status.as_u16(), status.canonical_reason().unwrap_or("Unknown"), url),
                        is_error: true,
                    }
                }
                match response.text().await {
                    Ok(body) => {
                        let truncated = if body.len() > max_length {
                            format!("{}...\n\n[Response truncated to {} characters]",
                                &body[..max_length], max_length)
                        } else {
                            body
                        };
                        ToolResult {
                            call_id: call_id.to_string(),
                            output: format!("Status: {}\n\n{}", status.as_u16(), truncated),
                            is_error: false,
                        }
                    }
                    Err(e) => ToolResult {
                        call_id: call_id.to_string(),
                        output: format!("Failed to read response body: {}", e),
                        is_error: true,
                    },
                }
            }
            Err(e) => ToolResult {
                call_id: call_id.to_string(),
                output: format!("Request failed: {}", e),
                is_error: true,
            },
        }
    }
}

// ── Todo Tree Tool ──

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum TodoStatus {
    Todo,
    InProgress,
    Done,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct TodoNode {
    id: String,
    title: String,
    status: TodoStatus,
    children: Vec<String>,
    parent: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct TodoStore {
    nodes: HashMap<String, TodoNode>,
    root_items: Vec<String>,
}

impl TodoStore {
    fn load(path: &str) -> Self {
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

    fn add_node(
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

    fn update_status(&mut self, id: &str, new_status: TodoStatus) -> Result<(), String> {
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

    fn build_tree_json(&self, ids: &[String]) -> Vec<Value> {
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

    fn remove_node(&mut self, id: &str) -> Result<Vec<String>, String> {
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
                    "enum": ["create", "list", "update", "get", "delete"],
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
                    "Unknown operation: {}. Use create, list, update, get, or delete.",
                    other
                ),
                is_error: true,
            },
        }
    }
}

pub struct McpProxyTool {
    pub local_name: String,
    pub local_description: String,
    pub schema: Value,
    pub server: McpServerConfig,
    pub remote_name: String,
}

#[async_trait]
impl Tool for McpProxyTool {
    fn name(&self) -> &str {
        &self.local_name
    }

    fn description(&self) -> &str {
        &self.local_description
    }

    fn input_schema(&self) -> Value {
        self.schema.clone()
    }

    fn danger(&self) -> ToolDanger {
        ToolDanger::Destructive
    }

    async fn execute(&self, call_id: &str, args: Value) -> ToolResult {
        match crate::mcp::call_tool(self.server.clone(), self.remote_name.clone(), args).await {
            Ok((output, is_error)) => ToolResult {
                call_id: call_id.to_string(),
                output,
                is_error,
            },
            Err(e) => ToolResult {
                call_id: call_id.to_string(),
                output: format!("MCP tool call failed: {}", e),
                is_error: true,
            },
        }
    }
}

// ── Tool Registry ──

pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    /// A registry with no tools registered — used to build the strict,
    /// Squire-only tool surface (Q5) instead of the default built-in set.
    pub fn empty() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn new() -> Self {
        let mut reg = Self {
            tools: HashMap::new(),
        };
        // Remove other built-in tools for now, only keep TerminalTool
        //        reg.register(Box::new(FileReadTool));
        //        reg.register(Box::new(FileWriteTool));
        //        reg.register(Box::new(CodeSearchTool));
        reg.register(Box::new(TerminalTool));
        reg.register(Box::new(WebFetchTool));
        // Placeholder store path; callers with a live session id override this
        // via `TodoTreeTool::for_session(..)` before the turn runs.
        reg.register(Box::new(TodoTreeTool::default()));
        //        reg.register(Box::new(GitTool));
        reg
    }

    pub fn register(&mut self, tool: Box<dyn Tool>) {
        let name = tool.name().to_string();
        self.tools.insert(name, tool);
    }

    pub fn get(&self, name: &str) -> Option<&dyn Tool> {
        self.tools.get(name).map(|t| t.as_ref())
    }

    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools
            .values()
            .map(|t| ToolDefinition {
                name: t.name().to_string(),
                description: t.description().to_string(),
                input_schema: t.input_schema(),
            })
            .collect()
    }

    pub fn danger(&self, name: &str) -> Option<ToolDanger> {
        self.tools.get(name).map(|t| t.danger())
    }

    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }
}

// ── Pending Tool Calls State ──

use tokio::sync::oneshot;

pub type ApprovalSender = oneshot::Sender<bool>;
pub type ApprovalReceiver = oneshot::Receiver<bool>;

use tokio::sync::Mutex;

pub struct PendingApprovals {
    pub pending: Arc<Mutex<HashMap<String, ApprovalSender>>>,
}

impl PendingApprovals {
    pub fn new() -> Self {
        Self {
            pending: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

// ── Pending AskUser Questions State (sa-5: Squire response-field AskUser loop) ──
//
// Structurally identical to `PendingApprovals` above — see
// `.AiControl/root/Squire/ask-user-loop/decisions.md` for why this is a
// separate registry/event/command trio rather than reusing the tool-approval
// one. `String` (the user's free-text answer) stands in for `bool` (the
// approval decision) as the oneshot channel's payload.

pub type AskUserAnswerSender = oneshot::Sender<String>;
pub type AskUserAnswerReceiver = oneshot::Receiver<String>;

pub struct PendingAskUserQuestions {
    pub pending: Arc<Mutex<HashMap<String, AskUserAnswerSender>>>,
}

impl PendingAskUserQuestions {
    pub fn new() -> Self {
        Self {
            pending: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Default for PendingAskUserQuestions {
    fn default() -> Self {
        Self::new()
    }
}

// ── Subagent Tool ──
//
// Spawns a child LLM conversation (subagent) that runs independently with
// access to the full tool registry. The subagent's progress is streamed to
// the frontend via Tauri events, and the result is returned when complete.
// Used as a regular tool in Legacy mode; discoverable through `invoke` in
// Squire mode.

pub struct SubagentTool {
    pub app_handle: tauri::AppHandle,
    pub store: Arc<dyn crate::storage::conversation_store::ConversationStore>,
    pub enabled_mcp_servers: Vec<crate::state::config::McpServerConfig>,
    pub provider: Arc<dyn crate::llm::provider::LlmProvider>,
    pub model: String,
    pub provider_name: String,
    pub verbose_logging: bool,
    pub project_path: String,
}

#[async_trait]
impl Tool for SubagentTool {
    fn name(&self) -> &str {
        "subagent"
    }

    fn description(&self) -> &str {
        "Spawn a sub-agent to work on a task independently. The sub-agent gets full tool access and reports back when done."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "task": {
                    "type": "string",
                    "description": "The task to delegate to the sub-agent"
                }
            },
            "required": ["task"]
        })
    }

    fn danger(&self) -> ToolDanger {
        ToolDanger::Safe
    }

    async fn execute(&self, call_id: &str, args: Value) -> ToolResult {
        let task = match args.get("task").and_then(|v| v.as_str()) {
            Some(t) => t,
            None => {
                return ToolResult {
                    call_id: call_id.to_string(),
                    output: "Missing required argument: task".to_string(),
                    is_error: true,
                }
            }
        };

        let session_title = if task.len() > 60 {
            format!("Subagent: {}...", &task[..57])
        } else {
            format!("Subagent: {}", task)
        };

        // Create a new hidden session for the subagent
        let new_session = match self
            .store
            .create_session(
                crate::storage::conversation_store::NewSession {
                    title: session_title,
                    context_mode: Some(crate::storage::conversation_store::ContextMode::Legacy),
                },
            )
            .await
        {
            Ok(s) => s,
            Err(e) => {
                return ToolResult {
                    call_id: call_id.to_string(),
                    output: format!("Failed to create subagent session: {}", e),
                    is_error: true,
                }
            }
        };

        let subagent_session_id = new_session.id;

        // Append the task as a user message
        let _ = self
            .store
            .append_message(
                crate::storage::conversation_store::NewMessage {
                    session_id: subagent_session_id,
                    role: crate::storage::conversation_store::MessageRole::User,
                    content: task.to_string(),
                    thinking_content: None,
                },
            )
            .await;

        // Clone everything needed for the background task
        let app = self.app_handle.clone();
        let store = self.store.clone();
        let enabled_mcp_servers = self.enabled_mcp_servers.clone();
        let provider = self.provider.clone();
        let model = self.model.clone();
        let provider_name = self.provider_name.clone();
        let verbose = self.verbose_logging;
        let parent_call_id = call_id.to_string();
        let task_string = task.to_string();
        let _project_path = self.project_path.clone();
        let subagent_id_str = subagent_session_id.to_string();

        // Emit created event
        let _ = app.emit(
            "subagent-created",
            serde_json::json!({
                "session_id": subagent_id_str,
                "parent_call_id": parent_call_id,
                "task": task_string,
                "provider_name": provider_name,
                "model": model,
            }),
        );

        // Spawn background task that runs the subagent
        let handle = tokio::spawn(async move {
            use crate::agent::ToolRegistry;
            use crate::agent::McpProxyTool;
            use crate::llm::provider::{ChatMessage as LmChatMessage, ChatRole, ChatRequest, FinishReason, StreamEvent, ToolCall};
            use crate::storage::conversation_store::NewMessage;
            use std::collections::HashSet;

            // Build a fresh tool registry for the subagent (same pattern as the parent)
            let mut sub_tool_registry = ToolRegistry::new();
            // Scope the subagent's todo tree to its own hidden session.
            sub_tool_registry.register(Box::new(TodoTreeTool::for_session(
                &subagent_session_id.to_string(),
            )));
            let mut used_names: HashSet<String> = sub_tool_registry
                .definitions()
                .into_iter()
                .map(|d| d.name)
                .collect();

            for server in &enabled_mcp_servers {
                match crate::mcp::discover_tools(server.clone()).await {
                    Ok(tools) => {
                        for tool in tools {
                            let server_id = server
                                .id
                                .chars()
                                .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
                                .collect::<String>();
                            let remote_tool_name = tool.name.clone();
                            let tool_id = remote_tool_name
                                .chars()
                                .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
                                .collect::<String>();

                            let mut local_name = format!("mcp_{}_{}", server_id, tool_id);
                            let mut i = 2;
                            while used_names.contains(&local_name) {
                                local_name = format!("mcp_{}_{}_{}", server_id, tool_id, i);
                                i += 1;
                            }
                            used_names.insert(local_name.clone());

                            let local_description = format!(
                                "MCP tool '{}' from server '{}': {}",
                                remote_tool_name, server.name, tool.description
                            );

                            sub_tool_registry.register(Box::new(McpProxyTool {
                                local_name: local_name.clone(),
                                local_description,
                                schema: tool.input_schema.clone(),
                                server: server.clone(),
                                remote_name: remote_tool_name.clone(),
                            }));
                        }
                    }
                    Err(e) => {
                        if verbose {
                            let _ = app.emit(
                                "output:append",
                                serde_json::json!({
                                    "source": "subagent",
                                    "line": format!(
                                        "WARNING: MCP discovery failed for server '{}': {}",
                                        server.name, e
                                    ),
                                    "timestamp": chrono::Utc::now().to_rfc3339(),
                                }),
                            );
                        }
                    }
                }
            }

            let tool_registry = Arc::new(sub_tool_registry);
            let tool_defs = tool_registry.definitions();

            let mut messages: Vec<LmChatMessage> = Vec::new();

            // Load the user message from the subagent session
            match store.get_session(subagent_session_id).await {
                Ok(session) => {
                    for msg in &session.messages {
                        let role = match msg.role {
                            crate::storage::conversation_store::MessageRole::User => ChatRole::User,
                            crate::storage::conversation_store::MessageRole::Assistant => ChatRole::Assistant,
                            crate::storage::conversation_store::MessageRole::System => ChatRole::System,
                        };
                        messages.push(LmChatMessage {
                            role,
                            content: msg.content.clone(),
                            tool_call_id: None,
                            tool_calls: None,
                            reasoning_content: msg.thinking_content.clone(),
                        });
                    }
                }
                Err(e) => {
                    let _ = app.emit(
                        "subagent-error",
                        serde_json::json!({
                            "session_id": subagent_session_id.to_string(),
                            "error": format!("Failed to load session: {}", e),
                        }),
                    );
                    return;
                }
            }

            // Add a system message to guide the subagent
            if messages.is_empty() || !matches!(messages[0].role, ChatRole::System) {
                messages.insert(0, LmChatMessage {
                    role: ChatRole::System,
                    content: "You are a helpful sub-agent. You have access to tools to help complete the task. Work autonomously and report your findings.".to_string(),
                    tool_call_id: None,
                    tool_calls: None,
                    reasoning_content: None,
                });
            }

            // Main subagent loop — at most 10 tool-call rounds
            let mut max_tool_rounds = 10;
            loop {
                if max_tool_rounds == 0 {
                    let _ = app.emit(
                        "subagent-error",
                        serde_json::json!({
                            "session_id": subagent_session_id.to_string(),
                            "error": "Subagent exceeded maximum tool call rounds".to_string(),
                        }),
                    );
                    return;
                }
                max_tool_rounds -= 1;

                let request = ChatRequest {
                    model: model.clone(),
                    messages: messages.clone(),
                    tools: tool_defs.clone(),
                    thinking_level: None,
                    temperature: None,
                    max_tokens: None,
                };

                if verbose {
                    let _ = app.emit(
                        "output:append",
                        serde_json::json!({
                            "source": "subagent",
                            "line": format!("[subagent] >>> REQUEST ({} messages)", messages.len()),
                            "timestamp": chrono::Utc::now().to_rfc3339(),
                        }),
                    );
                }

                let mut stream = match provider.chat(request).await {
                    Ok(s) => s,
                    Err(e) => {
                        let _ = app.emit(
                            "subagent-error",
                            serde_json::json!({
                                "session_id": subagent_session_id.to_string(),
                                "error": format!("Subagent LLM request failed: {}", e),
                            }),
                        );
                        return;
                    }
                };

                let mut full_response = String::new();
                let mut tool_calls: Vec<ToolCall> = Vec::new();
                let mut finish_reason: Option<FinishReason> = None;

                while let Some(event) = stream.recv().await {
                    match event {
                        StreamEvent::Chunk(text) => {
                            full_response.push_str(&text);
                            let _ = app.emit(
                                "subagent-chunk",
                                serde_json::json!({
                                    "session_id": subagent_session_id.to_string(),
                                    "text": text,
                                }),
                            );
                        }
                        StreamEvent::Thinking(text) => {
                            let think_chunk = format!("[thinking] {}", text);
                            let _ = app.emit(
                                "subagent-chunk",
                                serde_json::json!({
                                    "session_id": subagent_session_id.to_string(),
                                    "text": think_chunk,
                                }),
                            );
                        }
                        StreamEvent::ToolCall(tc) => {
                            tool_calls.push(tc);
                        }
                        StreamEvent::Log(msg) => {
                            if verbose {
                                let _ = app.emit(
                                    "output:append",
                                    serde_json::json!({
                                        "source": "subagent",
                                        "line": format!("[subagent] {}", msg),
                                        "timestamp": chrono::Utc::now().to_rfc3339(),
                                    }),
                                );
                            }
                        }
                        StreamEvent::Done(reason) => {
                            finish_reason = Some(reason);
                            break;
                        }
                        StreamEvent::Error(err) => {
                            let _ = app.emit(
                                "subagent-error",
                                serde_json::json!({
                                    "session_id": subagent_session_id.to_string(),
                                    "error": format!("Subagent stream error: {}", err),
                                }),
                            );
                            return;
                        }
                    }
                }

                if finish_reason.is_none() {
                    finish_reason = Some(FinishReason::Stop);
                }

                let reason = finish_reason.unwrap();

                match reason {
                    FinishReason::ToolCalls => {
                        // Persist the assistant message with tool calls
                        let _ = store
                            .append_message(NewMessage {
                                session_id: subagent_session_id,
                                role: crate::storage::conversation_store::MessageRole::Assistant,
                                content: full_response.clone(),
                                thinking_content: None,
                            })
                            .await;

                        // Push assistant message with tool calls to local messages
                        messages.push(LmChatMessage {
                            role: ChatRole::Assistant,
                            content: full_response,
                            tool_call_id: None,
                            tool_calls: Some(tool_calls.clone()),
                            reasoning_content: None,
                        });

                        // Execute each tool call (auto-approved — subagent has implicit approval)
                        for tc in &tool_calls {
                            let tool_result = if let Some(tool) = tool_registry.get(&tc.name) {
                                tool.execute(&tc.id, tc.arguments.clone()).await
                            } else {
                                ToolResult {
                                    call_id: tc.id.clone(),
                                    output: format!("Unknown tool: {}", tc.name),
                                    is_error: true,
                                }
                            };

                            // Push tool result to messages
                            messages.push(LmChatMessage {
                                role: ChatRole::Tool,
                                content: tool_result.output.clone(),
                                tool_call_id: Some(tc.id.clone()),
                                tool_calls: None,
                                reasoning_content: None,
                            });

                            if verbose {
                                let _ = app.emit(
                                    "output:append",
                                    serde_json::json!({
                                        "source": "subagent",
                                        "line": format!(
                                            "[subagent] tool {} = {} bytes, is_error={}",
                                            tc.name,
                                            tool_result.output.len(),
                                            tool_result.is_error
                                        ),
                                        "timestamp": chrono::Utc::now().to_rfc3339(),
                                    }),
                                );
                            }
                        }

                        // Continue the loop for another LLM call
                        continue;
                    }
                    FinishReason::Stop | FinishReason::Length => {
                        // Persist the final assistant message
                        let _ = store
                            .append_message(NewMessage {
                                session_id: subagent_session_id,
                                role: crate::storage::conversation_store::MessageRole::Assistant,
                                content: full_response.clone(),
                                thinking_content: None,
                            })
                            .await;

                        let _ = app.emit(
                            "subagent-done",
                            serde_json::json!({
                                "session_id": subagent_session_id.to_string(),
                                "result": full_response,
                                "is_error": false,
                            }),
                        );

                        if verbose {
                            let _ = app.emit(
                                "output:append",
                                serde_json::json!({
                                    "source": "subagent",
                                    "line": format!(
                                        "[subagent] completed: {} chars",
                                        full_response.len()
                                    ),
                                    "timestamp": chrono::Utc::now().to_rfc3339(),
                                }),
                            );
                        }

                        return;
                    }
                    FinishReason::Error => {
                        let _ = app.emit(
                            "subagent-error",
                            serde_json::json!({
                                "session_id": subagent_session_id.to_string(),
                                "error": "Subagent LLM returned an error finish reason".to_string(),
                            }),
                        );
                        return;
                    }
                }
            }
        });

        // Store handle so the subagent can be stopped/cancelled
        let sid_str = subagent_session_id.to_string();
        if let Some(state) = self.app_handle.try_state::<crate::commands::AppState>() {
            let mut tasks = state.subagent_tasks.lock().await;
            tasks.insert(sid_str, handle);
        }

        // Return immediately — subagent runs in background
        ToolResult {
            call_id: call_id.to_string(),
            output: format!(
                "[Subagent started on task: {}]\nSession: {}\nThe subagent is working independently and will report back when done.",
                task,
                subagent_session_id
            ),
            is_error: false,
        }
    }
}

#[cfg(test)]
#[path = "mod_test.rs"]
mod tests;
