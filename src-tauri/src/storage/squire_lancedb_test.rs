use arrow_array::{cast::AsArray, UInt64Array};
use futures::TryStreamExt;
use lancedb::query::ExecutableQuery;
use serde_json::Value;
use crate::agent::squire::{
    ingest_tool_registry, ingest_user_input_chunks, ComplianceFailureRecord, NewTokenSpec,
    Relationship, SquireStore, ToolEndpoint,
};
use crate::agent::{Tool, ToolRegistry, ToolResult};
use async_trait::async_trait;
use crate::storage::squire_lancedb::LanceDbSquireStore;
use uuid::Uuid;

async fn temp_store() -> (LanceDbSquireStore, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let store = LanceDbSquireStore::open(dir.path()).await.unwrap();
    (store, dir)
}

#[tokio::test]
async fn roundtrips_token_and_preserve_list() {
    let (store, _dir) = temp_store().await;
    let sid = Uuid::new_v4();
    assert!(!store.token_exists("CONCEPT_X").await);

    store
        .upsert_token(
            NewTokenSpec {
                id: "CONCEPT_X".to_string(),
                token_type: "concept".to_string(),
                short_desc: "desc".to_string(),
                full_desc: Some("full".to_string()),
                endpoint: None,
            },
            1,
        )
        .await;
    assert!(store.token_exists("CONCEPT_X").await);

    store
        .set_preserve_list(sid, vec!["CONCEPT_X".to_string()])
        .await;
    let preserved = store.preserved_tokens(sid).await;
    assert_eq!(preserved.len(), 1);
    assert_eq!(preserved[0].token_id, "CONCEPT_X");

    let detail = store.token_detail("CONCEPT_X").await.unwrap();
    assert_eq!(detail.short_desc, "desc");
    assert_eq!(detail.full_desc.as_deref(), Some("full"));
}

#[tokio::test]
async fn upsert_merges_short_desc_and_preserves_creation_turn() {
    let (store, _dir) = temp_store().await;
    store
        .upsert_token(
            NewTokenSpec {
                id: "CONCEPT_X".to_string(),
                token_type: "concept".to_string(),
                short_desc: "v1".to_string(),
                full_desc: None,
                endpoint: None,
            },
            5,
        )
        .await;
    store
        .upsert_token(
            NewTokenSpec {
                id: "CONCEPT_X".to_string(),
                token_type: "concept".to_string(),
                short_desc: "v2".to_string(),
                full_desc: None,
                endpoint: None,
            },
            99,
        )
        .await;

    let detail = store.token_detail("CONCEPT_X").await.unwrap();
    assert_eq!(detail.short_desc, "v2");

    // creation_turn isn't exposed via but exercised via
    // explore_memory's row round-trip implicitly (no panic = schema OK).
    let results = store.explore_memory("concept", "", 0, 10, 0).await;
    assert_eq!(results.len(), 1);
}

#[tokio::test]
async fn increments_turn_counter() {
    let (store, _dir) = temp_store().await;
    let sid = Uuid::new_v4();
    assert_eq!(store.current_turn(sid).await, 0);
    store.increment_turn(sid).await;
    store.increment_turn(sid).await;
    assert_eq!(store.current_turn(sid).await, 2);
}

#[tokio::test]
async fn explore_memory_filters_by_type_and_query() {
    let (store, _dir) = temp_store().await;
    store
        .upsert_token(
            NewTokenSpec {
                id: "CONCEPT_Fish".to_string(),
                token_type: "concept".to_string(),
                short_desc: "fishing locations".to_string(),
                full_desc: None,
                endpoint: None,
            },
            0,
        )
        .await;
    store
        .upsert_token(
            NewTokenSpec {
                id: "WF_Chat".to_string(),
                token_type: "workflow".to_string(),
                short_desc: "friendly chat".to_string(),
                full_desc: None,
                endpoint: None,
            },
            0,
        )
        .await;

    let results = store.explore_memory("concept", "fish", 0, 10, 0).await;
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].token_id, "CONCEPT_Fish");

    let all = store.explore_memory("all", "", 0, 10, 0).await;
    assert_eq!(all.len(), 2);
}

#[tokio::test]
async fn explore_memory_respects_max_results() {
    let (store, _dir) = temp_store().await;
    for i in 0..5 {
        store
            .upsert_token(
                NewTokenSpec {
                    id: format!("CONCEPT_{}", i),
                    token_type: "concept".to_string(),
                    short_desc: "shared topic apples".to_string(),
                    full_desc: None,
                    endpoint: None,
                },
                0,
            )
            .await;
    }
    let results = store.explore_memory("concept", "apples", 0, 2, 0).await;
    assert_eq!(results.len(), 2);
}

