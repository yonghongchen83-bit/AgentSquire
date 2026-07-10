use super::*;
use crate::agent::context_adapter::ContextManagerAdapter;
use crate::agent::{Tool, ToolRegistry, ToolResult};
use crate::llm::provider::{ChatRole, ToolDefinition};
use crate::state::config::SquirePrefetchConfig;
use crate::storage::conversation_store::{
    ContextMode, ConversationStore, Message, MessageRole, NewMessage, Session, SessionId,
    SessionWithMessages, StoreError,
};
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use uuid::Uuid;

fn fixture_session(user_text: &str) -> SessionWithMessages {
    SessionWithMessages {
        session: Session {
            id: Uuid::new_v4(),
            title: "Test".to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            context_mode: ContextMode::Squire,
        },
        messages: vec![Message {
            id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            role: MessageRole::User,
            content: user_text.to_string(),
            created_at: chrono::Utc::now(),
            blocks_json: None,
            thinking_content: None,
        }],
    }
}

struct RecordingStore {
    appended: StdMutex<Vec<NewMessage>>,
}

#[async_trait]
impl ConversationStore for RecordingStore {
    async fn create_session(
        &self,
        _session: crate::storage::conversation_store::NewSession,
    ) -> Result<Session, StoreError> {
        unimplemented!()
    }
    async fn append_message(&self, msg: NewMessage) -> Result<Message, StoreError> {
        let stored = Message {
            id: Uuid::new_v4(),
            session_id: msg.session_id,
            role: msg.role.clone(),
            content: msg.content.clone(),
            created_at: chrono::Utc::now(),
            blocks_json: None,
            thinking_content: msg.thinking_content.clone(),
        };
        self.appended.lock().unwrap().push(msg);
        Ok(stored)
    }
    async fn get_session(&self, _id: SessionId) -> Result<SessionWithMessages, StoreError> {
        unimplemented!()
    }
    async fn list_sessions(
        &self,
    ) -> Result<Vec<crate::storage::conversation_store::SessionSummary>, StoreError> {
        unimplemented!()
    }
    async fn update_session_title(
        &self,
        _id: SessionId,
        _title: String,
    ) -> Result<(), StoreError> {
        unimplemented!()
    }
    async fn delete_session(&self, _id: SessionId) -> Result<(), StoreError> {
        unimplemented!()
    }
    async fn truncate_messages_from(
        &self,
        _session_id: SessionId,
        _message_id: Uuid,
    ) -> Result<(), StoreError> {
        unimplemented!()
    }
    async fn set_message_blocks(
        &self,
        _message_id: Uuid,
        _blocks_json: String,
    ) -> Result<(), StoreError> {
        unimplemented!()
    }
}

/// Extracts the context JSON block (expanded_tokens, tokens, active_process_state)
/// from the system message in a `build_turn_input` response, where it is appended
/// after the `--- Context for this turn ---` delimiter.
fn extract_context(system_content: &str) -> Value {
    let marker = "--- Context for this turn ---\n";
    let pos = system_content.find(marker).expect("context block not found in system message");
    serde_json::from_str(&system_content[pos + marker.len()..]).expect("context JSON should parse")
}

// ---- sigil parsing ----

#[test]
fn extract_inline_refs_finds_ids_terminated_by_whitespace_or_sigil() {
    let refs = extract_inline_refs("See §!WF_Chat and §!CONCEPT_Fish§!TOOL_X done");
    assert_eq!(refs, vec!["WF_Chat", "CONCEPT_Fish", "TOOL_X"]);
}

#[test]
fn extract_spans_captures_closed_span_and_flags_unclosed() {
    let (spans, unclosed) = extract_spans("intro §^TRT_A hello world §^ outro §^TRT_B dangling");
    assert_eq!(spans, vec![("TRT_A".to_string(), "hello world".to_string())]);
    assert_eq!(unclosed, Some("TRT_B".to_string()));
}

#[test]
fn strip_span_markers_leaves_clean_text() {
    let out = strip_span_markers("before §^TRT_A inner text §^ after");
    assert_eq!(out, "before inner text after");
}

// ---- raw partition: unmarked_residual (spec §4.1/§4.3) ----

#[test]
fn unmarked_residual_returns_whole_content_when_no_spans_present() {
    let out = unmarked_residual("just plain prose with §!WF_Chat an inline ref");
    assert_eq!(out, "just plain prose with §!WF_Chat an inline ref");
}

#[test]
fn unmarked_residual_excludes_closed_span_body_keeps_surrounding_text() {
    let out = unmarked_residual("before §^TRT_A inner text that becomes a token §^ after");
    assert_eq!(out, "before after");
}

#[test]
fn unmarked_residual_is_empty_when_entire_content_is_one_span() {
    let out = unmarked_residual("§^TRT_A the whole response is one span §^");
    assert_eq!(out, "");
}

#[test]
fn unmarked_residual_handles_multiple_spans_keeping_all_gaps() {
    let out = unmarked_residual(
        "lead-in §^TRT_A span one §^ middle §^TRT_B span two §^ trailing",
    );
    assert_eq!(out, "lead-in middle trailing");
}

#[test]
fn unmarked_residual_preserves_inline_refs_outside_spans() {
    let out = unmarked_residual("See §!CONCEPT_Fish for details. §^TRT_A spot info §^ Done.");
    assert_eq!(out, "See §!CONCEPT_Fish for details. Done.");
}

#[test]
fn unmarked_residual_of_empty_content_is_empty() {
    assert_eq!(unmarked_residual(""), "");
}

// ---- validation gates (spec §8.3) ----

#[test]
fn validate_rejects_ask_user_and_content_together() {
    let resp = SquireResponse {
        ask_user: "question?".to_string(),
        content: "answer".to_string(),
        ..Default::default()
    };
    let err = validate_squire_response(&resp, |_| false).unwrap_err();
    assert_eq!(err.reason, "ask_user and content cannot coexist");
}

#[test]
fn validate_allows_ask_user_alone() {
    let resp = SquireResponse {
        ask_user: "question?".to_string(),
        ..Default::default()
    };
    assert!(validate_squire_response(&resp, |_| false).is_ok());
}

#[test]
fn validate_rejects_empty_close_response() {
    let resp = SquireResponse::default();
    let err = validate_squire_response(&resp, |_| false).unwrap_err();
    assert_eq!(err.reason, "empty close response");
}

#[test]
fn validate_rejects_preserve_with_unknown_token() {
    let resp = SquireResponse {
        preserve: vec!["CONCEPT_X".to_string()],
        ..Default::default()
    };
    let err = validate_squire_response(&resp, |_| false).unwrap_err();
    assert_eq!(err.reason, "preserved token does not exist: CONCEPT_X");
}

#[test]
fn validate_allows_preserve_known_to_store() {
    let resp = SquireResponse {
        preserve: vec!["CONCEPT_X".to_string()],
        ..Default::default()
    };
    assert!(validate_squire_response(&resp, |id| id == "CONCEPT_X").is_ok());
}

#[test]
fn validate_allows_preserve_defined_in_new_tokens() {
    let resp = SquireResponse {
        preserve: vec!["CONCEPT_New".to_string()],
        new_tokens: vec![NewTokenSpec {
            id: "CONCEPT_New".to_string(),
            token_type: "concept".to_string(),
            short_desc: "new".to_string(),
            full_desc: None,
            endpoint: None,
            ranges: vec![],
        }],
        ..Default::default()
    };
    assert!(validate_squire_response(&resp, |_| false).is_ok());
}

#[test]
fn validate_rejects_undisplayable_token_reference() {
    let resp = SquireResponse {
        content: "See §!CONCEPT_Missing".to_string(),
        ..Default::default()
    };
    let err = validate_squire_response(&resp, |_| false).unwrap_err();
    assert_eq!(err.reason, "undisplayable token §!CONCEPT_Missing");
}

#[test]
fn validate_allows_inline_ref_defined_in_new_tokens() {
    let resp = SquireResponse {
        content: "See §!CONCEPT_New".to_string(),
        new_tokens: vec![NewTokenSpec {
            id: "CONCEPT_New".to_string(),
            token_type: "concept".to_string(),
            short_desc: "new concept".to_string(),
            full_desc: None,
            endpoint: None,
            ranges: vec![],
        }],
        ..Default::default()
    };
    assert!(validate_squire_response(&resp, |_| false).is_ok());
}

#[test]
fn validate_allows_inline_ref_known_to_store() {
    let resp = SquireResponse {
        content: "See §!CONCEPT_Old".to_string(),
        ..Default::default()
    };
    assert!(validate_squire_response(&resp, |id| id == "CONCEPT_Old").is_ok());
}

