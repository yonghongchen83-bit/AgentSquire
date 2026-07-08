use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

use crate::state::config::McpServerConfig;

pub mod context_adapter;
mod decision_tree;
mod file_tools;
mod search_tools;
mod git_tools;
mod terminal_tool;
mod web_tools;
mod todo_tree;
mod subagent;
pub use decision_tree::DecisionTreeTool;
pub use file_tools::{FileReadTool, FileWriteTool};
pub use search_tools::CodeSearchTool;
pub use git_tools::GitTool;
pub use terminal_tool::TerminalTool;
pub use web_tools::WebFetchTool;
pub use todo_tree::{todo_store_path, TodoTreeTool};
pub use subagent::SubagentTool;
#[cfg(test)]
pub(crate) use todo_tree::{TodoStatus, TodoStore};
pub mod squire;
pub mod squire_prompts;
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

#[cfg(test)]
#[path = "mod_test.rs"]
mod tests;






