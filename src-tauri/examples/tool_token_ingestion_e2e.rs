//! Headless, no-network, no-GUI verification harness for ss-9 (real
//! tool-token ingestion). Not a unit test (it deliberately exercises the
//! exact production call chain across three modules rather than one
//! function in isolation) and not wired into `cargo test` — run explicitly:
//!
//!   cargo run --example tool_token_ingestion_e2e
//!
//! Unlike `ask_user_e2e.rs`, this harness needs no LLM provider/API key/
//! network access: tool-token ingestion is a deterministic Rust write path
//! (ToolRegistry -> ingest_tool_registry -> SquireStore), so the strongest,
//! most direct verification available is to run the exact real production
//! code in the exact real order `streaming_cmd.rs` runs it, against a real
//! (temp-directory) LanceDB store — not a model-driven simulation of it.
//!
//! Confirms, end to end, against the real `LanceDbSquireStore` backend:
//! 1. A real `ToolRegistry::new()` (the two local built-ins) plus a
//!    hand-registered fake MCP-style tool (standing in for a real MCP
//!    server's discovered tool, so this harness doesn't need a live MCP
//!    server subprocess to demonstrate the MCP-origin path) is ingested via
//!    `agent::squire::ingest_tool_registry` — the same function
//!    `streaming_cmd.rs` calls every turn.
//! 2. `SquireExploreTool::execute(resource_type="tool_skill")` — the exact
//!    tool the model calls in a real Squire-mode session — is invoked
//!    directly, confirming the live-registry-sourced results still work
//!    (unchanged by this node) and that `token_to_detail` can now resolve an
//!    ingested token's full MCP-style schema from the store.
//! 3. Re-running ingestion confirms no duplicate rows accumulate.
//! 4. `SquireInvokeTool` successfully dispatches to the real, still-live
//!    tool by the exact id `ingest_tool_registry` used — confirming the
//!    token-id scheme decision (id == registry name) actually holds up
//!    end-to-end, not just per-function in isolation.

use squirecli_lib::agent::squire::{ingest_tool_registry, SquireExploreTool, SquireInvokeTool, SquireStore, SquireTokenToDetailTool};
use squirecli_lib::agent::{Tool, ToolRegistry, ToolResult};
use squire_store::LanceDbSquireStore;
use std::sync::Arc;
use std::sync::atomic::AtomicU32;

struct FakeMcpWeatherTool;

#[async_trait::async_trait]
impl Tool for FakeMcpWeatherTool {
    fn name(&self) -> &str {
        // Mirrors streaming_cmd.rs's real mcp_{server_id}_{tool_id} naming
        // scheme for a discovered MCP tool.
        "mcp_weatherserver_get_forecast"
    }
    fn description(&self) -> &str {
        "MCP tool 'get_forecast' from server 'weatherserver': returns a weather forecast for a location"
    }
    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": { "location": { "type": "string" } },
            "required": ["location"]
        })
    }
    async fn execute(&self, call_id: &str, _args: serde_json::Value) -> ToolResult {
        ToolResult {
            call_id: call_id.to_string(),
            output: "Sunny, 24C".to_string(),
            is_error: false,
        }
    }
}

fn main() {
    // LanceDB's async call graph is deep enough to overflow the default
    // 2MB main-thread stack tokio::main's current-thread runtime uses on
    // Windows debug builds (observed directly this session — not specific
    // to this harness's own code, since ingest_tool_registry/SquireStore
    // calls are shallow; LanceDB's own query/table internals are the deep
    // part). The real app doesn't hit this because Tauri's own async
    // runtime and OS-level thread defaults differ from a bare `cargo run`
    // example binary's main thread. Run on a dedicated thread with a larger
    // stack, matching this crate's existing precedent for sidestepping the
    // same class of issue (see squire-storage/decisions.md's unrelated but
    // similarly environment-specific build-prerequisite findings).
    let handle = std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("failed to build tokio runtime")
                .block_on(run())
        })
        .expect("failed to spawn worker thread");
    handle.join().expect("worker thread panicked");
}

