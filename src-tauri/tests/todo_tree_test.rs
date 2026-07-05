use squirecli_lib::agent::{Tool, TodoTreeTool, ToolResult};
use serde_json::Value;

fn call_tool(operation: &str, args: &[(&str, &str)]) -> ToolResult {
    let tool = TodoTreeTool::default();
    let mut map = serde_json::Map::new();
    map.insert("operation".into(), Value::String(operation.into()));
    let path = format!("todo_tree_test_{}.json", std::process::id());
    map.insert("path".into(), Value::String(path.clone()));
    for (k, v) in args {
        map.insert(k.to_string(), Value::String(v.to_string()));
    }
    let future = tool.execute("test-call", Value::Object(map));
    let result = tokio::runtime::Runtime::new().unwrap().block_on(future);
    let _ = std::fs::remove_file(&path);
    result
}

#[test]
fn test_tool_create_root() {
    let result = call_tool("create", &[("title", "Root task")]);
    assert!(!result.is_error, "Create failed: {}", result.output);
    assert!(result.output.contains("Root task"));
}

#[test]
fn test_tool_create_child() {
    let tool = TodoTreeTool::default();
    let path = format!("todo_tree_test_child_{}.json", std::process::id());

    let mut args1 = serde_json::Map::new();
    args1.insert("operation".into(), Value::String("create".into()));
    args1.insert("title".into(), Value::String("Parent".into()));
    args1.insert("path".into(), Value::String(path.clone()));
    let r1 = tokio::runtime::Runtime::new().unwrap().block_on(tool.execute("c1", Value::Object(args1)));
    assert!(!r1.is_error, "Create parent failed: {}", r1.output);

    let parent_id = r1.output.split("(id: ").nth(1).and_then(|s| s.split(')').next()).unwrap_or("").to_string();
    assert!(!parent_id.is_empty(), "Could not extract parent ID from: {}", r1.output);

    let mut args2 = serde_json::Map::new();
    args2.insert("operation".into(), Value::String("create".into()));
    args2.insert("title".into(), Value::String("Child".into()));
    args2.insert("parent_id".into(), Value::String(parent_id));
    args2.insert("path".into(), Value::String(path.clone()));
    let r2 = tokio::runtime::Runtime::new().unwrap().block_on(tool.execute("c2", Value::Object(args2)));
    assert!(!r2.is_error, "Create child failed: {}", r2.output);

    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_tool_list_returns_json() {
    let tool = TodoTreeTool::default();
    let path = format!("todo_tree_test_list_{}.json", std::process::id());

    let mut args_create = serde_json::Map::new();
    args_create.insert("operation".into(), Value::String("create".into()));
    args_create.insert("title".into(), Value::String("Test".into()));
    args_create.insert("path".into(), Value::String(path.clone()));
    tokio::runtime::Runtime::new().unwrap().block_on(tool.execute("c1", Value::Object(args_create)));

    let mut args_list = serde_json::Map::new();
    args_list.insert("operation".into(), Value::String("list".into()));
    args_list.insert("path".into(), Value::String(path.clone()));
    let result = tokio::runtime::Runtime::new().unwrap().block_on(tool.execute("l1", Value::Object(args_list)));
    assert!(!result.is_error, "List failed: {}", result.output);

    let parsed: Value = serde_json::from_str(&result.output)
        .unwrap_or_else(|_| panic!("List output is not valid JSON: {}", result.output));
    assert_eq!(parsed["_type"], "todo_tree");
    assert!(parsed["items"].is_array());

    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_tool_update_status() {
    let tool = TodoTreeTool::default();
    let path = format!("todo_tree_test_update_{}.json", std::process::id());

    let mut args_create = serde_json::Map::new();
    args_create.insert("operation".into(), Value::String("create".into()));
    args_create.insert("title".into(), Value::String("Item".into()));
    args_create.insert("path".into(), Value::String(path.clone()));
    let create_result = tokio::runtime::Runtime::new().unwrap().block_on(tool.execute("c1", Value::Object(args_create)));
    let item_id = create_result.output.split("(id: ").nth(1).and_then(|s| s.split(')').next()).unwrap_or("").to_string();

    let mut args_update = serde_json::Map::new();
    args_update.insert("operation".into(), Value::String("update".into()));
    args_update.insert("id".into(), Value::String(item_id));
    args_update.insert("status".into(), Value::String("in_progress".into()));
    args_update.insert("path".into(), Value::String(path.clone()));
    let result = tokio::runtime::Runtime::new().unwrap().block_on(tool.execute("u1", Value::Object(args_update)));
    assert!(!result.is_error, "Update failed: {}", result.output);

    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_tool_get_returns_json() {
    let tool = TodoTreeTool::default();
    let path = format!("todo_tree_test_get_{}.json", std::process::id());

    let mut args_create = serde_json::Map::new();
    args_create.insert("operation".into(), Value::String("create".into()));
    args_create.insert("title".into(), Value::String("GetMe".into()));
    args_create.insert("path".into(), Value::String(path.clone()));
    let create_result = tokio::runtime::Runtime::new().unwrap().block_on(tool.execute("c1", Value::Object(args_create)));
    let item_id = create_result.output.split("(id: ").nth(1).and_then(|s| s.split(')').next()).unwrap_or("").to_string();

    let mut args_get = serde_json::Map::new();
    args_get.insert("operation".into(), Value::String("get".into()));
    args_get.insert("id".into(), Value::String(item_id));
    args_get.insert("path".into(), Value::String(path.clone()));
    let result = tokio::runtime::Runtime::new().unwrap().block_on(tool.execute("g1", Value::Object(args_get)));
    assert!(!result.is_error, "Get failed: {}", result.output);

    let parsed: Value = serde_json::from_str(&result.output)
        .unwrap_or_else(|_| panic!("Get output is not valid JSON: {}", result.output));
    assert_eq!(parsed["_type"], "todo_tree");

    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_tool_delete() {
    let tool = TodoTreeTool::default();
    let path = format!("todo_tree_test_delete_{}.json", std::process::id());

    let mut args_create = serde_json::Map::new();
    args_create.insert("operation".into(), Value::String("create".into()));
    args_create.insert("title".into(), Value::String("DeleteMe".into()));
    args_create.insert("path".into(), Value::String(path.clone()));
    let create_result = tokio::runtime::Runtime::new().unwrap().block_on(tool.execute("c1", Value::Object(args_create)));
    let item_id = create_result.output.split("(id: ").nth(1).and_then(|s| s.split(')').next()).unwrap_or("").to_string();

    let mut args_delete = serde_json::Map::new();
    args_delete.insert("operation".into(), Value::String("delete".into()));
    args_delete.insert("id".into(), Value::String(item_id));
    args_delete.insert("path".into(), Value::String(path.clone()));
    let result = tokio::runtime::Runtime::new().unwrap().block_on(tool.execute("d1", Value::Object(args_delete)));
    assert!(!result.is_error, "Delete failed: {}", result.output);
    assert!(result.output.contains("Deleted"));

    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_tool_multi_turn_create_then_list() {
    let tool = TodoTreeTool::default();
    let path = format!("todo_tree_test_multi_{}.json", std::process::id());

    for (title, _parent) in &[("Plan", None::<&str>), ("Research", None), ("Implement", None)] {
        let mut args = serde_json::Map::new();
        args.insert("operation".into(), Value::String("create".into()));
        args.insert("title".into(), Value::String(title.to_string()));
        args.insert("path".into(), Value::String(path.clone()));
        let r = tokio::runtime::Runtime::new().unwrap().block_on(tool.execute("c", Value::Object(args)));
        assert!(!r.is_error, "Create '{}' failed: {}", title, r.output);
    }

    let mut args_list = serde_json::Map::new();
    args_list.insert("operation".into(), Value::String("list".into()));
    args_list.insert("path".into(), Value::String(path.clone()));
    let list_result = tokio::runtime::Runtime::new().unwrap().block_on(tool.execute("l", Value::Object(args_list)));
    assert!(!list_result.is_error, "List failed: {}", list_result.output);

    let parsed: Value = serde_json::from_str(&list_result.output)
        .unwrap_or_else(|_| panic!("List output not JSON: {}", list_result.output));
    assert_eq!(parsed["items"].as_array().unwrap().len(), 3);

    let _ = std::fs::remove_file(&path);
}
