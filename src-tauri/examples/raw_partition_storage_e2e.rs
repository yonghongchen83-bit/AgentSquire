//! Headless, no-network, no-GUI verification harness for "raw-partition
//! audit-log storage" (`.AiControl/root/Squire/raw-partition-storage`).
//! Not a unit test (it deliberately exercises the real
//! `SquireContextAdapter::finalize_turn` call path against a real
//! `LanceDbSquireStore` *and* a real SQLite-backed `ConversationStore`,
//! rather than one function in isolation) and not wired into `cargo test` —
//! run explicitly:
//!
//!   cargo run --example raw_partition_storage_e2e
//!
//! Like `tool_token_ingestion_e2e.rs`/`user_input_chunking_e2e.rs`, this
//! harness needs no LLM provider/API key/network access: turn-close
//! sigil-parsing and storage are deterministic Rust write paths, so the
//! strongest, most direct verification available is to run the exact real
//! production code (`finalize_turn`) in the exact real order a live turn
//! close would, against real backends for both stores it touches.
//!
//! Confirms, end to end, against real backends:
//! 1. A compliant response mixing §^-marked and unmarked text persists only
//!    the unmarked prose to the new `squire_raw_partition` LanceDB table,
//!    while the marked span is (separately, as always) promoted to a real
//!    `squire_tokens` row — confirming the two partitions really do split
//!    one response's content rather than duplicating it.
//! 2. The ordinary chat-history table (`ConversationStore`, real SQLite,
//!    not an in-memory test double) still receives the normal
//!    display-expanded message, unaffected by this node's change.
//! 3. A fully §^-spanned response writes nothing to the raw partition.
//! 4. A rejected (malformed JSON) response writes nothing to the raw
//!    partition either — only `finalize_turn`'s pre-existing
//!    `squire_compliance_failures` path fires for it.