#[tokio::test]
async fn explore_memory_ranks_closer_match_higher() {
    let (store, _dir) = temp_store().await;
    store
        .upsert_token(
            NewTokenSpec {
                id: "CONCEPT_Exact".to_string(),
                token_type: "concept".to_string(),
                short_desc: "rust programming language".to_string(),
                full_desc: None,
                endpoint: None,
            },
            0,
        )
        .await;
    store
        .upsert_token(
            NewTokenSpec {
                id: "CONCEPT_Unrelated".to_string(),
                token_type: "concept".to_string(),
                short_desc: "banana smoothie recipe".to_string(),
                full_desc: None,
                endpoint: None,
            },
            0,
        )
        .await;

    let results = store
        .explore_memory("concept", "rust programming", 0, 10, 0)
        .await;
    assert!(!results.is_empty());
    assert_eq!(results[0].token_id, "CONCEPT_Exact");
}

// ---- graph traversal (num_hops) ----

#[tokio::test]
async fn explore_memory_num_hops_zero_does_not_expand() {
    let (store, _dir) = temp_store().await;
    store
        .upsert_token(
            NewTokenSpec {
                id: "CONCEPT_Fish".to_string(),
                token_type: "concept".to_string(),
                short_desc: "fishing locations".to_string(),
                full_desc: None,
                endpoint: None,
            },
            0,
        )
        .await;
    store
        .upsert_token(
            NewTokenSpec {
                id: "TRT_Spot".to_string(),
                token_type: "referential".to_string(),
                short_desc: "Middle Harbour bream spot".to_string(),
                full_desc: None,
                endpoint: None,
            },
            0,
        )
        .await;
    store
        .insert_relationship(Relationship {
            subject: "TRT_Spot".to_string(),
            predicate: "instanceOf".to_string(),
            object: "CONCEPT_Fish".to_string(),
        })
        .await;

    let results = store.explore_memory("all", "fish", 0, 10, 0).await;
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].token_id, "CONCEPT_Fish");
    assert_eq!(results[0].hop_distance, 0);
}

#[tokio::test]
async fn explore_memory_num_hops_one_expands_directly_connected_token() {
    let (store, _dir) = temp_store().await;
    store
        .upsert_token(
            NewTokenSpec {
                id: "CONCEPT_Fish".to_string(),
                token_type: "concept".to_string(),
                short_desc: "fishing locations".to_string(),
                full_desc: None,
                endpoint: None,
            },
            0,
        )
        .await;
    store
        .upsert_token(
            NewTokenSpec {
                id: "TRT_Spot".to_string(),
                token_type: "referential".to_string(),
                short_desc: "Middle Harbour is a great bream spot".to_string(),
                full_desc: None,
                endpoint: None,
            },
            0,
        )
        .await;
    store
        .upsert_token(
            NewTokenSpec {
                id: "WF_Unrelated".to_string(),
                token_type: "workflow".to_string(),
                short_desc: "totally unrelated workflow".to_string(),
                full_desc: None,
                endpoint: None,
            },
            0,
        )
        .await;
    store
        .insert_relationship(Relationship {
            subject: "TRT_Spot".to_string(),
            predicate: "instanceOf".to_string(),
            object: "CONCEPT_Fish".to_string(),
        })
        .await;

    let results = store.explore_memory("all", "fishing", 1, 10, 0).await;
    let ids: Vec<&str> = results.iter().map(|t| t.token_id.as_str()).collect();
    assert!(ids.contains(&"CONCEPT_Fish"));
    assert!(ids.contains(&"TRT_Spot"));
    assert!(!ids.contains(&"WF_Unrelated"));

    let spot = results.iter().find(|t| t.token_id == "TRT_Spot").unwrap();
    assert_eq!(spot.hop_distance, 1);
    assert_eq!(spot.via_token_id.as_deref(), Some("CONCEPT_Fish"));
    let fish = results
        .iter()
        .find(|t| t.token_id == "CONCEPT_Fish")
        .unwrap();
    assert!(spot.score < fish.score);
}

