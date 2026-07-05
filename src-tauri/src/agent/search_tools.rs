use async_trait::async_trait;
use serde_json::Value;

use super::{Tool, ToolResult};

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