#[test]
fn validate_rejects_unclosed_span() {
    let resp = SquireResponse {
        content: "§^TRT_A never closed".to_string(),
        ..Default::default()
    };
    let err = validate_squire_response(&resp, |_| false).unwrap_err();
    assert_eq!(err.reason, "unclosed §^ span TRT_A");
}

#[test]
fn validate_rejects_undisplayable_span_reference() {
    let resp = SquireResponse {
        content: "§^TRT_Ghost some text §^".to_string(),
        ..Default::default()
    };
    let err = validate_squire_response(&resp, |_| false).unwrap_err();
    assert_eq!(err.reason, "undisplayable span reference §^TRT_Ghost");
}

#[test]
fn validate_allows_span_ref_defined_in_new_tokens() {
    let resp = SquireResponse {
        content: "§^REF_New key content §^".to_string(),
        new_tokens: vec![NewTokenSpec {
            id: "REF_New".to_string(),
            token_type: "referential".to_string(),
            short_desc: "new ref".to_string(),
            full_desc: None,
            endpoint: None,
            ranges: vec![],
        }],
        ..Default::default()
    };
    assert!(validate_squire_response(&resp, |_| false).is_ok());
}

#[test]
fn validate_allows_span_ref_known_to_store() {
    let resp = SquireResponse {
        content: "§^TRT_Old text §^".to_string(),
        ..Default::default()
    };
    assert!(validate_squire_response(&resp, |id| id == "TRT_Old").is_ok());
}

#[test]
fn validate_rejects_relationship_unknown_token() {
    let resp = SquireResponse {
        relationships: vec![Relationship {
            subject: "NONEXISTENT".to_string(),
            predicate: "respondsTo".to_string(),
            object: "ALSO_MISSING".to_string(),
        }],
        ..Default::default()
    };
    let err = validate_squire_response(&resp, |_| false).unwrap_err();
    assert_eq!(
        err.reason,
        "relationship references unknown token: NONEXISTENT"
    );
}

#[test]
fn validate_allows_relationship_with_known_tokens() {
    let resp = SquireResponse {
        relationships: vec![Relationship {
            subject: "CONCEPT_Fish".to_string(),
            predicate: "respondsTo".to_string(),
            object: "USR_Q1".to_string(),
        }],
        ..Default::default()
    };
    assert!(
        validate_squire_response(&resp, |id| id == "CONCEPT_Fish" || id == "USR_Q1").is_ok()
    );
}

#[test]
fn validate_allows_relationship_with_new_token() {
    let resp = SquireResponse {
        relationships: vec![Relationship {
            subject: "CONCEPT_Fresh".to_string(),
            predicate: "respondsTo".to_string(),
            object: "USR_Q1".to_string(),
        }],
        new_tokens: vec![NewTokenSpec {
            id: "CONCEPT_Fresh".to_string(),
            token_type: "concept".to_string(),
            short_desc: "fresh concept".to_string(),
            full_desc: None,
            endpoint: None,
            ranges: vec![],
        }],
        ..Default::default()
    };
    assert!(validate_squire_response(&resp, |id| id == "USR_Q1").is_ok());
}

#[test]
fn validate_rejects_malformed_sigil_bare_section() {
    // § followed by non-sigil character
    let resp = SquireResponse {
        content: "some §a bad sigil".to_string(),
        ..Default::default()
    };
    let err = validate_squire_response(&resp, |_| false).unwrap_err();
    assert!(err.reason.starts_with("malformed sigil"));
}

#[test]
fn validate_allows_valid_sigils() {
    // All valid sigil forms with resolvable references should pass
    let resp = SquireResponse {
        content: "§!CONCEPT_X and §^REF_A span text §^ and bare §^bookmark§^".to_string(),
        new_tokens: vec![
            NewTokenSpec {
                id: "REF_A".to_string(),
                token_type: "referential".to_string(),
                short_desc: "test span".to_string(),
                full_desc: None,
                endpoint: None,
                ranges: vec![],
            },
            NewTokenSpec {
                id: "bookmark".to_string(),
                token_type: "referential".to_string(),
                short_desc: "bare bookmark".to_string(),
                full_desc: None,
                endpoint: None,
                ranges: vec![],
            },
        ],
        ..Default::default()
    };
    assert!(validate_squire_response(&resp, |id| id == "CONCEPT_X").is_ok());
}

// ---- InMemorySquireStore ----

#[tokio::test]
async fn in_memory_store_roundtrips_token_and_preserve_list() {
    let store = InMemorySquireStore::new();
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
                ranges: vec![],
            },
            1,
            SessionId::nil(),
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
async fn in_memory_store_increments_turn_counter() {
    let store = InMemorySquireStore::new();
    let sid = Uuid::new_v4();
    assert_eq!(store.current_turn(sid).await, 0);
    store.increment_turn(sid).await;
    store.increment_turn(sid).await;
    assert_eq!(store.current_turn(sid).await, 2);
}

#[tokio::test]
async fn in_memory_store_clear_all_preserve_lists_wipes_every_session() {
    let store = InMemorySquireStore::new();
    let sid_a = Uuid::new_v4();
    let sid_b = Uuid::new_v4();
    store
        .upsert_token(
            NewTokenSpec {
                id: "CONCEPT_X".to_string(),
                token_type: "concept".to_string(),
                short_desc: "desc".to_string(),
                full_desc: None,
                endpoint: None,
                ranges: vec![],
            },
            0,
            SessionId::nil(),
        )
        .await;
    store
        .set_preserve_list(sid_a, vec!["CONCEPT_X".to_string()])
        .await;
    store
        .set_preserve_list(sid_b, vec!["CONCEPT_X".to_string()])
        .await;
    assert_eq!(store.preserved_tokens(sid_a).await.len(), 1);
    assert_eq!(store.preserved_tokens(sid_b).await.len(), 1);

    store.clear_all_preserve_lists().await;

    assert!(store.preserved_tokens(sid_a).await.is_empty());
    assert!(store.preserved_tokens(sid_b).await.is_empty());
}

#[tokio::test]
async fn in_memory_store_records_compliance_failures() {
    let store = InMemorySquireStore::new();
    let sid = Uuid::new_v4();
    store
        .record_compliance_failure(ComplianceFailureRecord {
            session_id: sid,
            rule: "empty_close_response".to_string(),
            reason: "empty close response".to_string(),
            retry_count: 4,
            failed_content: "{}".to_string(),
            timestamp: chrono::Utc::now(),
        })
        .await;
    let failures = store.compliance_failures.lock().await;
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].rule, "empty_close_response");
}

// ---- classify_rejection_rule ----

#[test]
fn classify_rejection_rule_maps_known_reasons() {
    assert_eq!(
        classify_rejection_rule("response is not valid Squire protocol JSON: eof"),
        "malformed_json"
    );
    assert_eq!(
        classify_rejection_rule("ask_user and content cannot coexist"),
        "ask_user_content_conflict"
    );
    assert_eq!(
        classify_rejection_rule("empty close response"),
        "empty_close_response"
    );
    assert_eq!(
        classify_rejection_rule("undisplayable token §!CONCEPT_Ghost"),
        "undisplayable_token"
    );
    assert_eq!(
        classify_rejection_rule("unclosed §^ span TRT_A"),
        "unclosed_span"
    );
    assert_eq!(
        classify_rejection_rule("non-invocable token TOOL_X"),
        "non_invocable_token"
    );
    assert_eq!(
        classify_rejection_rule("undisplayable span reference §^TRT_Ghost"),
        "undisplayable_span_reference"
    );
    assert_eq!(
        classify_rejection_rule("malformed sigil: §a at position 42 — expected §!/§^/§#"),
        "malformed_sigil"
    );
    assert_eq!(
        classify_rejection_rule("preserved token does not exist: GHOST"),
        "preserve_token_unknown"
    );
    assert_eq!(
        classify_rejection_rule("relationship references unknown token: FAKE"),
        "relationship_unknown_token"
    );
    assert_eq!(classify_rejection_rule("something new"), "other");
}

#[tokio::test]
async fn explore_memory_filters_by_type_and_query() {
    let store = InMemorySquireStore::new();
    store
        .upsert_token(
            NewTokenSpec {
                id: "CONCEPT_Fish".to_string(),
                token_type: "concept".to_string(),
                short_desc: "fishing locations".to_string(),
                full_desc: None,
                endpoint: None,
                ranges: vec![],
            },
            0,
            SessionId::nil(),
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
                ranges: vec![],
            },
            0,
            SessionId::nil(),
        )
        .await;

    let results = store.explore_memory("concept", "fish", 0, 10, 0, SessionId::nil()).await;
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].token_id, "CONCEPT_Fish");

    let all = store.explore_memory("all", "", 0, 10, 0, SessionId::nil()).await;
    assert_eq!(all.len(), 2);
}