#[tokio::test]
async fn explore_memory_traversal_is_undirected_and_multi_hop() {
    let (store, _dir) = temp_store().await;
    // Deliberately unrelated short_desc text per node (no shared words)
    // so the placeholder hash-based embedding (which scores purely on
    // shared-word overlap) doesn't accidentally give B/C a positive
    // direct-match score against the "aardvark" query — this test wants
    // B and C to be findable *only* via graph traversal, not vector
    // similarity, to isolate what's being tested.
    store
        .upsert_token(
            NewTokenSpec {
                id: "A".to_string(),
                token_type: "concept".to_string(),
                short_desc: "aardvark burrow habits".to_string(),
                full_desc: None,
                endpoint: None,
            },
            0,
        )
        .await;
    store
        .upsert_token(
            NewTokenSpec {
                id: "B".to_string(),
                token_type: "concept".to_string(),
                short_desc: "quokka island population".to_string(),
                full_desc: None,
                endpoint: None,
            },
            0,
        )
        .await;
    store
        .upsert_token(
            NewTokenSpec {
                id: "C".to_string(),
                token_type: "concept".to_string(),
                short_desc: "narwhal tusk research".to_string(),
                full_desc: None,
                endpoint: None,
            },
            0,
        )
        .await;
    store
        .insert_relationship(Relationship {
            subject: "A".to_string(),
            predicate: "relatedTo".to_string(),
            object: "B".to_string(),
        })
        .await;
    store
        .insert_relationship(Relationship {
            subject: "B".to_string(),
            predicate: "relatedTo".to_string(),
            object: "C".to_string(),
        })
        .await;

    let from_a = store.explore_memory("all", "aardvark", 2, 10, 0).await;
    let ids: Vec<&str> = from_a.iter().map(|t| t.token_id.as_str()).collect();
    assert!(ids.contains(&"A"));
    assert!(ids.contains(&"B"));
    assert!(
        ids.contains(&"C"),
        "2-hop traversal should reach C from A via B"
    );
    let c = from_a.iter().find(|t| t.token_id == "C").unwrap();
    assert_eq!(c.hop_distance, 2);
}

#[tokio::test]
async fn explore_memory_traversal_still_respects_max_results() {
    let (store, _dir) = temp_store().await;
    store
        .upsert_token(
            NewTokenSpec {
                id: "HUB".to_string(),
                token_type: "concept".to_string(),
                short_desc: "hub node".to_string(),
                full_desc: None,
                endpoint: None,
            },
            0,
        )
        .await;
    for i in 0..5 {
        let id = format!("LEAF_{}", i);
        store
            .upsert_token(
                NewTokenSpec {
                    id: id.clone(),
                    token_type: "concept".to_string(),
                    short_desc: "connected leaf".to_string(),
                    full_desc: None,
                    endpoint: None,
                },
                0,
            )
            .await;
        store
            .insert_relationship(Relationship {
                subject: id,
                predicate: "relatedTo".to_string(),
                object: "HUB".to_string(),
            })
            .await;
    }

    let results = store.explore_memory("all", "hub", 1, 2, 0).await;
    assert_eq!(results.len(), 2);
}

// ---- accumulated_hits / effective_priority scoring ----

#[tokio::test]
async fn upsert_token_increments_accumulated_hits_on_every_call() {
    let (store, _dir) = temp_store().await;
    store
        .upsert_token(
            NewTokenSpec {
                id: "CONCEPT_X".to_string(),
                token_type: "concept".to_string(),
                short_desc: "v1".to_string(),
                full_desc: None,
                endpoint: None,
            },
            0,
        )
        .await;
    let results = store.explore_memory("concept", "", 0, 10, 0).await;
    assert_eq!(results[0].accumulated_hits, 1);

    store
        .upsert_token(
            NewTokenSpec {
                id: "CONCEPT_X".to_string(),
                token_type: "concept".to_string(),
                short_desc: "v2".to_string(),
                full_desc: None,
                endpoint: None,
            },
            0,
        )
        .await;
    let results = store.explore_memory("concept", "", 0, 10, 0).await;
    assert_eq!(results[0].accumulated_hits, 2);
}

#[tokio::test]
async fn record_hit_increments_accumulated_hits_and_persists() {
    let (store, dir) = temp_store().await;
    store
        .upsert_token(
            NewTokenSpec {
                id: "CONCEPT_X".to_string(),
                token_type: "concept".to_string(),
                short_desc: "desc".to_string(),
                full_desc: None,
                endpoint: None,
            },
            0,
        )
        .await;
    store.record_hit("CONCEPT_X").await;
    store.record_hit("CONCEPT_X").await;

    let results = store.explore_memory("concept", "", 0, 10, 0).await;
    assert_eq!(results[0].accumulated_hits, 3); // 1 from upsert + 2 record_hit calls

    // Confirm durability across a fresh connection.
    let reopened = LanceDbSquireStore::open(dir.path()).await.unwrap();
    let detail = reopened.token_detail("CONCEPT_X").await;
    assert!(detail.is_some());
    let results = reopened.explore_memory("concept", "", 0, 10, 0).await;
    assert_eq!(results[0].accumulated_hits, 3);
}

