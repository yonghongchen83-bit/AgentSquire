use async_trait::async_trait;
use serde_json::Value;

use super::{Tool, ToolResult};

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