// ---- graph traversal (num_hops) ----

#[tokio::test]
async fn explore_memory_num_hops_zero_does_not_expand() {
    let store = InMemorySquireStore::new();
    store
        .upsert_token(
            NewTokenSpec {
                id: "CONCEPT_Fish".to_string(),
                token_type: "concept".to_string(),
                short_desc: "fishing locations".to_string(),
                full_desc: None,
                endpoint: None,
                ranges: vec![],
            },
            0,
            SessionId::nil(),
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
                ranges: vec![],
            },
            0,
            SessionId::nil(),
        )
        .await;
    store
        .insert_relationship(Relationship {
            subject: "TRT_Spot".to_string(),
            predicate: "instanceOf".to_string(),
            object: "CONCEPT_Fish".to_string(),
        })
        .await;

    let results = store.explore_memory("all", "fish", 0, 10, 0, SessionId::nil()).await;
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].token_id, "CONCEPT_Fish");
    assert_eq!(results[0].hop_distance, 0);
}

#[tokio::test]
async fn explore_memory_num_hops_one_expands_directly_connected_token() {
    let store = InMemorySquireStore::new();
    store
        .upsert_token(
            NewTokenSpec {
                id: "CONCEPT_Fish".to_string(),
                token_type: "concept".to_string(),
                short_desc: "fishing locations".to_string(),
                full_desc: None,
                endpoint: None,
                ranges: vec![],
            },
            0,
            SessionId::nil(),
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
                ranges: vec![],
            },
            0,
            SessionId::nil(),
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
                ranges: vec![],
            },
            0,
            SessionId::nil(),
        )
        .await;
    store
        .insert_relationship(Relationship {
            subject: "TRT_Spot".to_string(),
            predicate: "instanceOf".to_string(),
            object: "CONCEPT_Fish".to_string(),
        })
        .await;

    let results = store.explore_memory("all", "fishing", 1, 10, 0, SessionId::nil()).await;
    let ids: Vec<&str> = results.iter().map(|t| t.token_id.as_str()).collect();
    assert!(ids.contains(&"CONCEPT_Fish"));
    assert!(ids.contains(&"TRT_Spot"));
    assert!(!ids.contains(&"WF_Unrelated"));

    let spot = results.iter().find(|t| t.token_id == "TRT_Spot").unwrap();
    assert_eq!(spot.hop_distance, 1);
    assert_eq!(spot.via_token_id.as_deref(), Some("CONCEPT_Fish"));
    let fish = results.iter().find(|t| t.token_id == "CONCEPT_Fish").unwrap();
    assert!(spot.score < fish.score);
}

#[tokio::test]
async fn explore_memory_traversal_is_undirected_and_multi_hop() {
    let store = InMemorySquireStore::new();
    for id in ["A", "B", "C"] {
        store
            .upsert_token(
                NewTokenSpec {
                    id: id.to_string(),
                    token_type: "concept".to_string(),
                    short_desc: format!("node {}", id),
                    full_desc: None,
                    endpoint: None,
                    ranges: vec![],
                },
                0,
                SessionId::nil(),
            )
            .await;
    }
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

    let from_a = store.explore_memory("all", "node A", 2, 10, 0, SessionId::nil()).await;
    let ids: Vec<&str> = from_a.iter().map(|t| t.token_id.as_str()).collect();
    assert!(ids.contains(&"A"));
    assert!(ids.contains(&"B"));
    assert!(ids.contains(&"C"), "2-hop traversal should reach C from A via B");
    let c = from_a.iter().find(|t| t.token_id == "C").unwrap();
    assert_eq!(c.hop_distance, 2);
}

#[tokio::test]
async fn explore_memory_traversal_still_respects_max_results() {
    let store = InMemorySquireStore::new();
    store
        .upsert_token(
            NewTokenSpec {
                id: "HUB".to_string(),
                token_type: "concept".to_string(),
                short_desc: "hub node".to_string(),
                full_desc: None,
                endpoint: None,
                ranges: vec![],
            },
            0,
            SessionId::nil(),
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
                    ranges: vec![],
                },
                0,
            SessionId::nil(),
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

    let results = store.explore_memory("all", "hub", 1, 2, 0, SessionId::nil()).await;
    assert_eq!(results.len(), 2);
}

// ---- accumulated_hits / effective_priority scoring ----

#[test]
fn effective_priority_matches_spec_formula() {
    assert_eq!(effective_priority(0, 5, 5), 0);
    assert_eq!(effective_priority(3, 5, 2), 0);
    assert_eq!(effective_priority(5, 5, 2), 2);
    assert_eq!(effective_priority(0, 10, 2), -8);
}

#[tokio::test]
async fn record_hit_increments_accumulated_hits() {
    let store = InMemorySquireStore::new();
    store
        .upsert_token(
            NewTokenSpec {
                id: "CONCEPT_X".to_string(),
                token_type: "concept".to_string(),
                short_desc: "desc".to_string(),
                full_desc: None,
                endpoint: None,
                ranges: vec![],
            },
            0,
            SessionId::nil(),
        )
        .await;
    let results = store.explore_memory("all", "", 0, 10, 0, SessionId::nil()).await;
    assert_eq!(results[0].accumulated_hits, 1);

    store.record_hit("CONCEPT_X").await;
    store.record_hit("CONCEPT_X").await;
    let results = store.explore_memory("all", "", 0, 10, 0, SessionId::nil()).await;
    assert_eq!(results[0].accumulated_hits, 3);
}

#[tokio::test]
async fn preserved_tokens_increments_hit_on_load() {
    let store = InMemorySquireStore::new();
    let sid = Uuid::new_v4();
    store
        .upsert_token(
            NewTokenSpec {
                id: "CONCEPT_X".to_string(),
                token_type: "concept".to_string(),
                short_desc: "desc".to_string(),
                full_desc: None,
                endpoint: None,
                ranges: vec![],
            },
            0,
            SessionId::nil(),
        )
        .await;
    store
        .set_preserve_list(sid, vec!["CONCEPT_X".to_string()])
        .await;

    let first = store.preserved_tokens(sid).await;
    assert_eq!(first[0].accumulated_hits, 2);
    let second = store.preserved_tokens(sid).await;
    assert_eq!(second[0].accumulated_hits, 3);
}

#[tokio::test]
async fn token_to_detail_tool_increments_hit_count_on_store_backed_token() {
    let store = Arc::new(InMemorySquireStore::new());
    store
        .upsert_token(
            NewTokenSpec {
                id: "CONCEPT_X".to_string(),
                token_type: "concept".to_string(),
                short_desc: "desc".to_string(),
                full_desc: Some("full".to_string()),
                endpoint: None,
                ranges: vec![],
            },
            0,
            SessionId::nil(),
        )
        .await;
    let tool = SquireTokenToDetailTool {
        store: store.clone(),
    };

    tool.execute(
        "call-1",
        serde_json::json!({"token_id": "CONCEPT_X", "detail_level": "short"}),
    )
    .await;

    let results = store.explore_memory("all", "", 0, 10, 0, SessionId::nil()).await;
    assert_eq!(results[0].accumulated_hits, 2);
}

#[tokio::test]
async fn explore_memory_breaks_near_ties_by_effective_priority() {
    let store = InMemorySquireStore::new();
    store
        .upsert_token(
            NewTokenSpec {
                id: "CONCEPT_Popular".to_string(),
                token_type: "concept".to_string(),
                short_desc: "shared topic".to_string(),
                full_desc: None,
                endpoint: None,
                ranges: vec![],
            },
            0,
            SessionId::nil(),
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
                ranges: vec![],
            },
            0,
            SessionId::nil(),
        )
        .await;
    store.record_hit("CONCEPT_Popular").await;
    store.record_hit("CONCEPT_Popular").await;
    store.record_hit("CONCEPT_Popular").await;

    let results = store.explore_memory("all", "shared topic", 0, 10, 10, SessionId::nil()).await;
    assert_eq!(results[0].token_id, "CONCEPT_Popular");
}

// ---- SquireContextAdapter ----

