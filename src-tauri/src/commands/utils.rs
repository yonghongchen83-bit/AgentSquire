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

// ── DeepSeek JSON cleanup via safe-json-repair ──

/// Clean up malformed JSON text commonly produced by DeepSeek models.
///
/// Delegates to the `safe-json-repair` crate, which uses a stack-aware
/// tolerant parser that never throws, never silently drops data, and
/// handles all the common DeepSeek failure modes: premature root close,
/// markdown fences, trailing commas, JS keywords, single quotes,
/// full-width punctuation, etc.
pub fn clean_deepseek_json(raw: &str) -> String {
    let result = safe_json_repair::repair(raw, &safe_json_repair::Options::default());
    result.json
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
