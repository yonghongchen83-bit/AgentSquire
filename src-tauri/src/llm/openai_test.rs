use super::*;
use serde_json::json;

#[test]
fn accumulates_incremental_classic_tool_calls() {
    let mut pending = HashMap::new();

    let chunks = vec![
        json!({
            "index": 0,
            "id": "call_00_Btv7wxzaKle8PKCgxFTj4416",
            "type": "function",
            "function": { "name": "run_terminal", "arguments": "" }
        }),
        json!({ "index": 0, "function": { "arguments": "{" } }),
        json!({ "index": 0, "function": { "arguments": "\"command\"" } }),
        json!({ "index": 0, "function": { "arguments": ": " } }),
        json!({ "index": 0, "function": { "arguments": "\"pwd && ls -la\"" } }),
        json!({ "index": 0, "function": { "arguments": "}" } }),
    ];

    for chunk in &chunks {
        accumulate_classic_tool_call_chunk(&mut pending, chunk);
    }

    let tool_calls = flush_pending_tool_calls(&mut pending);
    assert_eq!(tool_calls.len(), 1);
    assert_eq!(tool_calls[0].id, "call_00_Btv7wxzaKle8PKCgxFTj4416");
    assert_eq!(tool_calls[0].name, "run_terminal");
    assert_eq!(tool_calls[0].arguments["command"], "pwd && ls -la");
}