#[tokio::test]
async fn build_turn_input_merges_base_tools_with_built_ins() {
    let store = Arc::new(InMemorySquireStore::new());
    let mut adapter = SquireContextAdapter::new(store);
    let session = fixture_session("hello squire");
    let base_tools = vec![ToolDefinition {
        name: "run_terminal".to_string(),
        description: "runs shell commands".to_string(),
        input_schema: serde_json::json!({}),
    }];

    let turn_input = adapter.build_turn_input(&session, &base_tools).await.unwrap();

    let tool_names: Vec<&str> = turn_input.tools.iter().map(|t| t.name.as_str()).collect();
    // SquireContextAdapter replaces base_tools with its own built-in set.
    assert_eq!(tool_names, vec!["explore", "token_to_detail", "invoke"]);

    assert!(matches!(turn_input.messages[0].role, ChatRole::System));
    assert!(matches!(turn_input.messages[1].role, ChatRole::User));
    let request: Value = serde_json::from_str(&turn_input.messages[1].content).unwrap();
    assert_eq!(request["user_request"], "§^chunk_0§^hello squire");
    let ctx = extract_context(&turn_input.messages[0].content);
    assert!(ctx["expanded_tokens"].is_array());
    assert!(ctx["tokens"].is_array());
    assert!(ctx["active_process_state"].is_object());
}

#[tokio::test]
async fn build_turn_input_prefetches_workflow_tool_skill_and_memory_with_individual_limits() {
    let store = Arc::new(InMemorySquireStore::new());
    store
        .upsert_token(
            NewTokenSpec {
                id: "CONCEPT_A".to_string(),
                token_type: "concept".to_string(),
                short_desc: "alpha memory".to_string(),
                full_desc: None,
                endpoint: None,
                ranges: vec![],
            },
            0,
            SessionId::nil(),
        )
        .await;
    store
        .upsert_token(
            NewTokenSpec {
                id: "WF_A".to_string(),
                token_type: "workflow".to_string(),
                short_desc: "alpha workflow".to_string(),
                full_desc: None,
                endpoint: None,
                ranges: vec![],
            },
            0,
            SessionId::nil(),
        )
        .await;
    store
        .upsert_token(
            NewTokenSpec {
                id: "TOOL_A".to_string(),
                token_type: "tool".to_string(),
                short_desc: "alpha tool".to_string(),
                full_desc: None,
                endpoint: None,
                ranges: vec![],
            },
            0,
            SessionId::nil(),
        )
        .await;
    store
        .upsert_token(
            NewTokenSpec {
                id: "SKILL_A".to_string(),
                token_type: "skill".to_string(),
                short_desc: "alpha skill".to_string(),
                full_desc: None,
                endpoint: None,
                ranges: vec![],
            },
            0,
            SessionId::nil(),
        )
        .await;

    let mut adapter = SquireContextAdapter::new_with_prefetch(
        store.clone(),
        SquirePrefetchConfig {
            memory_top_k: 2,  // 2 because the USR_T0_001_* token ingested in
                              // build_turn_input also matches "alpha"
            workflow_top_k: 1,
            tool_top_k: 1,
            skill_top_k: 1,
            min_score: 0.0,
        },
    );
    let session = fixture_session("alpha");
    let turn_input = adapter.build_turn_input(&session, &[]).await.unwrap();
    // The user chunk USR_T0_001_* was ingested into the store but is not in
    // the request JSON (it's not part of preserved/prefetched tokens).
    let session_short = &session.session.id.simple().to_string()[..8];
    assert!(store.token_exists(&format!("USR_T0_001_{}", session_short)).await);
    let ctx = extract_context(&turn_input.messages[0].content);

    // Prefetched tokens have no full_desc, so they go to the tokens (short) list.
    let ids: Vec<String> = ctx["tokens"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.get("token_id").and_then(|id| id.as_str()).map(str::to_string))
        .collect();

    assert!(ids.contains(&"CONCEPT_A".to_string()));
    assert!(ids.contains(&"WF_A".to_string()));
    assert!(ids.contains(&"TOOL_A".to_string()));
    assert!(ids.contains(&"SKILL_A".to_string()));
}

#[tokio::test]
async fn build_turn_input_merges_preserved_first_then_prefetch_without_duplicates() {
    let store = Arc::new(InMemorySquireStore::new());
    store
        .upsert_token(
            NewTokenSpec {
                id: "CONCEPT_Keep".to_string(),
                token_type: "concept".to_string(),
                short_desc: "topic".to_string(),
                full_desc: None,
                endpoint: None,
                ranges: vec![],
            },
            0,
            SessionId::nil(),
        )
        .await;

    let session = fixture_session("topic");
    let sid = session.session.id;
    store
        .set_preserve_list(sid, vec!["CONCEPT_Keep".to_string()])
        .await;

    let mut adapter = SquireContextAdapter::new(store);
    let turn_input = adapter.build_turn_input(&session, &[]).await.unwrap();
    let ctx = extract_context(&turn_input.messages[0].content);
    // Preserved tokens (CONCEPT_Keep) are expanded — they go in expanded_tokens.
    // The user chunk USR_T0_001 has full_desc, so it is also in expanded_tokens.
    let expanded = ctx["expanded_tokens"].as_array().unwrap();
    assert!(!expanded.is_empty());
    let expanded_ids: Vec<String> = expanded
        .iter()
        .filter_map(|v| v.get("token_id").and_then(|id| id.as_str()).map(str::to_string))
        .collect();
    // Preserved tokens come first in the merged list, so CONCEPT_Keep
    // should be the first entry in expanded_tokens.
    assert_eq!(expanded_ids.first().map(String::as_str), Some("CONCEPT_Keep"));
    // No duplicates.
    assert_eq!(
        expanded_ids
            .iter()
            .filter(|id| id.as_str() == "CONCEPT_Keep")
            .count(),
        1
    );
}

#[tokio::test]
async fn finalize_turn_persists_expanded_content_on_compliant_response() {
    let store = Arc::new(InMemorySquireStore::new());
    let mut adapter = SquireContextAdapter::new(store);
    let conv_store = RecordingStore {
        appended: StdMutex::new(Vec::new()),
    };
    let sid = Uuid::new_v4();
    let mut messages = Vec::new();

    let response = serde_json::json!({
        "ask_user": "",
        "content": "§^TRT_Answer The answer is 42 §^",
        "preserve": [],
        "new_tokens": [{"id": "TRT_Answer", "type": "referential", "short_desc": "the answer"}],
        "relationships": []
    })
    .to_string();

    let outcome = adapter
        .finalize_turn(sid, response, None, &mut messages, &conv_store)
        .await
        .unwrap();

    assert!(matches!(outcome, TurnOutcome::Done));
    let appended = conv_store.appended.lock().unwrap();
    assert_eq!(appended.len(), 1);
    assert_eq!(appended[0].content, "The answer is 42");
    drop(appended);
    assert!(adapter.store.token_exists("TRT_Answer").await);
}

// ---- hit-count fidelity ----

#[tokio::test]
async fn finalize_turn_credits_a_hit_for_a_preexisting_token_cited_via_sigil_without_token_to_detail() {
    let store = Arc::new(InMemorySquireStore::new());
    store
        .upsert_token(
            NewTokenSpec {
                id: "WF_Existing".to_string(),
                token_type: "workflow".to_string(),
                short_desc: "an existing workflow".to_string(),
                full_desc: None,
                endpoint: None,
                ranges: vec![],
            },
            0,
            SessionId::nil(),
        )
        .await;
    let baseline_hits = store.explore_memory("all", "", 0, 10, 0, SessionId::nil()).await[0].accumulated_hits;

    let mut adapter = SquireContextAdapter::new(store.clone());
    let conv_store = RecordingStore {
        appended: StdMutex::new(Vec::new()),
    };
    let sid = Uuid::new_v4();
    let mut messages = Vec::new();

    let response = serde_json::json!({
        "ask_user": "",
        "content": "The best approach follows §!WF_Existing which starts there.",
        "preserve": [],
        "new_tokens": [],
        "relationships": []
    })
    .to_string();

    let outcome = adapter
        .finalize_turn(sid, response, None, &mut messages, &conv_store)
        .await
        .unwrap();
    assert!(matches!(outcome, TurnOutcome::Done));

    let results = store.explore_memory("all", "", 0, 10, 0, SessionId::nil()).await;
    let after = results
        .iter()
        .find(|t| t.token_id == "WF_Existing")
        .unwrap();
    assert_eq!(after.accumulated_hits, baseline_hits + 1);
}