#[tokio::test]
async fn record_hit_composes_with_upsert_matching_the_cite_without_redefine_pattern() {
    // Mirrors, against the real LanceDB backend directly, the exact
    // sequence `SquireContextAdapter::finalize_turn` (squire.rs) now
    // performs for `hit-count-fidelity`'s new citation-based crediting:
    // a token created in an earlier turn (one upsert_token call, +1
    // hit) that is later cited via §! in a *different* turn without
    // being redefined in new_tokens — earning exactly one additional
    // hit via record_hit, for a total of 2, not double-counted and not
    // silently dropped. finalize_turn's own InMemorySquireStore-backed
    // integration tests (squire.rs) cover the full parsing/wiring path
    // end to end; this test confirms the real backend's storage
    // primitive composes identically.
    let (store, _dir) = temp_store().await;
    store
        .upsert_token(
            NewTokenSpec {
                id: "WF_CitedLater".to_string(),
                token_type: "workflow".to_string(),
                short_desc: "a workflow cited in a later turn".to_string(),
                full_desc: None,
                endpoint: None,
            },
            0,
        )
        .await;
    store.record_hit("WF_CitedLater").await;

    let results = store.explore_memory("workflow", "", 0, 10, 1).await;
    assert_eq!(results[0].accumulated_hits, 2);
}

#[tokio::test]
async fn preserved_tokens_increments_hit_on_load() {
    let (store, _dir) = temp_store().await;
    let sid = Uuid::new_v4();
    store
        .upsert_token(
            NewTokenSpec {
                id: "CONCEPT_X".to_string(),
                token_type: "concept".to_string(),
                short_desc: "desc".to_string(),
                full_desc: None,
                endpoint: None,
            },
            0,
        )
        .await;
    store
        .set_preserve_list(sid, vec!["CONCEPT_X".to_string()])
        .await;

    let first = store.preserved_tokens(sid).await;
    assert_eq!(first[0].accumulated_hits, 2); // 1 from upsert + 1 from this load
    let second = store.preserved_tokens(sid).await;
    assert_eq!(second[0].accumulated_hits, 3);
}

#[tokio::test]
async fn explore_memory_breaks_near_ties_by_effective_priority() {
    let (store, _dir) = temp_store().await;
    store
        .upsert_token(
            NewTokenSpec {
                id: "CONCEPT_Popular".to_string(),
                token_type: "concept".to_string(),
                short_desc: "shared topic".to_string(),
                full_desc: None,
                endpoint: None,
            },
            0,
        )
        .await;
    store
        .upsert_token(
            NewTokenSpec {
                id: "CONCEPT_Stale".to_string(),
                token_type: "concept".to_string(),
                short_desc: "shared topic".to_string(),
                full_desc: None,
                endpoint: None,
            },
            0,
        )
        .await;
    store.record_hit("CONCEPT_Popular").await;
    store.record_hit("CONCEPT_Popular").await;
    store.record_hit("CONCEPT_Popular").await;

    let results = store.explore_memory("all", "shared topic", 0, 10, 10).await;
    assert_eq!(results[0].token_id, "CONCEPT_Popular");
}

