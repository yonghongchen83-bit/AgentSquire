//! Headless, no-network, no-GUI verification harness for the "user-input
//! auto-chunking into USR_TN_NNN tokens" gap (`.AiControl/root/Squire/
//! user-input-chunking`). Not a unit test (it deliberately exercises the
//! real `SquireContextAdapter::build_turn_input` call path against a real
//! LanceDB backend rather than one function in isolation) and not wired
//! into `cargo test` — run explicitly:
//!
//!   cargo run --example user_input_chunking_e2e
//!
//! Like `tool_token_ingestion_e2e.rs` (and unlike `ask_user_e2e.rs`), this
//! harness needs no LLM provider/API key/network access: chunking is a
//! deterministic Rust write path (build_turn_input -> ingest_user_input_chunks
//! -> SquireStore), so the strongest, most direct verification available is
//! to run the exact real production code in the exact real order a live turn
//! would, against a real (temp-directory) LanceDB store.
//!
//! Confirms, end to end, against the real `LanceDbSquireStore` backend:
//! 1. A multi-paragraph user message run through the real
//!    `SquireContextAdapter::build_turn_input` produces real `USR_T{turn}_{NNN}`
//!    rows in the store — not just in an isolated call to
//!    `ingest_user_input_chunks` directly.
//! 2. Those rows are immediately discoverable via `SquireExploreTool` (the
//!    exact tool a real model would call) using
//!    `resource_type="system_referential"`, in the *same* turn they were
//!    created in — confirming the ordering requirement (chunk before bootstrap
//!    vector search) actually holds in the real adapter, not just in a
//!    hand-constructed unit test.
//! 3. A second turn's chunk numbering restarts at 001 rather than continuing
//!    the first turn's sequence, confirmed against the real backend.
//! 4. `token_to_detail` resolves a chunk's full text from the store.

use squirecli_lib::agent::context_adapter::ContextManagerAdapter;
use squirecli_lib::agent::squire::{SquireContextAdapter, SquireExploreTool, SquireStore};
use squirecli_lib::agent::{Tool, ToolRegistry};
use squirecli_lib::storage::conversation_store::{
    ContextMode, Message, MessageRole, Session, SessionWithMessages,
};
use squirecli_lib::storage::squire_lancedb::LanceDbSquireStore;
use std::sync::Arc;

fn fixture_session(user_text: &str) -> SessionWithMessages {
    let session_id = uuid::Uuid::new_v4();
    SessionWithMessages {
        session: Session {
            id: session_id,
            title: "user-input-chunking e2e".to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            context_mode: ContextMode::Squire,
        },
        messages: vec![Message {
            id: uuid::Uuid::new_v4(),
            session_id,
            role: MessageRole::User,
            content: user_text.to_string(),
            created_at: chrono::Utc::now(),
            blocks_json: None,
            thinking_content: None,
        }],
    }
}