#[tokio::test]
async fn finalize_turn_does_not_double_credit_a_token_defined_and_cited_in_the_same_turn() {
    let store = Arc::new(InMemorySquireStore::new());
    let mut adapter = SquireContextAdapter::new(store.clone());
    let conv_store = RecordingStore {
        appended: StdMutex::new(Vec::new()),
    };
    let sid = Uuid::new_v4();
    let mut messages = Vec::new();

    let response = serde_json::json!({
        "ask_user": "",
        "content": "See §!TRT_New for the answer.",
        "preserve": [],
        "new_tokens": [{"id": "TRT_New", "type": "referential", "short_desc": "the answer"}],
        "relationships": []
    })
    .to_string();

    let outcome = adapter
        .finalize_turn(sid, response, None, &mut messages, &conv_store)
        .await
        .unwrap();
    assert!(matches!(outcome, TurnOutcome::Done));

    let results = store.explore_memory("all", "", 0, 10, 0, sid).await;
    let token = results.iter().find(|t| t.token_id == "TRT_New").unwrap();
    assert_eq!(token.accumulated_hits, 1);
}

#[tokio::test]
async fn finalize_turn_credits_exactly_one_hit_for_repeated_citations_of_the_same_token() {
    let store = Arc::new(InMemorySquireStore::new());
    store
        .upsert_token(
            NewTokenSpec {
                id: "CONCEPT_Repeated".to_string(),
                token_type: "concept".to_string(),
                short_desc: "cited twice".to_string(),
                full_desc: None,
                endpoint: None,
                ranges: vec![],
            },
            0,
            SessionId::nil(),
        )
        .await;
    let baseline_hits = store.explore_memory("all", "", 0, 10, 0, SessionId::nil()).await[0].accumulated_hits;

    let mut adapter = SquireContextAdapter::new(store.clone());
    let conv_store = RecordingStore {
        appended: StdMutex::new(Vec::new()),
    };
    let sid = Uuid::new_v4();
    let mut messages = Vec::new();

    let response = serde_json::json!({
        "ask_user": "",
        "content": "First mention §!CONCEPT_Repeated and second mention §!CONCEPT_Repeated too.",
        "preserve": [],
        "new_tokens": [],
        "relationships": []
    })
    .to_string();

    let outcome = adapter
        .finalize_turn(sid, response, None, &mut messages, &conv_store)
        .await
        .unwrap();
    assert!(matches!(outcome, TurnOutcome::Done));

    let results = store.explore_memory("all", "", 0, 10, 0, SessionId::nil()).await;
    let after = results
        .iter()
        .find(|t| t.token_id == "CONCEPT_Repeated")
        .unwrap();
    assert_eq!(after.accumulated_hits, baseline_hits + 1);
}

// ---- raw partition ----

#[tokio::test]
async fn finalize_turn_persists_unmarked_text_to_raw_partition_on_compliant_response() {
    let store = Arc::new(InMemorySquireStore::new());
    let mut adapter = SquireContextAdapter::new(store.clone());
    let conv_store = RecordingStore {
        appended: StdMutex::new(Vec::new()),
    };
    let sid = Uuid::new_v4();
    let mut messages = Vec::new();

    let response = serde_json::json!({
        "ask_user": "",
        "content": "Sure thing. §^TRT_Answer The answer is 42 §^ Hope that helps.",
        "preserve": [],
        "new_tokens": [{"id": "TRT_Answer", "type": "referential", "short_desc": "the answer"}],
        "relationships": []
    })
    .to_string();

    let outcome = adapter
        .finalize_turn(sid, response, None, &mut messages, &conv_store)
        .await
        .unwrap();
    assert!(matches!(outcome, TurnOutcome::Done));

    let records = store.raw_partition_records().await;
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].session_id, sid);
    assert_eq!(records[0].content, "Sure thing. Hope that helps.");
    assert!(!records[0].content.contains("The answer is 42"));
}

#[tokio::test]
async fn finalize_turn_writes_nothing_to_raw_partition_when_response_is_fully_spanned() {
    let store = Arc::new(InMemorySquireStore::new());
    let mut adapter = SquireContextAdapter::new(store.clone());
    let conv_store = RecordingStore {
        appended: StdMutex::new(Vec::new()),
    };
    let sid = Uuid::new_v4();
    let mut messages = Vec::new();

    let response = serde_json::json!({
        "ask_user": "",
        "content": "§^TRT_Answer The entire response is one span §^",
        "preserve": [],
        "new_tokens": [{"id": "TRT_Answer", "type": "referential", "short_desc": "the answer"}],
        "relationships": []
    })
    .to_string();

    let outcome = adapter
        .finalize_turn(sid, response, None, &mut messages, &conv_store)
        .await
        .unwrap();
    assert!(matches!(outcome, TurnOutcome::Done));
    assert!(store.raw_partition_records().await.is_empty());
}

#[tokio::test]
async fn finalize_turn_writes_nothing_to_raw_partition_on_rejection() {
    let store = Arc::new(InMemorySquireStore::new());
    let mut adapter = SquireContextAdapter::new(store.clone());
    let conv_store = RecordingStore {
        appended: StdMutex::new(Vec::new()),
    };
    let sid = Uuid::new_v4();
    let mut messages = Vec::new();

    let outcome = adapter
        .finalize_turn(sid, "See §!CONCEPT_Ghost".to_string(), None, &mut messages, &conv_store)
        .await
        .unwrap();
    assert!(matches!(outcome, TurnOutcome::Retry));
    assert!(store.raw_partition_records().await.is_empty());
}

#[tokio::test]
async fn finalize_turn_raw_partition_records_carry_the_current_turn_number() {
    let store = Arc::new(InMemorySquireStore::new());
    let mut adapter = SquireContextAdapter::new(store.clone());
    let conv_store = RecordingStore {
        appended: StdMutex::new(Vec::new()),
    };
    let sid = Uuid::new_v4();

    let mut messages = Vec::new();
    adapter
        .finalize_turn(
            sid,
            serde_json::json!({
                "ask_user": "", "content": "first turn unmarked prose",
                "preserve": [], "new_tokens": [], "relationships": []
            })
            .to_string(),
            None,
            &mut messages,
            &conv_store,
        )
        .await
        .unwrap();

    let mut messages = Vec::new();
    adapter
        .finalize_turn(
            sid,
            serde_json::json!({
                "ask_user": "", "content": "second turn unmarked prose",
                "preserve": [], "new_tokens": [], "relationships": []
            })
            .to_string(),
            None,
            &mut messages,
            &conv_store,
        )
        .await
        .unwrap();

    let records = store.raw_partition_records().await;
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].turn, 0);
    assert_eq!(records[0].content, "first turn unmarked prose");
    assert_eq!(records[1].turn, 1);
    assert_eq!(records[1].content, "second turn unmarked prose");
}

#[tokio::test]
async fn finalize_turn_retries_on_malformed_json_then_fails_after_max_retries() {
    let store = Arc::new(InMemorySquireStore::new());
    let mut adapter = SquireContextAdapter::new(store);
    let conv_store = RecordingStore {
        appended: StdMutex::new(Vec::new()),
    };
    let sid = Uuid::new_v4();

    for _ in 0..3 {
        let mut messages = Vec::new();
        let outcome = adapter
            .finalize_turn(
                sid,
                "See §!CONCEPT_Ghost".to_string(),
                None,
                &mut messages,
                &conv_store,
            )
            .await
            .unwrap();
        assert!(matches!(outcome, TurnOutcome::Retry));
        assert_eq!(messages.len(), 2);
    }

    let mut messages = Vec::new();
    let outcome = adapter
        .finalize_turn(
            sid,
            "See §!CONCEPT_Ghost".to_string(),
            None,
            &mut messages,
            &conv_store,
        )
        .await
        .unwrap();
    match outcome {
        TurnOutcome::Failed { reason, failed_content } => {
            assert!(reason.contains("undisplayable token"));
            assert_eq!(failed_content, "See §!CONCEPT_Ghost");
        }
        _ => panic!("expected Failed after exhausting retries"),
    }

    let appended = conv_store.appended.lock().unwrap();
    assert_eq!(appended.len(), 1);
    assert!(matches!(appended[0].role, MessageRole::Assistant));
    assert!(appended[0].content.contains("compliance failure"));
    assert!(appended[0].content.contains("undisplayable token"));
    assert!(appended[0].content.contains("See §!CONCEPT_Ghost"));
}