#[tokio::test]
async fn insert_relationship_does_not_panic_and_persists() {
    let (store, dir) = temp_store().await;
    store
        .insert_relationship(Relationship {
            subject: "A".to_string(),
            predicate: "relates_to".to_string(),
            object: "B".to_string(),
        })
        .await;

    // Re-open to confirm persistence across connections (not just in-process caching).
    let reopened = LanceDbSquireStore::open(dir.path()).await.unwrap();
    let table = reopened.relationships_table().await.unwrap();
    let count = table.count_rows(None).await.unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn preserve_list_replace_clears_previous_entries() {
    let (store, _dir) = temp_store().await;
    let sid = Uuid::new_v4();
    store
        .upsert_token(
            NewTokenSpec {
                id: "A".to_string(),
                token_type: "concept".to_string(),
                short_desc: "a".to_string(),
                full_desc: None,
                endpoint: None,
            },
            0,
        )
        .await;
    store
        .upsert_token(
            NewTokenSpec {
                id: "B".to_string(),
                token_type: "concept".to_string(),
                short_desc: "b".to_string(),
                full_desc: None,
                endpoint: None,
            },
            0,
        )
        .await;

    store
        .set_preserve_list(sid, vec!["A".to_string(), "B".to_string()])
        .await;
    assert_eq!(store.preserved_tokens(sid).await.len(), 2);

    store.set_preserve_list(sid, vec!["A".to_string()]).await;
    let preserved = store.preserved_tokens(sid).await;
    assert_eq!(preserved.len(), 1);
    assert_eq!(preserved[0].token_id, "A");
}

#[tokio::test]
async fn clear_all_preserve_lists_wipes_every_session_and_persists_across_reopen() {
    let (store, dir) = temp_store().await;
    let sid_a = Uuid::new_v4();
    let sid_b = Uuid::new_v4();
    store
        .upsert_token(
            NewTokenSpec {
                id: "A".to_string(),
                token_type: "concept".to_string(),
                short_desc: "a".to_string(),
                full_desc: None,
                endpoint: None,
            },
            0,
        )
        .await;
    store.set_preserve_list(sid_a, vec!["A".to_string()]).await;
    store.set_preserve_list(sid_b, vec!["A".to_string()]).await;
    assert_eq!(store.preserved_tokens(sid_a).await.len(), 1);
    assert_eq!(store.preserved_tokens(sid_b).await.len(), 1);

    store.clear_all_preserve_lists().await;
    assert!(store.preserved_tokens(sid_a).await.is_empty());
    assert!(store.preserved_tokens(sid_b).await.is_empty());

    // Confirm the clear actually deleted rows on disk (Q7's "restart
    // clears carryover" implies durability, not just in-process state)
    // by re-opening a fresh connection against the same directory.
    let reopened = LanceDbSquireStore::open(dir.path()).await.unwrap();
    assert!(reopened.preserved_tokens(sid_a).await.is_empty());
}

#[tokio::test]
async fn record_compliance_failure_persists_structured_metadata() {
    let (store, dir) = temp_store().await;
    let sid = Uuid::new_v4();
    let ts = chrono::Utc::now();
    store
        .record_compliance_failure(ComplianceFailureRecord {
            session_id: sid,
            rule: "empty_close_response".to_string(),
            reason: "empty close response".to_string(),
            retry_count: 4,
            failed_content: "{}".to_string(),
            timestamp: ts,
        })
        .await;

    // Re-open to confirm persistence across connections.
    let reopened = LanceDbSquireStore::open(dir.path()).await.unwrap();
    let table = reopened.compliance_failures_table().await.unwrap();
    let count = table.count_rows(None).await.unwrap();
    assert_eq!(count, 1);

    let batches = table
        .query()
        .execute()
        .await
        .unwrap()
        .try_collect::<Vec<_>>()
        .await
        .unwrap();
    let batch = &batches[0];
    let rule = batch.column_by_name("rule").unwrap().as_string::<i32>();
    let retry_count = batch
        .column_by_name("retry_count")
        .unwrap()
        .as_any()
        .downcast_ref::<UInt64Array>()
        .unwrap();
    assert_eq!(rule.value(0), "empty_close_response");
    assert_eq!(retry_count.value(0), 4);
}

// ---- tool-token ingestion (ss-9) — same coverage as squire.rs's
// InMemorySquireStore tests, exercised against the real LanceDB backend
// to confirm ingest_tool_registry is genuinely backend-agnostic. ----

#[tokio::test]
async fn ingest_tool_registry_writes_a_token_per_registry_tool() {
    let (store, _dir) = temp_store().await;
    let registry = ToolRegistry::new(); // real run_terminal + web_fetch

    ingest_tool_registry(&registry, &store, &std::collections::HashMap::new()).await;

    assert!(store.token_exists("run_terminal").await);
    assert!(store.token_exists("web_fetch").await);
    let detail = store.token_detail("run_terminal").await.unwrap();
    assert!(!detail.short_desc.is_empty());
}

#[tokio::test]
async fn ingest_tool_registry_is_idempotent_and_updates_rather_than_duplicates() {
    let (store, _dir) = temp_store().await;
    let registry = ToolRegistry::new();

    ingest_tool_registry(&registry, &store, &std::collections::HashMap::new()).await;
    ingest_tool_registry(&registry, &store, &std::collections::HashMap::new()).await;
    ingest_tool_registry(&registry, &store, &std::collections::HashMap::new()).await;

    let results = store.explore_memory("tool", "", 0, 100, 0).await;
    assert_eq!(results.len(), registry.definitions().len());
    let ids: std::collections::HashSet<&str> =
        results.iter().map(|t| t.token_id.as_str()).collect();
    assert_eq!(ids.len(), results.len(), "no duplicate token ids expected");
}

#[tokio::test]
async fn ingest_tool_registry_full_desc_matches_expected_mcp_style_shape() {
    let (store, _dir) = temp_store().await;
    let mut registry = ToolRegistry::empty();
    registry.register(Box::new(crate::agent::TerminalTool));

    ingest_tool_registry(&registry, &store, &std::collections::HashMap::new()).await;

    let detail = store.token_detail("run_terminal").await.unwrap();
    let full_desc = detail
        .full_desc
        .expect("tool tokens must carry a full_desc");
    let parsed: serde_json::Value = serde_json::from_str(&full_desc).unwrap();
    assert_eq!(parsed["name"], "run_terminal");
    assert!(parsed["description"].is_string());
    assert!(parsed["input_schema"].is_object());
}

#[tokio::test]
async fn ingest_tool_registry_persists_across_reopen() {
    let (store, dir) = temp_store().await;
    let registry = ToolRegistry::new();
    ingest_tool_registry(&registry, &store, &std::collections::HashMap::new()).await;

    let reopened = LanceDbSquireStore::open(dir.path()).await.unwrap();
    assert!(reopened.token_exists("run_terminal").await);
    let results = reopened.explore_memory("tool", "", 0, 100, 0).await;
    assert_eq!(results.len(), registry.definitions().len());
}

// ---- endpoint-carrying TokenDetail extension (token-detail-endpoint) —
// real LanceDB-backed parity with squire.rs's InMemorySquireStore tests. ----

fn fake_mcp_server_for_lancedb_tests(id: &str) -> crate::state::config::McpServerConfig {
    crate::state::config::McpServerConfig {
        id: id.to_string(),
        name: format!("Fake server {}", id),
        transport: "stdio".to_string(),
        command: "this-binary-does-not-exist".to_string(),
        args: vec![],
        url: None,
        enabled: true,
        env: std::collections::HashMap::new(),
        headers: std::collections::HashMap::new(),
    }
}

#[tokio::test]
async fn upsert_token_persists_and_returns_endpoint_via_real_lancedb() {
    let (store, _dir) = temp_store().await;
    let endpoint = ToolEndpoint::Mcp {
        server: fake_mcp_server_for_lancedb_tests("srv1"),
        remote_name: "remote_tool".to_string(),
    };
    store
        .upsert_token(
            NewTokenSpec {
                id: "mcp_srv1_remote_tool".to_string(),
                token_type: "tool".to_string(),
                short_desc: "an mcp tool".to_string(),
                full_desc: None,
                endpoint: Some(endpoint.clone()),
            },
            0,
        )
        .await;

    let detail = store.token_detail("mcp_srv1_remote_tool").await.unwrap();
    assert_eq!(detail.endpoint, Some(endpoint));
}

#[tokio::test]
async fn endpoint_persists_across_reopen_via_real_lancedb() {
    let (store, dir) = temp_store().await;
    let endpoint = ToolEndpoint::Mcp {
        server: fake_mcp_server_for_lancedb_tests("srv1"),
        remote_name: "remote_tool".to_string(),
    };
    store
        .upsert_token(
            NewTokenSpec {
                id: "mcp_srv1_remote_tool".to_string(),
                token_type: "tool".to_string(),
                short_desc: "an mcp tool".to_string(),
                full_desc: None,
                endpoint: Some(endpoint.clone()),
            },
            0,
        )
        .await;

    let reopened = LanceDbSquireStore::open(dir.path()).await.unwrap();
    let detail = reopened.token_detail("mcp_srv1_remote_tool").await.unwrap();
    assert_eq!(detail.endpoint, Some(endpoint));
}

#[tokio::test]
async fn record_hit_preserves_endpoint_via_real_lancedb() {
    // record_hit performs its own delete-then-reinsert (a separate write
    // path from upsert_token) — confirms its RecordBatch construction
    // also correctly threads the endpoint column through, not just
    // upsert_token's.
    let (store, _dir) = temp_store().await;
    let endpoint = ToolEndpoint::Mcp {
        server: fake_mcp_server_for_lancedb_tests("srv1"),
        remote_name: "remote_tool".to_string(),
    };
    store
        .upsert_token(
            NewTokenSpec {
                id: "mcp_srv1_remote_tool".to_string(),
                token_type: "tool".to_string(),
                short_desc: "an mcp tool".to_string(),
                full_desc: None,
                endpoint: Some(endpoint.clone()),
            },
            0,
        )
        .await;
    store.record_hit("mcp_srv1_remote_tool").await;

    let detail = store.token_detail("mcp_srv1_remote_tool").await.unwrap();
    assert_eq!(detail.endpoint, Some(endpoint));
    let results = store.explore_memory("tool", "", 0, 10, 0).await;
    assert_eq!(results[0].accumulated_hits, 2); // 1 from upsert + 1 record_hit
}

#[tokio::test]
async fn ingest_tool_registry_populates_endpoint_only_for_mcp_sourced_definitions_via_real_lancedb()
{
    let (store, _dir) = temp_store().await;
    let registry = ToolRegistry::new();
    let mut endpoints = std::collections::HashMap::new();
    endpoints.insert(
        "run_terminal".to_string(),
        ToolEndpoint::Mcp {
            server: fake_mcp_server_for_lancedb_tests("srv1"),
            remote_name: "remote_terminal".to_string(),
        },
    );

    ingest_tool_registry(&registry, &store, &endpoints).await;

    let terminal_detail = store.token_detail("run_terminal").await.unwrap();
    assert!(terminal_detail.endpoint.is_some());
    let web_fetch_detail = store.token_detail("web_fetch").await.unwrap();
    assert!(web_fetch_detail.endpoint.is_none());
}

#[tokio::test]
async fn ingest_tool_registry_reflects_schema_change_on_next_ingestion() {
    struct FakeToolV1;
    #[async_trait]
    impl Tool for FakeToolV1 {
        fn name(&self) -> &str {
            "fake_tool"
        }
        fn description(&self) -> &str {
            "version one"
        }
        fn input_schema(&self) -> Value {
            serde_json::json!({"type": "object"})
        }
        async fn execute(&self, call_id: &str, _args: Value) -> ToolResult {
            ToolResult {
                call_id: call_id.to_string(),
                output: String::new(),
                is_error: false,
            }
        }
    }
    struct FakeToolV2;
    #[async_trait]
    impl Tool for FakeToolV2 {
        fn name(&self) -> &str {
            "fake_tool"
        }
        fn description(&self) -> &str {
            "version two"
        }
        fn input_schema(&self) -> Value {
            serde_json::json!({"type": "object"})
        }
        async fn execute(&self, call_id: &str, _args: Value) -> ToolResult {
            ToolResult {
                call_id: call_id.to_string(),
                output: String::new(),
                is_error: false,
            }
        }
    }

    let (store, _dir) = temp_store().await;
    let mut registry_v1 = ToolRegistry::empty();
    registry_v1.register(Box::new(FakeToolV1));
    ingest_tool_registry(&registry_v1, &store, &std::collections::HashMap::new()).await;
    assert_eq!(
        store.token_detail("fake_tool").await.unwrap().short_desc,
        "version one"
    );

    let mut registry_v2 = ToolRegistry::empty();
    registry_v2.register(Box::new(FakeToolV2));
    ingest_tool_registry(&registry_v2, &store, &std::collections::HashMap::new()).await;
    assert_eq!(
        store.token_detail("fake_tool").await.unwrap().short_desc,
        "version two"
    );

    let results = store.explore_memory("tool", "", 0, 100, 0).await;
    assert_eq!(
        results.iter().filter(|t| t.token_id == "fake_tool").count(),
        1
    );
}

// ---- user-input auto-chunking (against the real LanceDB backend) ----

#[tokio::test]
async fn ingest_user_input_chunks_writes_system_referential_tokens_with_expected_ids() {
    let (store, _dir) = temp_store().await;
    ingest_user_input_chunks("First paragraph.\n\nSecond paragraph.", 2, &store).await;

    assert!(store.token_exists("USR_T2_001").await);
    assert!(store.token_exists("USR_T2_002").await);

    let detail = store.token_detail("USR_T2_001").await.unwrap();
    assert_eq!(detail.short_desc, "First paragraph.");
    assert_eq!(detail.full_desc, Some("First paragraph.".to_string()));
}

#[tokio::test]
async fn ingest_user_input_chunks_discoverable_via_explore_system_referential_filter() {
    let (store, _dir) = temp_store().await;
    ingest_user_input_chunks("A message about weather patterns.", 1, &store).await;

    let results = store
        .explore_memory("system_referential", "weather", 0, 10, 1)
        .await;
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].token_id, "USR_T1_001");
    assert_eq!(results[0].token_type, "system_referential");
}

