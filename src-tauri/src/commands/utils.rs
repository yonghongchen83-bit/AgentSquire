pub fn derive_session_title_from_message(content: &str) -> Option<String> {
    let first_line = content
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())?;

    let normalized = first_line.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        return None;
    }

    let max_chars = 60;
    let mut chars = normalized.chars();
    let head: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        Some(format!("{}...", head.trim_end()))
    } else {
        Some(head)
    }
}

pub fn blocked_hint_for_tool(tool_name: &str) -> &'static str {
    if tool_name.starts_with("mcp_") {
        "MCP server may be waiting, unresponsive, or not sending a JSON-RPC response"
    } else if tool_name == "run_terminal" {
        "terminal command may be long-running or waiting for interactive input"
    } else {
        "tool call is taking unusually long without completion signal"
    }
}

pub fn is_valid_tool_schema(schema: &serde_json::Value) -> bool {
    matches!(schema.get("type").and_then(|v| v.as_str()), Some("object"))
}

// ── Terminal command path analysis ──

use serde::Serialize;
use std::path::Path;

/// Analysis result of a terminal command's arguments for path extraction.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzedPath {
    pub original: String,
    pub resolved: String,
    pub is_outside_workspace: bool,
}

/// Parsed command info with extracted path information.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandAnalysis {
    pub command: String,
    pub args: Vec<String>,
    pub paths: Vec<AnalyzedPath>,
}

/// Analyze a terminal command's arguments to find file/folder paths
/// and check whether they lie inside or outside the workspace.
pub fn analyze_terminal_command(
    command: &str,
    args: &[String],
    workdir: Option<&str>,
    workspace_path: &str,
) -> CommandAnalysis {
    let workspace = Path::new(workspace_path);
    let workdir_path = workdir
        .and_then(|w| {
            let p = Path::new(w);
            if p.is_absolute() {
                Some(p.to_path_buf())
            } else {
                Some(workspace.join(p))
            }
        })
        .unwrap_or_else(|| workspace.to_path_buf());

    let mut paths: Vec<AnalyzedPath> = Vec::new();
    for arg in args {
        // Try the whole arg first; for --flag=value patterns, extract the value part
        let candidates: Vec<&str> = if arg.contains('=') && (arg.starts_with("--") || arg.starts_with('-')) {
            arg.splitn(2, '=').nth(1).map(|v| vec![v.trim()]).unwrap_or_default()
        } else {
            vec![arg.as_str()]
        };

        for candidate in candidates {
            if looks_like_path(candidate) {
                let resolved = if Path::new(candidate).is_absolute() {
                    Path::new(candidate).to_path_buf()
                } else {
                    workdir_path.join(candidate)
                };

                let normalized = normalize_path(&resolved);
                let is_outside = !normalized.starts_with(workspace);

                // Deduplicate
                if !paths.iter().any(|p| p.original == candidate) {
                    paths.push(AnalyzedPath {
                        original: candidate.to_string(),
                        resolved: normalized.to_string_lossy().to_string(),
                        is_outside_workspace: is_outside,
                    });
                }
            }
        }
    }

    CommandAnalysis {
        command: command.to_string(),
        args: args.to_vec(),
        paths,
    }
}

fn looks_like_path(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    // Starts with path-like prefixes
    if s.starts_with('.') || s.starts_with('/') || s.starts_with('\\') || s.starts_with('~') {
        return true;
    }
    // Contains path separators
    if s.contains('/') || s.contains('\\') {
        return true;
    }
    // Contains a file extension pattern (e.g. config.json, script.py)
    if let Some(dot_idx) = s.rfind('.') {
        if dot_idx > 0 && dot_idx < s.len() - 1 {
            let after_dot = &s[dot_idx + 1..];
            if (2..=6).contains(&after_dot.len())
                && after_dot.chars().all(|c| c.is_ascii_alphanumeric())
            {
                // Avoid flag-like patterns (--verbose, -o)
                if !s.starts_with("--") && !s.starts_with('-') {
                    return true;
                }
            }
        }
    }
    false
}