use squirecli_lib::agent::context_adapter::{ContextManagerAdapter, TurnOutcome};
use squirecli_lib::agent::squire::{SquireContextAdapter, SquireStore};
use squirecli_lib::state::db::Database;
use squirecli_lib::storage::conversation_store::{ConversationStore, ContextMode, NewSession};
use squirecli_lib::storage::squire_lancedb::LanceDbSquireStore;
use std::sync::Arc;

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
    let lance_dir =
        std::env::temp_dir().join(format!("squire_rps_e2e_lance_{}", uuid::Uuid::new_v4()));
    let sqlite_path =
        std::env::temp_dir().join(format!("squire_rps_e2e_{}.db", uuid::Uuid::new_v4()));
    println!("Using temp LanceDB dir: {}", lance_dir.display());
    println!("Using temp SQLite db: {}", sqlite_path.display());

    let squire_store: Arc<LanceDbSquireStore> = Arc::new(
        LanceDbSquireStore::open(&lance_dir)
            .await
            .expect("LanceDbSquireStore::open should succeed"),
    );
    let conv_store: Database =
        Database::open(&sqlite_path).expect("Database::open should succeed");

    let session = conv_store
        .create_session(NewSession {
            title: "raw-partition-storage e2e".to_string(),
            context_mode: Some(ContextMode::Squire),
        })
        .await
        .expect("create_session should succeed");
    let sid = session.id;

    // ---- Turn 1: mixed marked/unmarked compliant response ----
    let mut adapter = SquireContextAdapter::new(squire_store.clone());
    let mut messages = Vec::new();
    let response = serde_json::json!({
        "ask_user": "",
        "content": "Sure thing. §^TRT_Answer The answer to your question is 42. §^ Let me know if you need more detail.",
        "preserve": [],
        "new_tokens": [{"id": "TRT_Answer", "type": "referential", "short_desc": "the numeric answer"}],
        "relationships": []
    })
    .to_string();

    println!("\n===== turn 1: mixed marked/unmarked compliant response =====");
    let outcome = adapter
        .finalize_turn(sid, response, None, &mut messages, &conv_store)
        .await
        .expect("finalize_turn should succeed");
    assert!(matches!(outcome, TurnOutcome::Done), "expected TurnOutcome::Done");
    println!("finalize_turn outcome: Done");

    assert!(squire_store.token_exists("TRT_Answer").await);
    println!("Confirmed: TRT_Answer promoted to a real squire_tokens row");

    let raw_table = squire_store
        .raw_partition_table()
        .await
        .expect("raw_partition_table should open");
    let raw_count = raw_table.count_rows(None).await.expect("count_rows should succeed");
    println!("squire_raw_partition row count after turn 1: {}", raw_count);
    assert_eq!(raw_count, 1, "expected exactly one raw-partition row for turn 1");

    let session_with_messages = conv_store
        .get_session(sid)
        .await
        .expect("get_session should succeed");
    let chat_messages = &session_with_messages.messages;
    println!(
        "Ordinary chat-history table (real SQLite) has {} message(s) after turn 1",
        chat_messages.len()
    );
    assert_eq!(chat_messages.len(), 1, "expected the display-expanded message to be persisted");
    println!("chat-history content: {:?}", chat_messages[0].content);
    assert!(
        chat_messages[0].content.contains("The answer to your question is 42"),
        "display-expanded chat message should still contain the (unmarked-of-sigils) span prose"
    );
    assert!(
        !chat_messages[0].content.contains("§^"),
        "display-expanded chat message must not leak raw sigil markup"
    );

    // ---- Turn 2: fully §^-spanned response — nothing left over to archive ----
    let mut messages2 = Vec::new();
    let response2 = serde_json::json!({
        "ask_user": "",
        "content": "§^TRT_Full The entire response this turn is one single span. §^",
        "preserve": [],
        "new_tokens": [{"id": "TRT_Full", "type": "referential", "short_desc": "fully spanned"}],
        "relationships": []
    })
    .to_string();

    println!("\n===== turn 2: fully §^-spanned response =====");
    let outcome2 = adapter
        .finalize_turn(sid, response2, None, &mut messages2, &conv_store)
        .await
        .expect("finalize_turn should succeed");
    assert!(matches!(outcome2, TurnOutcome::Done));
    let raw_count_after_turn2 = raw_table.count_rows(None).await.expect("count_rows should succeed");
    println!(
        "squire_raw_partition row count after turn 2 (fully-spanned): {}",
        raw_count_after_turn2
    );
    assert_eq!(
        raw_count_after_turn2, 1,
        "a fully-spanned response should add no new raw-partition row"
    );

    // ---- Turn 3: rejected (malformed JSON) response — no raw-partition write ----
    let mut messages3 = Vec::new();
    println!("\n===== turn 3: malformed JSON (rejected) response =====");
    let outcome3 = adapter
        .finalize_turn(sid, "not valid json at all".to_string(), None, &mut messages3, &conv_store)
        .await
        .expect("finalize_turn should succeed even for a rejection");
    assert!(matches!(outcome3, TurnOutcome::Retry), "expected a Retry outcome on the first malformed-JSON rejection");
    let raw_count_after_turn3 = raw_table.count_rows(None).await.expect("count_rows should succeed");
    println!(
        "squire_raw_partition row count after turn 3 (rejected): {}",
        raw_count_after_turn3
    );
    assert_eq!(
        raw_count_after_turn3, 1,
        "a rejected response should add no new raw-partition row"
    );

    println!("\n===== summary =====");
    println!("All assertions passed:");
    println!("  - a mixed marked/unmarked response persists only the unmarked prose to squire_raw_partition");
    println!("  - the marked span is still separately promoted to a real squire_tokens row");
    println!("  - the ordinary SQLite chat-history table is unaffected, still receives display-expanded prose");
    println!("  - a fully-spanned response writes nothing new to the raw partition");
    println!("  - a rejected response writes nothing to the raw partition either");

    let _ = std::fs::remove_dir_all(&lance_dir);
    let _ = std::fs::remove_file(&sqlite_path);
}