#[tokio::test]
async fn finalize_turn_records_structured_failure_metadata_on_final_failure() {
    let store = Arc::new(InMemorySquireStore::new());
    let mut adapter = SquireContextAdapter::new(store.clone());
    let conv_store = RecordingStore {
        appended: StdMutex::new(Vec::new()),
    };
    let sid = Uuid::new_v4();

    for _ in 0..4 {
        let mut messages = Vec::new();
        let _ = adapter
            .finalize_turn(
                sid,
                "See §!CONCEPT_Ghost".to_string(),
                None,
                &mut messages,
                &conv_store,
            )
            .await
            .unwrap();
    }

    let failures = store.compliance_failures.lock().await;
    assert_eq!(failures.len(), 1);
    let record = &failures[0];
    assert_eq!(record.session_id, sid);
    assert_eq!(record.rule, "undisplayable_token");
    assert!(record.reason.contains("undisplayable token"));
    assert_eq!(record.retry_count, 4);
    assert_eq!(record.failed_content, "See §!CONCEPT_Ghost");
}

#[tokio::test]
async fn finalize_turn_rejects_response_with_undisplayable_token() {
    let store = Arc::new(InMemorySquireStore::new());
    let mut adapter = SquireContextAdapter::new(store);
    let conv_store = RecordingStore {
        appended: StdMutex::new(Vec::new()),
    };
    let sid = Uuid::new_v4();
    let mut messages = Vec::new();

    let response = serde_json::json!({
        "ask_user": "",
        "content": "See §!CONCEPT_Ghost",
        "preserve": [],
        "new_tokens": [],
        "relationships": []
    })
    .to_string();

    let outcome = adapter
        .finalize_turn(sid, response, None, &mut messages, &conv_store)
        .await
        .unwrap();

    assert!(matches!(outcome, TurnOutcome::Retry));
    assert!(conv_store.appended.lock().unwrap().is_empty());
    let rejection: Value = serde_json::from_str(&messages[1].content).unwrap();
    assert_eq!(rejection["reason"], "undisplayable token §!CONCEPT_Ghost");
}

#[tokio::test]
async fn finalize_turn_returns_ask_user_outcome_instead_of_erroring() {
    let store = Arc::new(InMemorySquireStore::new());
    let mut adapter = SquireContextAdapter::new(store);
    let conv_store = RecordingStore {
        appended: StdMutex::new(Vec::new()),
    };
    let sid = Uuid::new_v4();
    let mut messages = Vec::new();

    let response = serde_json::json!({
        "ask_user": "Which city are you asking about?",
        "content": "",
        "preserve": [],
        "new_tokens": [],
        "relationships": []
    })
    .to_string();

    let outcome = adapter
        .finalize_turn(sid, response, None, &mut messages, &conv_store)
        .await
        .expect("ask_user should be a valid outcome, not an Err");

    match outcome {
        TurnOutcome::AskUser { question } => {
            assert_eq!(question, "Which city are you asking about?");
        }
        _other => panic!("expected TurnOutcome::AskUser, got something else"),
    }
    assert!(conv_store.appended.lock().unwrap().is_empty());
    assert!(messages.is_empty());
}

#[tokio::test]
async fn finalize_turn_rejects_ask_user_and_content_both_populated_via_ask_user_branch() {
    let store = Arc::new(InMemorySquireStore::new());
    let mut adapter = SquireContextAdapter::new(store);
    let conv_store = RecordingStore {
        appended: StdMutex::new(Vec::new()),
    };
    let sid = Uuid::new_v4();
    let mut messages = Vec::new();

    let response = serde_json::json!({
        "ask_user": "Which city?",
        "content": "Sydney is lovely this time of year.",
        "preserve": [],
        "new_tokens": [],
        "relationships": []
    })
    .to_string();

    let outcome = adapter
        .finalize_turn(sid, response, None, &mut messages, &conv_store)
        .await
        .unwrap();

    assert!(matches!(outcome, TurnOutcome::Retry));
    let rejection: Value = serde_json::from_str(&messages[1].content).unwrap();
    assert_eq!(rejection["reason"], "ask_user and content cannot coexist");
}

#[tokio::test]
async fn finalize_turn_ask_user_does_not_reset_retry_count() {
    let store = Arc::new(InMemorySquireStore::new());
    let mut adapter = SquireContextAdapter::new(store);
    let conv_store = RecordingStore {
        appended: StdMutex::new(Vec::new()),
    };
    let sid = Uuid::new_v4();

    let ask_response = "§#ask_user\nWhich city?".to_string();
    let mut messages = Vec::new();
    let outcome = adapter
        .finalize_turn(sid, ask_response, None, &mut messages, &conv_store)
        .await
        .unwrap();
    assert!(matches!(outcome, TurnOutcome::AskUser { .. }));

    for _ in 0..3 {
        let mut messages = Vec::new();
        let outcome = adapter
            .finalize_turn(
                sid,
                "See §!CONCEPT_Ghost".to_string(),
                None,
                &mut messages,
                &conv_store,
            )
            .await
            .unwrap();
        assert!(matches!(outcome, TurnOutcome::Retry));
    }
    let mut messages = Vec::new();
    let outcome = adapter
        .finalize_turn(
            sid,
            "See §!CONCEPT_Ghost".to_string(),
            None,
            &mut messages,
            &conv_store,
        )
        .await
        .unwrap();
    assert!(matches!(outcome, TurnOutcome::Failed { .. }));
}

// ---- built-in tools ----

#[tokio::test]
async fn explore_tool_searches_full_tool_registry_for_resource_type_tool() {
    let mut registry = ToolRegistry::empty();
    registry.register(Box::new(crate::agent::TerminalTool));
    let tool_defs_snapshot = registry.definitions();
    let tool = SquireExploreTool {
        store: Arc::new(InMemorySquireStore::new()),
        tool_defs: tool_defs_snapshot,
        session_id: Uuid::new_v4(),
    };

    let result = tool
        .execute("call-1", serde_json::json!({"resource_type": "tool", "query": "terminal"}))
        .await;
    assert!(!result.is_error);
    let parsed: Vec<TokenSummary> = serde_json::from_str(&result.output).unwrap();
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].token_id, "run_terminal");
}

// ---- endpoint-carrying TokenDetail extension ----

fn fake_mcp_server(id: &str) -> crate::state::config::McpServerConfig {
    crate::state::config::McpServerConfig {
        id: id.to_string(),
        name: format!("Fake server {}", id),
        transport: "stdio".to_string(),
        command: "this-binary-does-not-exist-token-detail-endpoint-test".to_string(),
        args: vec![],
        url: None,
        enabled: true,
        env: std::collections::HashMap::new(),
        headers: std::collections::HashMap::new(),
    }
}

#[test]
fn token_detail_and_new_token_spec_endpoint_round_trip_through_serde() {
    let endpoint = ToolEndpoint::Mcp {
        server: fake_mcp_server("srv1").into(),
        remote_name: "remote_tool".to_string(),
    };
    let detail = TokenDetail {
        short_desc: "d".to_string(),
        full_desc: None,
        endpoint: Some(endpoint.clone()),
        ranges: vec![],
    };
    let json = serde_json::to_string(&detail).unwrap();
    let back: TokenDetail = serde_json::from_str(&json).unwrap();
    assert_eq!(back.endpoint, Some(endpoint));
}

#[tokio::test]
async fn upsert_token_persists_and_returns_endpoint_via_in_memory_store() {
    let store = InMemorySquireStore::new();
    let endpoint = ToolEndpoint::Mcp {
        server: fake_mcp_server("srv1").into(),
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
                ranges: vec![],
            },
            0,
            SessionId::nil(),
        )
        .await;

    let detail = store.token_detail("mcp_srv1_remote_tool").await.unwrap();
    assert_eq!(detail.endpoint, Some(endpoint));
}

#[tokio::test]
async fn upsert_token_without_endpoint_preserves_previously_stored_endpoint() {
    let store = InMemorySquireStore::new();
    let endpoint = ToolEndpoint::Mcp {
        server: fake_mcp_server("srv1").into(),
        remote_name: "remote_tool".to_string(),
    };
    store
        .upsert_token(
            NewTokenSpec {
                id: "mcp_srv1_remote_tool".to_string(),
                token_type: "tool".to_string(),
                short_desc: "v1".to_string(),
                full_desc: None,
                endpoint: Some(endpoint.clone()),
                ranges: vec![],
            },
            0,
            SessionId::nil(),
        )
        .await;
    store
        .upsert_token(
            NewTokenSpec {
                id: "mcp_srv1_remote_tool".to_string(),
                token_type: "tool".to_string(),
                short_desc: "v2".to_string(),
                full_desc: None,
                endpoint: None,
                ranges: vec![],
            },
            0,
            SessionId::nil(),
        )
        .await;

    let detail = store.token_detail("mcp_srv1_remote_tool").await.unwrap();
    assert_eq!(detail.short_desc, "v2");
    assert_eq!(detail.endpoint, Some(endpoint));
}