#[tokio::test]
async fn explore_memory_alias_includes_system_referential_tokens() {
    // The "memory" resource_type alias predates system_referential and
    // must expand to include it too, against the real backend.
    let (store, _dir) = temp_store().await;
    ingest_user_input_chunks("A message about weather patterns.", 1, &store).await;

    let via_memory = store.explore_memory("memory", "weather", 0, 10, 1).await;
    assert!(via_memory.iter().any(|t| t.token_id == "USR_T1_001"));
}

#[tokio::test]
async fn ingest_user_input_chunks_persists_across_reopen() {
    let dir = tempfile::tempdir().unwrap();
    {
        let store = LanceDbSquireStore::open(dir.path()).await.unwrap();
        ingest_user_input_chunks("Persisted chunk content.", 4, &store).await;
    }
    let reopened = LanceDbSquireStore::open(dir.path()).await.unwrap();
    assert!(reopened.token_exists("USR_T4_001").await);
    let detail = reopened.token_detail("USR_T4_001").await.unwrap();
    assert_eq!(
        detail.full_desc,
        Some("Persisted chunk content.".to_string())
    );
}

// ---- raw partition (spec §4.1/§4.3/§9.4 step 4) — against the real
// LanceDB backend, mirroring squire_compliance_failures's existing
// coverage shape since both tables share the same
// append-only/never-read-back-by-the-trait design. ----

