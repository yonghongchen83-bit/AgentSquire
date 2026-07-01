use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

use crate::state::config::McpServerConfig;

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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_tool_registry_contains_all_tools() {
        let reg = ToolRegistry::new();
        let defs = reg.definitions();
        let names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"run_terminal"));
        assert!(names.contains(&"web_fetch"));
        assert_eq!(defs.len(), 2);
    }

    #[test]
    fn test_tool_danger_levels() {
        let reg = ToolRegistry::new();
        assert_eq!(reg.danger("run_terminal"), Some(ToolDanger::Destructive));
        assert_eq!(reg.danger("web_fetch"), Some(ToolDanger::Safe));
        assert_eq!(reg.danger("nonexistent"), None);
    }

    #[test]
    fn test_tool_definitions_have_schemas() {
        let reg = ToolRegistry::new();
        for def in reg.definitions() {
            assert!(!def.name.is_empty(), "tool name should not be empty");
            assert!(
                !def.description.is_empty(),
                "tool description should not be empty"
            );
            assert!(
                def.input_schema.get("type").is_some(),
                "tool {} should have a JSON schema type",
                def.name
            );
        }
    }

    #[tokio::test]
    async fn test_file_read_tool() {
        let tool = FileReadTool;
        assert_eq!(tool.name(), "read_file");
        assert_eq!(tool.danger(), ToolDanger::Safe);

        let result = tool
            .execute("call_1", json!({"path": "/nonexistent/path"}))
            .await;
        assert!(result.is_error);
        assert_eq!(result.call_id, "call_1");
    }

    #[tokio::test]
    async fn test_file_read_tool_missing_arg() {
        let tool = FileReadTool;
        let result = tool.execute("call_1", json!({})).await;
        assert!(result.is_error);
        assert!(result.output.contains("Missing"));
    }

    #[tokio::test]
    async fn test_file_write_tool_missing_args() {
        let tool = FileWriteTool;
        let result = tool.execute("call_1", json!({})).await;
        assert!(result.is_error);
        assert!(result.output.contains("path"));

        let result = tool
            .execute("call_2", json!({"path": "/tmp/test.txt"}))
            .await;
        assert!(result.is_error);
        assert!(result.output.contains("content"));
    }

    #[tokio::test]
    async fn test_search_code_tool_missing_query() {
        let tool = CodeSearchTool;
        let result = tool.execute("call_1", json!({})).await;
        assert!(result.is_error);
        assert!(result.output.contains("query"));
    }

    #[tokio::test]
    async fn test_terminal_tool_missing_command() {
        let tool = TerminalTool;
        assert_eq!(tool.danger(), ToolDanger::Destructive);
        let result = tool.execute("call_1", json!({})).await;
        assert!(result.is_error);
        assert!(result.output.contains("command"));
    }

    #[tokio::test]
    async fn test_git_tool_missing_operation() {
        let tool = GitTool;
        let result = tool.execute("call_1", json!({})).await;
        assert!(result.is_error);
        assert!(result.output.contains("operation"));
    }

    #[tokio::test]
    async fn test_git_tool_bad_operation() {
        let tool = GitTool;
        let result = tool.execute("call_1", json!({"operation": "blarg"})).await;
        assert!(result.is_error);
        assert!(result.output.contains("blarg"));
    }

    #[test]
    fn test_tool_result_serialize() {
        let r = ToolResult {
            call_id: "call_abc".into(),
            output: "hello".into(),
            is_error: false,
        };
        let j = serde_json::to_string(&r).unwrap();
        assert!(j.contains("call_abc"));
        assert!(j.contains("hello"));
    }

    #[test]
    fn test_pending_approvals_new() {
        let pa = PendingApprovals::new();
        assert!(pa.pending.try_lock().unwrap().is_empty());
    }
}