#[tokio::test]
async fn ingest_tool_registry_populates_endpoint_only_for_mcp_sourced_definitions() {
    let registry = ToolRegistry::new();
    let store = InMemorySquireStore::new();
    let mut endpoints = HashMap::new();
    endpoints.insert(
        "run_terminal".to_string(),
        ToolEndpoint::Mcp {
            server: fake_mcp_server("srv1").into(),
            remote_name: "remote_terminal".to_string(),
        },
    );

    ingest_tool_registry(&registry, &store, &endpoints).await;

    let terminal_detail = store.token_detail("run_terminal").await.unwrap();
    assert!(terminal_detail.endpoint.is_some());
    let web_fetch_detail = store.token_detail("web_fetch").await.unwrap();
    assert!(
        web_fetch_detail.endpoint.is_none(),
        "a tool absent from the endpoints map must not get one synthesized"
    );
}

#[tokio::test]
async fn ingest_tool_registry_with_empty_endpoints_map_matches_pre_existing_behavior() {
    let registry = ToolRegistry::new();
    let store = InMemorySquireStore::new();
    ingest_tool_registry(&registry, &store, &HashMap::new()).await;

    for def in registry.definitions() {
        let detail = store.token_detail(&def.name).await.unwrap();
        assert!(detail.endpoint.is_none());
    }
}

#[tokio::test]
async fn token_to_detail_tool_output_never_leaks_endpoint_data() {
    let store = Arc::new(InMemorySquireStore::new());
    let mut server = fake_mcp_server("srv1");
    server
        .env
        .insert("API_KEY".to_string(), "super-secret-value".to_string());
    store
        .upsert_token(
            NewTokenSpec {
                id: "mcp_srv1_remote_tool".to_string(),
                token_type: "tool".to_string(),
                short_desc: "an mcp tool".to_string(),
                full_desc: Some("full description".to_string()),
                endpoint: Some(ToolEndpoint::Mcp {
                    server: server.into(),
                    remote_name: "remote_tool".to_string(),
                }),
                ranges: vec![],
            },
            0,
            SessionId::nil(),
        )
        .await;
    let tool = SquireTokenToDetailTool {
        store: store.clone(),
    };

    let short = tool
        .execute("call-1", serde_json::json!({"token_id": "mcp_srv1_remote_tool"}))
        .await;
    assert!(!short.output.contains("super-secret-value"));
    assert!(!short.output.contains("srv1"));

    let full = tool
        .execute(
            "call-2",
            serde_json::json!({"token_id": "mcp_srv1_remote_tool", "detail_level": "full"}),
        )
        .await;
    assert!(!full.output.contains("super-secret-value"));
    assert!(!full.output.contains("srv1"));
}

#[tokio::test]
async fn token_to_detail_tool_returns_tool_schema_from_store() {
    let mut registry = ToolRegistry::empty();
    registry.register(Box::new(crate::agent::TerminalTool));
    let store = Arc::new(InMemorySquireStore::new());
    ingest_tool_registry(&registry, store.as_ref(), &HashMap::new()).await;
    let tool = SquireTokenToDetailTool {
        store: store.clone(),
    };

    let result = tool
        .execute(
            "call-1",
            serde_json::json!({"token_id": "run_terminal", "detail_level": "full"}),
        )
        .await;
    assert!(!result.is_error);
    let parsed: Value = serde_json::from_str(&result.output).unwrap();
    assert_eq!(parsed["name"], "run_terminal");
    assert!(parsed["input_schema"].is_object());
}

// ---- tool-token ingestion (ss-9) ----

#[tokio::test]
async fn ingest_tool_registry_writes_a_token_per_registry_tool() {
    let registry = ToolRegistry::new();
    let store = InMemorySquireStore::new();

    ingest_tool_registry(&registry, &store, &HashMap::new()).await;

    assert!(store.token_exists("run_terminal").await);
    assert!(store.token_exists("web_fetch").await);
    let detail = store.token_detail("run_terminal").await.unwrap();
    assert!(!detail.short_desc.is_empty());
    assert_eq!(detail.short_desc, registry.get("run_terminal").unwrap().description());
}

#[tokio::test]
async fn ingest_tool_registry_token_id_matches_registry_name_exactly() {
    let registry = ToolRegistry::new();
    let store = InMemorySquireStore::new();
    ingest_tool_registry(&registry, &store, &HashMap::new()).await;

    for def in registry.definitions() {
        assert!(
            store.token_exists(&def.name).await,
            "expected a token with id exactly '{}' (the registry name, unprefixed)",
            def.name
        );
    }
}

#[tokio::test]
async fn ingest_tool_registry_full_desc_matches_token_to_detail_tools_own_full_shape() {
    let mut registry = ToolRegistry::empty();
    registry.register(Box::new(crate::agent::TerminalTool));
    let store = InMemorySquireStore::new();

    ingest_tool_registry(&registry, &store, &HashMap::new()).await;

    let detail = store.token_detail("run_terminal").await.unwrap();
    let full_desc = detail.full_desc.expect("tool tokens must carry a full_desc");
    let parsed: Value = serde_json::from_str(&full_desc).unwrap();
    assert_eq!(parsed["name"], "run_terminal");
    assert!(parsed["description"].is_string());
    assert!(parsed["input_schema"].is_object());
}

#[tokio::test]
async fn ingest_tool_registry_is_idempotent_and_updates_rather_than_duplicates() {
    let registry = ToolRegistry::new();
    let store = InMemorySquireStore::new();

    ingest_tool_registry(&registry, &store, &HashMap::new()).await;
    ingest_tool_registry(&registry, &store, &HashMap::new()).await;
    ingest_tool_registry(&registry, &store, &HashMap::new()).await;

    let results = store.explore_memory("tool", "", 0, 100, 0, SessionId::nil()).await;
    assert_eq!(results.len(), registry.definitions().len());

    let ids: std::collections::HashSet<&str> =
        results.iter().map(|t| t.token_id.as_str()).collect();
    assert_eq!(ids.len(), results.len(), "no duplicate token ids expected");
}

#[tokio::test]
async fn ingest_tool_registry_reflects_schema_change_on_next_ingestion() {
    struct FakeToolV1;
    #[async_trait]
    impl Tool for FakeToolV1 {
        fn name(&self) -> &str { "fake_tool" }
        fn description(&self) -> &str { "version one" }
        fn input_schema(&self) -> Value { serde_json::json!({"type": "object"}) }
        async fn execute(&self, call_id: &str, _args: Value) -> ToolResult {
            ToolResult { call_id: call_id.to_string(), output: String::new(), is_error: false }
        }
    }
    struct FakeToolV2;
    #[async_trait]
    impl Tool for FakeToolV2 {
        fn name(&self) -> &str { "fake_tool" }
        fn description(&self) -> &str { "version two" }
        fn input_schema(&self) -> Value { serde_json::json!({"type": "object"}) }
        async fn execute(&self, call_id: &str, _args: Value) -> ToolResult {
            ToolResult { call_id: call_id.to_string(), output: String::new(), is_error: false }
        }
    }

    let store = InMemorySquireStore::new();
    let mut registry_v1 = ToolRegistry::empty();
    registry_v1.register(Box::new(FakeToolV1));
    ingest_tool_registry(&registry_v1, &store, &HashMap::new()).await;
    assert_eq!(store.token_detail("fake_tool").await.unwrap().short_desc, "version one");

    let mut registry_v2 = ToolRegistry::empty();
    registry_v2.register(Box::new(FakeToolV2));
    ingest_tool_registry(&registry_v2, &store, &HashMap::new()).await;
    assert_eq!(store.token_detail("fake_tool").await.unwrap().short_desc, "version two");

    let results = store.explore_memory("tool", "", 0, 100, 0, SessionId::nil()).await;
    assert_eq!(results.iter().filter(|t| t.token_id == "fake_tool").count(), 1);
}

#[tokio::test]
async fn ingested_tool_tokens_are_discoverable_via_explore_tool_skill_type_filter() {
    let registry = ToolRegistry::new();
    let store = InMemorySquireStore::new();
    ingest_tool_registry(&registry, &store, &HashMap::new()).await;

    let by_tool_type = store.explore_memory("tool", "", 0, 100, 0, SessionId::nil()).await;
    assert_eq!(by_tool_type.len(), registry.definitions().len());

    let by_all = store.explore_memory("all", "", 0, 100, 0, SessionId::nil()).await;
    assert!(by_all.len() >= registry.definitions().len());
}

#[tokio::test]
async fn ingest_tool_registry_with_empty_registry_writes_nothing() {
    let registry = ToolRegistry::empty();
    let store = InMemorySquireStore::new();
    ingest_tool_registry(&registry, &store, &HashMap::new()).await;
    let results = store.explore_memory("tool", "", 0, 100, 0, SessionId::nil()).await;
    assert!(results.is_empty());
}