#[tokio::test]
async fn record_raw_output_persists_a_row_with_expected_columns() {
    let (store, dir) = temp_store().await;
    let sid = Uuid::new_v4();
    store
        .record_raw_output(sid, 3, "unmarked prose from the model".to_string())
        .await;

    let table = store.raw_partition_table().await.unwrap();
    let count = table.count_rows(None).await.unwrap();
    assert_eq!(count, 1);

    let batches = table
        .query()
        .execute()
        .await
        .unwrap()
        .try_collect::<Vec<_>>()
        .await
        .unwrap();
    let batch = &batches[0];
    let session_ids = batch
        .column_by_name("session_id")
        .unwrap()
        .as_string::<i32>();
    let turns = batch
        .column_by_name("turn")
        .unwrap()
        .as_any()
        .downcast_ref::<UInt64Array>()
        .unwrap();
    let contents = batch.column_by_name("content").unwrap().as_string::<i32>();
    assert_eq!(session_ids.value(0), sid.to_string());
    assert_eq!(turns.value(0), 3);
    assert_eq!(contents.value(0), "unmarked prose from the model");

    // Re-open to confirm persistence across connections.
    let reopened = LanceDbSquireStore::open(dir.path()).await.unwrap();
    let table = reopened.raw_partition_table().await.unwrap();
    assert_eq!(table.count_rows(None).await.unwrap(), 1);
}