fn main() {
    // Same LanceDB-under-bare-tokio::main stack-overflow workaround
    // `tool_token_ingestion_e2e.rs` documented and used first — see that
    // file's comment for the full explanation. Not specific to this node's
    // code; LanceDB's own internal async call depth is the cause.
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
    let dir = std::env::temp_dir().join(format!("squire_uic_e2e_{}", uuid::Uuid::new_v4()));
    println!("Using temp LanceDB dir: {}", dir.display());
    let store: Arc<LanceDbSquireStore> = Arc::new(
        LanceDbSquireStore::open(&dir)
            .await
            .expect("LanceDbSquireStore::open should succeed"),
    );

    // ---- Turn 1: multi-paragraph message through the real adapter ----
    let mut adapter = SquireContextAdapter::new(store.clone());
    let session = fixture_session(
        "Please review my project proposal.\n\n\
         The budget section needs another pass before we submit it.",
    );

    println!("\n===== turn 1: build_turn_input on a two-paragraph user message =====");
    let turn_input = adapter
        .build_turn_input(&session, &[])
        .await
        .expect("build_turn_input should succeed");

    assert!(store.token_exists("USR_T0_001").await);
    assert!(store.token_exists("USR_T0_002").await);
    println!("Confirmed real rows exist: USR_T0_001, USR_T0_002");

    let detail = store
        .token_detail("USR_T0_001")
        .await
        .expect("USR_T0_001 should resolve");
    println!("USR_T0_001 full_desc: {:?}", detail.full_desc);
    assert_eq!(
        detail.full_desc.as_deref(),
        Some("Please review my project proposal.")
    );

    // ---- Confirm same-turn bootstrap discoverability via the context block ----
    let marker = "--- Context for this turn ---\n";
    let pos = turn_input.messages[0].content.find(marker)
        .expect("context block should be present");
    let ctx: serde_json::Value = serde_json::from_str(&turn_input.messages[0].content[pos + marker.len()..])
        .expect("context JSON should parse");
    let tokens = ctx["tokens"].as_array()
        .expect("tokens should be an array");
    let chunk_in_prefetch = tokens
        .iter()
        .any(|t| t["token_id"] == "USR_T0_001" || t["token_id"] == "USR_T0_002");
    println!(
        "\n===== tokens includes this turn's own chunk(s): {} =====",
        chunk_in_prefetch
    );
    assert!(
        chunk_in_prefetch,
        "expected at least one of this turn's freshly-created chunks to appear in \
         the same turn's context — confirms chunking runs before the \
         bootstrap explore_memory call, not just that tokens exist afterward"
    );

    // ---- Confirm the exact real explore() tool surfaces them ----
    let explore_tool = SquireExploreTool {
        store: store.clone(),
        tool_registry: Arc::new(ToolRegistry::empty()),
        session_id: session.session.id,
    };
    let explore_result = explore_tool
        .execute(
            "call-1",
            serde_json::json!({"resource_type": "system_referential", "query": "proposal", "max_results": 10}),
        )
        .await;
    println!("\n===== explore(resource_type=\"system_referential\", query=\"proposal\") =====");
    println!("{}", explore_result.output);
    assert!(!explore_result.is_error);
    assert!(explore_result.output.contains("USR_T0_001"));

    // ---- Turn 2: confirm numbering resets per turn against the real store ----
    // Simulate turn close having advanced the counter, matching finalize_turn's
    // real increment_turn call (not otherwise exercised by this harness, which
    // is scoped to build_turn_input/chunking, not the full turn lifecycle).
    store.increment_turn(session.session.id).await;
    let mut adapter2 = SquireContextAdapter::new(store.clone());
    let session2 = fixture_session("A single short follow-up message.");
    // Reuse the same session id so current_turn reflects the increment above.
    let session2 = SessionWithMessages {
        session: Session {
            id: session.session.id,
            ..session2.session
        },
        messages: session2.messages,
    };
    println!("\n===== turn 2: build_turn_input on a single-sentence follow-up =====");
    adapter2
        .build_turn_input(&session2, &[])
        .await
        .expect("build_turn_input should succeed");
    assert!(store.token_exists("USR_T1_001").await);
    assert!(
        !store.token_exists("USR_T1_002").await,
        "turn 2's single-chunk message should restart numbering at 001, not continue turn 1's sequence"
    );
    println!("Confirmed: USR_T1_001 exists, numbering restarted (no USR_T1_002)");

    println!("\n===== summary =====");
    println!("All assertions passed:");
    println!("  - build_turn_input's real call path creates real USR_T{{turn}}_{{NNN}} rows");
    println!("  - a turn's own chunk(s) are bootstrap-discoverable within that same turn");
    println!("  - explore(resource_type=\"system_referential\") surfaces them via the real tool");
    println!("  - per-turn numbering reset holds against the real backend across two turns");

    let _ = std::fs::remove_dir_all(&dir);
}