// ---- user-input auto-chunking ----

#[test]
fn chunk_user_input_short_message_is_one_chunk() {
    let chunks = chunk_user_input("What's the weather like today?");
    assert_eq!(chunks, vec!["What's the weather like today?".to_string()]);
}

#[test]
fn chunk_user_input_empty_or_whitespace_produces_no_chunks() {
    assert!(chunk_user_input("").is_empty());
    assert!(chunk_user_input("   \n\n  ").is_empty());
}

#[test]
fn chunk_user_input_splits_on_blank_line_paragraph_boundaries() {
    let text = "First paragraph here.\n\nSecond paragraph here.\n\nThird one.";
    let chunks = chunk_user_input(text);
    assert_eq!(
        chunks,
        vec![
            "First paragraph here.".to_string(),
            "Second paragraph here.".to_string(),
            "Third one.".to_string(),
        ]
    );
}

#[test]
fn chunk_user_input_short_paragraph_is_not_sentence_split() {
    let text = "Hi there. How are you doing today?";
    let chunks = chunk_user_input(text);
    assert_eq!(chunks, vec![text.to_string()]);
}

#[test]
fn chunk_user_input_long_paragraph_is_split_into_sentences() {
    let sentence_a = "A".repeat(200) + ".";
    let sentence_b = "B".repeat(200) + ".";
    let sentence_c = "C".repeat(200) + ".";
    let long_paragraph = format!("{} {} {}", sentence_a, sentence_b, sentence_c);
    assert!(long_paragraph.len() > CHUNK_SOFT_LIMIT_CHARS);

    let chunks = chunk_user_input(&long_paragraph);
    assert_eq!(chunks, vec![sentence_a, sentence_b, sentence_c]);
}

#[test]
fn chunk_user_input_handles_multiple_long_paragraphs_independently() {
    let para1 = format!("{} {}", "X".repeat(250) + ".", "Y".repeat(250) + ".");
    let para2 = "A short second paragraph.";
    let text = format!("{}\n\n{}", para1, para2);

    let chunks = chunk_user_input(&text);
    assert_eq!(chunks.len(), 3);
    assert_eq!(chunks[2], "A short second paragraph.");
}

#[test]
fn first_sentence_extracts_up_to_first_terminator() {
    assert_eq!(
        first_sentence("This is the first sentence. This is the second."),
        "This is the first sentence."
    );
}

#[test]
fn first_sentence_returns_whole_chunk_when_no_terminator_found() {
    assert_eq!(first_sentence("no terminator here"), "no terminator here");
}

#[test]
fn first_sentence_stops_at_newline() {
    assert_eq!(first_sentence("line one\nline two"), "line one");
}

#[tokio::test]
async fn ingest_user_input_chunks_writes_one_token_per_chunk_with_expected_id_scheme() {
    let store = InMemorySquireStore::new();
    let text = "First paragraph.\n\nSecond paragraph.";
    ingest_user_input_chunks(text, 3, &store, SessionId::nil()).await;

    assert!(store.token_exists("USR_T3_001_00000000").await);
    assert!(store.token_exists("USR_T3_002_00000000").await);
    assert!(!store.token_exists("USR_T3_003_00000000").await);

    let d1 = store.token_detail("USR_T3_001_00000000").await.unwrap();
    assert_eq!(d1.short_desc, "First paragraph.");
    assert_eq!(d1.full_desc, Some("§^chunk_0§^First paragraph.".to_string()));

    let d2 = store.token_detail("USR_T3_002_00000000").await.unwrap();
    assert_eq!(d2.short_desc, "Second paragraph.");
    assert_eq!(d2.full_desc, Some("§^chunk_1§^Second paragraph.".to_string()));
}

#[tokio::test]
async fn ingest_user_input_chunks_uses_system_referential_type_discoverable_via_explore() {
    let store = InMemorySquireStore::new();
    ingest_user_input_chunks("Some chat message content.", 1, &store, SessionId::nil()).await;

    let results = store
        .explore_memory("system_referential", "chat message", 0, 10, 1, SessionId::nil())
        .await;
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].token_id, "USR_T1_001_00000000");
    assert_eq!(results[0].token_type, "system_referential");

    let via_all = store.explore_memory("all", "", 0, 100, 1, SessionId::nil()).await;
    assert!(via_all.iter().any(|t| t.token_id == "USR_T1_001_00000000"));
}

#[tokio::test]
async fn explore_memory_alias_includes_system_referential_tokens() {
    let store = InMemorySquireStore::new();
    ingest_user_input_chunks("Some chat message content.", 1, &store, SessionId::nil()).await;

    let via_memory = store
        .explore_memory("memory", "chat message", 0, 10, 1, SessionId::nil())
        .await;
    assert!(via_memory.iter().any(|t| t.token_id == "USR_T1_001_00000000"));
}

#[tokio::test]
async fn ingest_user_input_chunks_sequence_resets_per_turn() {
    let store = InMemorySquireStore::new();
    ingest_user_input_chunks("Turn one paragraph A.\n\nTurn one paragraph B.", 1, &store, SessionId::nil())
        .await;
    ingest_user_input_chunks("Turn two single message.", 2, &store, SessionId::nil()).await;

    assert!(store.token_exists("USR_T1_001_00000000").await);
    assert!(store.token_exists("USR_T1_002_00000000").await);
    assert!(store.token_exists("USR_T2_001_00000000").await);
    assert!(!store.token_exists("USR_T2_002_00000000").await);
}

#[tokio::test]
async fn ingest_user_input_chunks_creation_turn_matches_the_turn_argument() {
    let store = InMemorySquireStore::new();
    ingest_user_input_chunks("Some content here.", 5, &store, SessionId::nil()).await;

    let results = store.explore_memory("all", "content", 0, 10, 5, SessionId::nil()).await;
    let chunk = results.iter().find(|t| t.token_id == "USR_T5_001_00000000").unwrap();
    assert_eq!(chunk.accumulated_hits, 1);
}

#[tokio::test]
async fn ingest_user_input_chunks_empty_text_writes_no_tokens() {
    let store = InMemorySquireStore::new();
    ingest_user_input_chunks("   ", 1, &store, SessionId::nil()).await;
    let results = store.explore_memory("system_referential", "", 0, 10, 1, SessionId::nil()).await;
    assert!(results.is_empty());
}

#[tokio::test]
async fn build_turn_input_ingests_user_message_as_system_referential_chunk_same_turn() {
    let store = Arc::new(InMemorySquireStore::new());
    let mut adapter = SquireContextAdapter::new(store.clone());
    let session = fixture_session("Please summarize the quarterly report for me.");

    let turn_input = adapter.build_turn_input(&session, &[]).await.unwrap();

    let session_short = &session.session.id.simple().to_string()[..8];
    let usr_id = format!("USR_T0_001_{}", session_short);
    assert!(store.token_exists(&usr_id).await);
    let detail = store.token_detail(&usr_id).await.unwrap();
    assert_eq!(detail.full_desc.as_deref(), Some("§^chunk_0§^Please summarize the quarterly report for me."));

    let ctx = extract_context(&turn_input.messages[0].content);
    // The user chunk USR_T0_001_* has full_desc, so it is in expanded_tokens.
    let expanded = ctx["expanded_tokens"].as_array().unwrap();
    assert!(expanded
        .iter()
        .any(|t| t["token_id"] == usr_id));
}

#[tokio::test]
async fn build_turn_input_chunks_multi_paragraph_user_message_into_multiple_tokens() {
    let store = Arc::new(InMemorySquireStore::new());
    let mut adapter = SquireContextAdapter::new(store.clone());
    let session = fixture_session("First point to discuss.\n\nSecond point to discuss.");

    adapter.build_turn_input(&session, &[]).await.unwrap();

    let session_short = &session.session.id.simple().to_string()[..8];
    assert!(store.token_exists(&format!("USR_T0_001_{}", session_short)).await);
    assert!(store.token_exists(&format!("USR_T0_002_{}", session_short)).await);
}

#[tokio::test]
async fn ingest_user_input_chunks_does_not_write_relationships() {
    let store = Arc::new(InMemorySquireStore::new());
    ingest_user_input_chunks("Some content.\n\nMore content.", 1, store.as_ref(), SessionId::nil()).await;

    let results = store
        .explore_memory("system_referential", "", 1, 100, 1, SessionId::nil())
        .await;
    assert!(results.iter().all(|t| t.hop_distance == 0));
}

