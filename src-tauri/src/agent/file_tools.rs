use async_trait::async_trait;
use serde_json::Value;

use super::{Tool, ToolDanger, ToolResult};

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
