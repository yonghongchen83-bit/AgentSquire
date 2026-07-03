use squirecli_lib::commands::utils::{
    derive_session_title_from_message,
    blocked_hint_for_tool,
    is_valid_tool_schema,
    analyze_terminal_command,
};

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

#[test]
fn detects_relative_path_in_args() {
    let analysis = analyze_terminal_command(
        "npm",
        &["run", "build", "--config", "./webpack.config.js"].map(String::from),
        Some("."),
        "/project",
    );
    assert!(analysis.paths.iter().any(|p| p.original == "./webpack.config.js"));
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