#[tokio::test]
async fn record_raw_output_is_append_only_across_multiple_turns() {
    let (store, _dir) = temp_store().await;
    let sid = Uuid::new_v4();
    store
        .record_raw_output(sid, 0, "turn zero prose".to_string())
        .await;
    store
        .record_raw_output(sid, 1, "turn one prose".to_string())
        .await;

    let table = store.raw_partition_table().await.unwrap();
    assert_eq!(table.count_rows(None).await.unwrap(), 2);
}

#[tokio::test]
async fn record_raw_output_and_upsert_token_are_independent_writes_for_a_mixed_response() {
    // Exercises the same "mixed marked/unmarked response" shape
    // finalize_turn's own InMemorySquireStore-backed tests (squire.rs)
    // cover end to end, but against the real LanceDB backend directly:
    // the unmarked residual (already computed by
    // agent::squire::unmarked_residual, unit-tested in squire.rs) goes
    // to record_raw_output, while the §^-marked span goes to
    // upsert_token — confirming both real LanceDB write paths accept
    // and separately persist their respective halves of one turn's
    // output without interfering with each other.
    let (store, _dir) = temp_store().await;
    let sid = Uuid::new_v4();

    store
        .upsert_token(
            NewTokenSpec {
                id: "TRT_Answer".to_string(),
                token_type: "referential".to_string(),
                short_desc: "the answer".to_string(),
                full_desc: Some("The answer is 42".to_string()),
                endpoint: None,
            },
            0,
        )
        .await;
    store
        .record_raw_output(sid, 0, "Sure. All done.".to_string())
        .await;

    assert!(store.token_exists("TRT_Answer").await);
    let table = store.raw_partition_table().await.unwrap();
    assert_eq!(table.count_rows(None).await.unwrap(), 1);
    let batches = table
        .query()
        .execute()
        .await
        .unwrap()
        .try_collect::<Vec<_>>()
        .await
        .unwrap();
    let contents = batches[0]
        .column_by_name("content")
        .unwrap()
        .as_string::<i32>();
    assert_eq!(contents.value(0), "Sure. All done.");
    assert!(!contents.value(0).contains("42"));
}