/// Normalize a path, resolving `.` and `..` components.
fn normalize_path(path: &Path) -> std::path::PathBuf {
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => continue,
            std::path::Component::ParentDir => {
                components.pop();
            }
            other => components.push(other.as_os_str().to_os_string()),
        }
    }
    let mut result = std::path::PathBuf::new();
    for c in components {
        result.push(c);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_title_from_first_non_empty_line() {
        let title = derive_session_title_from_message("\n   Hello   world\nsecond line");
        assert_eq!(title.as_deref(), Some("Hello world"));
    }

    #[test]
    fn truncates_long_titles() {
        let long = "a".repeat(80);
        let title = derive_session_title_from_message(&long).expect("title should be derived");
        assert!(title.ends_with("..."));
        assert!(title.len() <= 63);
    }

    #[test]
    fn returns_none_for_empty_content() {
        let title = derive_session_title_from_message("\n   \n\t");
        assert!(title.is_none());
    }

    #[test]
    fn blocked_hint_maps_tool_categories() {
        assert!(blocked_hint_for_tool("mcp_server_tool").contains("MCP"));
        assert!(blocked_hint_for_tool("run_terminal").contains("terminal"));
        assert!(blocked_hint_for_tool("other").contains("tool call"));
    }

    #[test]
    fn schema_validation_accepts_object_schema() {
        let schema = serde_json::json!({"type": "object", "properties": {"a": {"type": "string"}}});
        assert!(is_valid_tool_schema(&schema));
    }

    #[test]
    fn schema_validation_rejects_non_object_schema() {
        let schema = serde_json::json!({"type": "array", "items": {"type": "string"}});
        assert!(!is_valid_tool_schema(&schema));
    }

    #[test]
    fn schema_validation_rejects_missing_type() {
        let schema = serde_json::json!({"properties": {"a": {"type": "string"}}});
        assert!(!is_valid_tool_schema(&schema));
    }

    // ── Terminal command path analysis tests ──

    #[test]
    fn detects_relative_path_in_args() {
        let analysis =
            analyze_terminal_command("npm", &["run", "build", "--config", "./webpack.config.js"].map(String::from), Some("."), "/project");
        assert!(analysis.paths.iter().any(|p| p.original == "./webpack.config.js"));
        assert!(!analysis.paths[0].is_outside_workspace);
    }

    #[test]
    fn detects_absolute_path_outside_workspace() {
        let analysis = analyze_terminal_command(
            "cat",
            &["/etc/passwd"].map(String::from),
            None,
            "/home/user/project",
        );
        assert!(analysis.paths.iter().any(|p| p.original == "/etc/passwd"));
        assert!(analysis.paths[0].is_outside_workspace);
    }

    #[test]
    fn detects_path_with_extension_as_arg() {
        let analysis = analyze_terminal_command(
            "python",
            &["script.py", "data.csv"].map(String::from),
            None,
            "/project",
        );
        assert_eq!(analysis.paths.len(), 2);
        assert!(!analysis.paths[0].is_outside_workspace);
    }

    #[test]
    fn does_not_flag_flags_as_paths() {
        let analysis = analyze_terminal_command(
            "cargo",
            &["build", "--release", "--verbose"].map(String::from),
            None,
            "/project",
        );
        assert!(analysis.paths.is_empty());
    }

    #[test]
    fn deduplicates_paths() {
        let analysis = analyze_terminal_command(
            "test",
            &["file.txt", "file.txt"].map(String::from),
            None,
            "/project",
        );
        assert_eq!(analysis.paths.len(), 1);
    }

    #[test]
    fn handles_equals_flag_value() {
        let analysis = analyze_terminal_command(
            "npm",
            &["run", "build", "--config=./webpack.config.js"].map(String::from),
            Some("."),
            "/project",
        );
        assert!(analysis.paths.iter().any(|p| p.original == "./webpack.config.js"));
    }

    #[test]
    fn command_and_args_are_preserved() {
        let analysis = analyze_terminal_command(
            "npm",
            &["install", "--save-dev", "typescript"].map(String::from),
            None,
            "/project",
        );
        assert_eq!(analysis.command, "npm");
        assert_eq!(analysis.args, vec!["install", "--save-dev", "typescript"]);
    }
}
