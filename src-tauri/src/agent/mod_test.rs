use super::*;

#[test]
fn test_create_root_item() {
    let mut store = TodoStore::default();
    store.add_node("a".into(), "Root task".into(), None).unwrap();
    assert_eq!(store.root_items, vec!["a"]);
    assert_eq!(store.nodes.get("a").unwrap().title, "Root task");
    assert_eq!(store.nodes.get("a").unwrap().status, TodoStatus::Todo);
}

#[test]
fn test_create_child_item() {
    let mut store = TodoStore::default();
    store.add_node("parent".into(), "Parent".into(), None).unwrap();
    store.add_node("child".into(), "Child".into(), Some("parent".into())).unwrap();

    let parent = store.nodes.get("parent").unwrap();
    assert_eq!(parent.children, vec!["child"]);
    assert_eq!(store.nodes.get("child").unwrap().parent, Some("parent".into()));
}

#[test]
fn test_create_child_unknown_parent_fails() {
    let mut store = TodoStore::default();
    let result = store.add_node("c".into(), "Orphan".into(), Some("missing".into()));
    assert!(result.is_err());
}

#[test]
fn test_list_empty() {
    let store = TodoStore::default();
    let tree = store.build_tree_json(&store.root_items);
    assert!(tree.is_empty());
}

#[test]
fn test_list_tree_structure() {
    let mut store = TodoStore::default();
    store.add_node("r".into(), "Root".into(), None).unwrap();
    store.add_node("a".into(), "Child A".into(), Some("r".into())).unwrap();
    store.add_node("b".into(), "Child B".into(), Some("r".into())).unwrap();

    let tree = store.build_tree_json(&store.root_items);
    assert_eq!(tree.len(), 1);
    assert_eq!(tree[0]["title"], "Root");
    assert_eq!(tree[0]["children"].as_array().unwrap().len(), 2);
}

#[test]
fn test_mark_done_only_when_children_done() {
    let mut store = TodoStore::default();
    store.add_node("p".into(), "Parent".into(), None).unwrap();
    store.add_node("c".into(), "Child".into(), Some("p".into())).unwrap();

    let result = store.update_status("p", TodoStatus::Done);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("child"));

    store.update_status("c", TodoStatus::Done).unwrap();
    store.update_status("p", TodoStatus::Done).unwrap();
    assert_eq!(store.nodes.get("p").unwrap().status, TodoStatus::Done);
}

#[test]
fn test_mark_in_progress_no_constraint() {
    let mut store = TodoStore::default();
    store.add_node("p".into(), "Parent".into(), None).unwrap();
    store.add_node("c".into(), "Child".into(), Some("p".into())).unwrap();

    store.update_status("p", TodoStatus::InProgress).unwrap();
    assert_eq!(store.nodes.get("p").unwrap().status, TodoStatus::InProgress);
}

#[test]
fn test_cannot_mark_done_chain() {
    let mut store = TodoStore::default();
    store.add_node("a".into(), "A".into(), None).unwrap();
    store.add_node("b".into(), "B".into(), Some("a".into())).unwrap();
    store.add_node("c".into(), "C".into(), Some("b".into())).unwrap();

    assert!(store.update_status("a", TodoStatus::Done).is_err());

    store.update_status("c", TodoStatus::Done).unwrap();
    store.update_status("b", TodoStatus::Done).unwrap();
    store.update_status("a", TodoStatus::Done).unwrap();
    assert_eq!(store.nodes.get("a").unwrap().status, TodoStatus::Done);
}

#[test]
fn test_delete_removes_descendants() {
    let mut store = TodoStore::default();
    store.add_node("a".into(), "A".into(), None).unwrap();
    store.add_node("b".into(), "B".into(), Some("a".into())).unwrap();
    store.add_node("c".into(), "C".into(), Some("b".into())).unwrap();

    let removed = store.remove_node("a").unwrap();
    assert_eq!(removed.len(), 3);
    assert!(store.nodes.is_empty());
    assert!(store.root_items.is_empty());
}

#[test]
fn test_delete_child_only() {
    let mut store = TodoStore::default();
    store.add_node("a".into(), "A".into(), None).unwrap();
    store.add_node("b".into(), "B".into(), Some("a".into())).unwrap();

    store.remove_node("b").unwrap();
    assert!(store.nodes.contains_key("a"));
    assert!(!store.nodes.contains_key("b"));
    assert_eq!(store.nodes.get("a").unwrap().children.len(), 0);
}

#[test]
fn test_tree_json_roundtrip() {
    let mut store = TodoStore::default();
    store.add_node("r".into(), "Root".into(), None).unwrap();
    store.add_node("c".into(), "Child".into(), Some("r".into())).unwrap();

    let json = serde_json::to_string(&store).unwrap();
    let restored: TodoStore = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.nodes.len(), 2);
    assert_eq!(restored.root_items, vec!["r"]);
}

#[test]
fn test_tool_output_is_json_tree() {
    let mut store = TodoStore::default();
    store.add_node("r".into(), "Root".into(), None).unwrap();
    store.add_node("c".into(), "Child".into(), Some("r".into())).unwrap();

    let tree = store.build_tree_json(&store.root_items);
    let payload = serde_json::json!({ "_type": "todo_tree", "items": tree });
    let output = serde_json::to_string(&payload).unwrap();

    let parsed: Value = serde_json::from_str(&output).unwrap();
    assert_eq!(parsed["_type"], "todo_tree");
    assert_eq!(parsed["items"][0]["title"], "Root");
    assert_eq!(parsed["items"][0]["children"][0]["title"], "Child");
    assert_eq!(parsed["items"][0]["children"][0]["status"], "todo");
}
