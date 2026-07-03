use serde_json::json;
use squirecli_lib::agent::{
    CodeSearchTool, FileReadTool, FileWriteTool, GitTool, PendingApprovals, TerminalTool, Tool,
    ToolDanger, ToolRegistry, ToolResult,
};

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