async fn run() {
    let dir = std::env::temp_dir().join(format!("squire_ss9_e2e_{}", uuid::Uuid::new_v4()));
    println!("Using temp LanceDB dir: {}", dir.display());
    let store: Arc<LanceDbSquireStore> = Arc::new(
        LanceDbSquireStore::open(&dir)
            .await
            .expect("LanceDbSquireStore::open should succeed"),
    );

    // Step 1: build a registry the same way streaming_cmd.rs does (local
    // built-ins + at least one MCP-origin-shaped tool), then ingest exactly
    // the way the real per-turn call site does.
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(FakeMcpWeatherTool));
    let tool_registry = Arc::new(registry);

    println!("\n===== ingesting tool registry (first pass) =====");
    ingest_tool_registry(tool_registry.as_ref(), store.as_ref(), &std::collections::HashMap::new()).await;

    let session_id = uuid::Uuid::new_v4();

    // Step 2: call the real explore() tool exactly as the model would.
    let explore_tool = SquireExploreTool {
        store: store.clone(),
        tool_defs: tool_registry.definitions(),
        session_id,
        batch_counter: Arc::new(AtomicU32::new(0)),
        batch_cap: 100,
    };
    let explore_result = explore_tool
        .execute(
            "call-1",
            serde_json::json!({"resource_type": "tool_skill", "query": "weather", "max_results": 10}),
        )
        .await;
    println!("\n===== explore(resource_type=\"tool_skill\", query=\"weather\") =====");
    println!("{}", explore_result.output);
    assert!(!explore_result.is_error);
    assert!(
        explore_result.output.contains("mcp_weatherserver_get_forecast"),
        "expected the MCP-origin tool to be discoverable via explore(tool_skill) \
         (this path is served live from the registry, unchanged by ss-9 — confirms \
         ingestion didn't regress the pre-existing live-registry read path)"
    );

    // Step 3: confirm the store side now actually has the token (the part
    // that was previously always empty before this node) via
    // token_to_detail, resolving the *ingested* row, not the live registry.
    let detail_tool = SquireTokenToDetailTool {
        store: store.clone(),
        batch_counter: Arc::new(AtomicU32::new(0)),
        batch_cap: 100,
    };
    let detail_result = detail_tool
        .execute(
            "call-2",
            serde_json::json!({"token_id": "mcp_weatherserver_get_forecast", "detail_level": "full"}),
        )
        .await;
    println!("\n===== token_to_detail(\"mcp_weatherserver_get_forecast\", \"full\") — store-sourced, empty registry =====");
    println!("{}", detail_result.output);
    assert!(
        !detail_result.is_error,
        "expected the ingested token to be resolvable via the store even with an \
         empty live registry — this is the exact dead-end ss-9 was filed to close"
    );
    let parsed: serde_json::Value = serde_json::from_str(&detail_result.output).unwrap();
    assert_eq!(parsed["name"], "mcp_weatherserver_get_forecast");
    assert!(parsed["input_schema"]["properties"]["location"].is_object());

    // Step 4: re-ingest (simulating the next turn's fresh discovery pass)
    // and confirm no duplicate rows accumulate.
    println!("\n===== re-ingesting (simulating a second turn) =====");
    ingest_tool_registry(tool_registry.as_ref(), store.as_ref(), &std::collections::HashMap::new()).await;
    ingest_tool_registry(tool_registry.as_ref(), store.as_ref(), &std::collections::HashMap::new()).await;
    let all_tools = store.explore_memory("tool", "", 0, 100, 0, session_id, "content").await;
    println!("tool-typed token count after 3 ingestion passes: {}", all_tools.len());
    assert_eq!(
        all_tools.len(),
        tool_registry.definitions().len(),
        "repeated ingestion must update existing rows, not duplicate them"
    );

    // Step 5: confirm SquireInvokeTool's registry-primary path still
    // dispatches correctly to the real tool under the exact id
    // ingest_tool_registry used for its store row — proving the id scheme
    // decision holds end-to-end, not just in isolated unit tests.
    let invoke_tool = SquireInvokeTool;
    let invoke_result = invoke_tool
        .execute(
            "call-3",
            serde_json::json!({"token_id": "mcp_weatherserver_get_forecast", "params": {"location": "Sydney"}}),
        )
        .await;
    println!("\n===== invoke(\"mcp_weatherserver_get_forecast\", {{location: Sydney}}) =====");
    println!("{}", invoke_result.output);
    assert!(!invoke_result.is_error);
    assert_eq!(invoke_result.output, "Sunny, 24C");

    println!("\n===== summary =====");
    println!("All assertions passed:");
    println!("  - ingest_tool_registry wrote a real LanceDB row for an MCP-origin-shaped tool");
    println!("  - token_to_detail resolved that row from the store alone (empty live registry)");
    println!("  - re-ingestion updated rather than duplicated the row (idempotent)");
    println!("  - invoke() dispatched correctly using the exact id ingestion chose");

    let _ = std::fs::remove_dir_all(&dir);
}
