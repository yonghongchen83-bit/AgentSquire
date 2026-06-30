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
}
