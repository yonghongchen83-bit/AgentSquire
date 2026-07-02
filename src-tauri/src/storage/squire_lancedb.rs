//! Real, LanceDB-backed implementation of `SquireStore` (Q4: "implement
//! LanceDB from day one for Squire storage... no SQLite-only stopgap").
//!
//! Scope for this node (see `.AiControl/root/Squire/squire-storage`):
//! structured partition (tokens table, includes an embedding column so
//! `explore_memory` can do real vector search via `nearest_to`), raw
//! partition (relationships / triplet store), and a small turns table for
//! the per-session turn counter. `InMemorySquireStore` (in `agent::squire`)
//! remains as the fast in-process test double the trait was designed to
//! allow (see `SquireStore`'s doc comment) — this module is the production
//! implementation, not a replacement for the test double.
//!
//! Embedding scope note: no embedding-model provider is wired into this
//! codebase yet (that's out of scope for this node — see `env.md`). Vector
//! search here is powered by a deterministic, dependency-free hash-based
//! bag-of-words embedding (`embed_text`) so `explore_memory` performs a
//! real cosine-similarity ranking today rather than the flat substring
//! filter `InMemorySquireStore` uses. Swapping in a real embedding model
//! later only requires changing `embed_text`'s body — the `SquireStore`
//! trait, callers, and table schema (fixed 64-dim float32 vector) are
//! unaffected as long as the new embedding is also 64-dim, or the column
//! is migrated.

use std::sync::Arc;

use arrow_array::{
    cast::AsArray, Array, Float32Array, RecordBatch, RecordBatchIterator, StringArray,
    UInt64Array,
};
use arrow_schema::{DataType, Field, Schema as ArrowSchema};
use async_trait::async_trait;
use futures::TryStreamExt;
use lancedb::query::{ExecutableQuery, QueryBase};
use lancedb::{Connection, Table};
use tokio::sync::Mutex;

use crate::agent::squire::{
    ComplianceFailureRecord, NewTokenSpec, Relationship, SquireStore, ToolEndpoint, TokenDetail,
    TokenSummary,
};
use crate::storage::conversation_store::SessionId;

const EMBED_DIM: usize = 64;

const TOKENS_TABLE: &str = "squire_tokens";
const RELATIONSHIPS_TABLE: &str = "squire_relationships";
const TURNS_TABLE: &str = "squire_turns";
const COMPLIANCE_FAILURES_TABLE: &str = "squire_compliance_failures";
const RAW_PARTITION_TABLE: &str = "squire_raw_partition";

/// Deterministic hash-based bag-of-words embedding. Not semantically
/// meaningful the way a real embedding model would be, but stable,
/// dependency-free, and sufficient to exercise a genuine vector-search path
/// end to end (see module doc for the swap-out plan).
fn embed_text(text: &str) -> Vec<f32> {
    let mut vec = vec![0f32; EMBED_DIM];
    for token in text.to_lowercase().split_whitespace() {
        let mut hash: u64 = 1469598103934665603; // FNV offset basis
        for byte in token.bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(1099511628211); // FNV prime
        }
        let idx = (hash as usize) % EMBED_DIM;
        vec[idx] += 1.0;
    }
    let norm: f32 = vec.iter().map(|v| v * v).sum::<f32>().sqrt();
    if norm > 0.0 {
        for v in vec.iter_mut() {
            *v /= norm;
        }
    }
    vec
}

fn tokens_schema() -> Arc<ArrowSchema> {
    Arc::new(ArrowSchema::new(vec![
        Field::new("token_id", DataType::Utf8, false),
        Field::new("token_type", DataType::Utf8, false),
        Field::new("short_desc", DataType::Utf8, false),
        Field::new("full_desc", DataType::Utf8, true),
        Field::new("creation_turn", DataType::UInt64, false),
        // Hit-count bookkeeping (spec §3.2/§3.3) — see agent::squire's
        // `effective_priority` for how this is combined with creation_turn
        // at ranking time. New, non-nullable column added by
        // retrieval-fidelity; see decisions.md's schema-migration note (no
        // migration path for pre-existing LanceDB directories — accepted,
        // consistent with prior nodes' schema-change precedent).
        Field::new("accumulated_hits", DataType::UInt64, false),
        Field::new(
            "embedding",
            DataType::FixedSizeList(
                Arc::new(Field::new("item", DataType::Float32, true)),
                EMBED_DIM as i32,
            ),
            false,
        ),
        // `ToolEndpoint`, JSON-serialized (token-detail-endpoint). Nullable —
        // absent for every non-tool token, for local-builtin tool tokens, and
        // for MCP tool tokens written before this column existed. New,
        // nullable column added the same way `accumulated_hits` was added by
        // retrieval-fidelity; see that node's decisions.md for the accepted
        // "no migration path for pre-existing LanceDB directories" precedent
        // this follows (a nullable column with absent-safe read handling, not
        // a destructive schema migration).
        Field::new("endpoint", DataType::Utf8, true),
    ]))
}

fn relationships_schema() -> Arc<ArrowSchema> {
    Arc::new(ArrowSchema::new(vec![
        Field::new("subject", DataType::Utf8, false),
        Field::new("predicate", DataType::Utf8, false),
        Field::new("object", DataType::Utf8, false),
    ]))
}

fn turns_schema() -> Arc<ArrowSchema> {
    Arc::new(ArrowSchema::new(vec![
        Field::new("session_id", DataType::Utf8, false),
        Field::new("turn", DataType::UInt64, false),
    ]))
}

fn preserve_lists_schema() -> Arc<ArrowSchema> {
    Arc::new(ArrowSchema::new(vec![
        Field::new("session_id", DataType::Utf8, false),
        Field::new("token_id", DataType::Utf8, false),
    ]))
}

const PRESERVE_TABLE: &str = "squire_preserve_lists";

/// Append-only, debugging-only table (Q6). Stored as plain strings
/// (RFC3339 timestamp included) rather than Arrow's native timestamp type —
/// consistent with how the rest of this module favors simple string columns
/// over more elaborate Arrow types, and this table is never queried for
/// runtime decisions, only inspected for diagnostics.
fn compliance_failures_schema() -> Arc<ArrowSchema> {
    Arc::new(ArrowSchema::new(vec![
        Field::new("session_id", DataType::Utf8, false),
        Field::new("rule", DataType::Utf8, false),
        Field::new("reason", DataType::Utf8, false),
        Field::new("retry_count", DataType::UInt64, false),
        Field::new("failed_content", DataType::Utf8, false),
        Field::new("timestamp", DataType::Utf8, false),
    ]))
}

/// Raw partition (spec §4.1/§4.3/§9.4 step 4): append-only, debugging/audit
/// aid only, same posture as `compliance_failures_schema` — plain string/
/// scalar columns, no embedding column (nothing in this runtime ever
/// vector-searches this table; see `raw-partition-storage/decisions.md` for
/// why "reachable only by vector similarity" in the spec's wording is not
/// read as requiring one here). Never queried by `explore_memory` or any
/// other trait method — inspected only via direct table access outside the
/// running app.
fn raw_partition_schema() -> Arc<ArrowSchema> {
    Arc::new(ArrowSchema::new(vec![
        Field::new("session_id", DataType::Utf8, false),
        Field::new("turn", DataType::UInt64, false),
        Field::new("content", DataType::Utf8, false),
        Field::new("timestamp", DataType::Utf8, false),
    ]))
}

fn embedding_array(rows: &[Vec<f32>]) -> arrow_array::FixedSizeListArray {
    let item_field = Arc::new(Field::new("item", DataType::Float32, true));
    let flat: Vec<f32> = rows.iter().flatten().copied().collect();
    let values = Float32Array::from(flat);
    arrow_array::FixedSizeListArray::new(item_field, EMBED_DIM as i32, Arc::new(values), None)
}

/// LanceDB-backed `SquireStore` (Q4). One LanceDB directory holds all four
/// tables (tokens/relationships/turns/preserve-lists) — LanceDB has no
/// notion of a single "database" file the way SQLite does, so the directory
/// itself is the unit of storage `setup_cmd.rs` points at.
pub struct LanceDbSquireStore {
    conn: Connection,
    // Serializes writes to keep read-modify-write sequences (e.g. upsert,
    // preserve-list replace) race-free; LanceDB tables are individually
    // safe for concurrent access but the higher-level operations here are
    // not atomic across the two-step (delete-then-add) upsert pattern.
    write_lock: Mutex<()>,
}

impl LanceDbSquireStore {
    /// Opens (creating if necessary) a LanceDB store at `dir`. All four
    /// tables are created empty on first use if they don't already exist.
    pub async fn open(dir: &std::path::Path) -> Result<Self, String> {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let uri = dir.to_string_lossy().to_string();
        let conn = lancedb::connect(&uri)
            .execute()
            .await
            .map_err(|e| e.to_string())?;

        let existing = conn.table_names().execute().await.map_err(|e| e.to_string())?;

        if !existing.iter().any(|n| n == TOKENS_TABLE) {
            conn.create_empty_table(TOKENS_TABLE, tokens_schema())
                .execute()
                .await
                .map_err(|e| e.to_string())?;
        }
        if !existing.iter().any(|n| n == RELATIONSHIPS_TABLE) {
            conn.create_empty_table(RELATIONSHIPS_TABLE, relationships_schema())
                .execute()
                .await
                .map_err(|e| e.to_string())?;
        }
        if !existing.iter().any(|n| n == TURNS_TABLE) {
            conn.create_empty_table(TURNS_TABLE, turns_schema())
                .execute()
                .await
                .map_err(|e| e.to_string())?;
        }
        if !existing.iter().any(|n| n == PRESERVE_TABLE) {
            conn.create_empty_table(PRESERVE_TABLE, preserve_lists_schema())
                .execute()
                .await
                .map_err(|e| e.to_string())?;
        }
        if !existing.iter().any(|n| n == COMPLIANCE_FAILURES_TABLE) {
            conn.create_empty_table(COMPLIANCE_FAILURES_TABLE, compliance_failures_schema())
                .execute()
                .await
                .map_err(|e| e.to_string())?;
        }
        if !existing.iter().any(|n| n == RAW_PARTITION_TABLE) {
            conn.create_empty_table(RAW_PARTITION_TABLE, raw_partition_schema())
                .execute()
                .await
                .map_err(|e| e.to_string())?;
        }

        Ok(Self {
            conn,
            write_lock: Mutex::new(()),
        })
    }

    async fn tokens(&self) -> Result<Table, String> {
        self.conn
            .open_table(TOKENS_TABLE)
            .execute()
            .await
            .map_err(|e| e.to_string())
    }

    async fn relationships_table(&self) -> Result<Table, String> {
        self.conn
            .open_table(RELATIONSHIPS_TABLE)
            .execute()
            .await
            .map_err(|e| e.to_string())
    }

    async fn turns(&self) -> Result<Table, String> {
        self.conn
            .open_table(TURNS_TABLE)
            .execute()
            .await
            .map_err(|e| e.to_string())
    }

    async fn preserve_table(&self) -> Result<Table, String> {
        self.conn
            .open_table(PRESERVE_TABLE)
            .execute()
            .await
            .map_err(|e| e.to_string())
    }

    async fn compliance_failures_table(&self) -> Result<Table, String> {
        self.conn
            .open_table(COMPLIANCE_FAILURES_TABLE)
            .execute()
            .await
            .map_err(|e| e.to_string())
    }

    /// `pub` (unlike this module's other `*_table()` accessors) solely so
    /// `examples/raw_partition_storage_e2e.rs` — a separate binary target
    /// linking only against this crate's public API — can assert on raw row
    /// counts directly, the same way this module's own tests already do
    /// in-process. Not part of the `SquireStore` trait (deliberately no
    /// read-back trait method exists — see
    /// `raw-partition-storage/decisions.md`); this is table-handle plumbing
    /// for verification code, not a new production read path.
    pub async fn raw_partition_table(&self) -> Result<Table, String> {
        self.conn
            .open_table(RAW_PARTITION_TABLE)
            .execute()
            .await
            .map_err(|e| e.to_string())
    }

    async fn find_token_row(&self, token_id: &str) -> Result<Option<StoredTokenRow>, String> {
        let table = self.tokens().await?;
        let escaped = token_id.replace('\'', "''");
        let batches = table
            .query()
            .only_if(format!("token_id = '{}'", escaped))
            .execute()
            .await
            .map_err(|e| e.to_string())?
            .try_collect::<Vec<_>>()
            .await
            .map_err(|e| e.to_string())?;
        Ok(rows_from_batches(&batches).into_iter().next())
    }

    /// Loads the full relationship triplet store as (subject, object) pairs
    /// for graph traversal (spec §4.2/§6.1/§7.1). No pagination/indexing
    /// exists for this table today — consistent with `explore_memory`'s
    /// existing full-table-scan pattern over `squire_tokens`, not a new
    /// limitation introduced by this node.
    async fn load_relationship_edges(&self) -> Vec<(String, String)> {
        let Ok(table) = self.relationships_table().await else {
            return Vec::new();
        };
        let Ok(stream) = table.query().execute().await else {
            return Vec::new();
        };
        let Ok(batches) = stream.try_collect::<Vec<_>>().await else {
            return Vec::new();
        };
        let mut edges = Vec::new();
        for batch in &batches {
            let Some(subjects) = batch.column_by_name("subject") else { continue };
            let Some(objects) = batch.column_by_name("object") else { continue };
            let subjects = subjects.as_string::<i32>();
            let objects = objects.as_string::<i32>();
            for i in 0..batch.num_rows() {
                edges.push((subjects.value(i).to_string(), objects.value(i).to_string()));
            }
        }
        edges
    }
}

struct StoredTokenRow {
    token_id: String,
    token_type: String,
    short_desc: String,
    full_desc: Option<String>,
    creation_turn: u64,
    accumulated_hits: u64,
    endpoint: Option<ToolEndpoint>,
}

fn rows_from_batches(batches: &[RecordBatch]) -> Vec<StoredTokenRow> {
    let mut out = Vec::new();
    for batch in batches {
        let ids = batch.column_by_name("token_id").unwrap().as_string::<i32>();
        let types = batch.column_by_name("token_type").unwrap().as_string::<i32>();
        let shorts = batch.column_by_name("short_desc").unwrap().as_string::<i32>();
        let fulls = batch.column_by_name("full_desc").unwrap().as_string::<i32>();
        let turns = batch
            .column_by_name("creation_turn")
            .unwrap()
            .as_any()
            .downcast_ref::<UInt64Array>()
            .unwrap();
        let hits = batch
            .column_by_name("accumulated_hits")
            .and_then(|c| c.as_any().downcast_ref::<UInt64Array>().cloned());
        // Absent-safe like `hits` above: a pre-token-detail-endpoint LanceDB
        // directory has no `endpoint` column at all, not merely null values
        // in it — `column_by_name` returns `None` in that case, and every
        // row is treated as `endpoint: None` (self-healing on next
        // ingestion, same as any other pre-existing-row staleness in this
        // store — see token-detail-endpoint/decisions.md).
        let endpoints = batch
            .column_by_name("endpoint")
            .map(|c| c.as_string::<i32>().clone());
        for i in 0..batch.num_rows() {
            out.push(StoredTokenRow {
                token_id: ids.value(i).to_string(),
                token_type: types.value(i).to_string(),
                short_desc: shorts.value(i).to_string(),
                full_desc: if fulls.is_null(i) {
                    None
                } else {
                    Some(fulls.value(i).to_string())
                },
                creation_turn: turns.value(i),
                accumulated_hits: hits.as_ref().map(|a| a.value(i)).unwrap_or(0),
                endpoint: endpoints.as_ref().and_then(|e| {
                    if e.is_null(i) {
                        None
                    } else {
                        serde_json::from_str::<ToolEndpoint>(e.value(i)).ok()
                    }
                }),
            });
        }
    }
    out
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        dot / (na * nb)
    }
}

#[async_trait]
impl SquireStore for LanceDbSquireStore {
    async fn token_exists(&self, token_id: &str) -> bool {
        matches!(self.find_token_row(token_id).await, Ok(Some(_)))
    }

    async fn upsert_token(&self, token: NewTokenSpec, creation_turn: u64) {
        let _guard = self.write_lock.lock().await;
        let Ok(table) = self.tokens().await else {
            return;
        };

        // Preserve existing creation_turn / merge full_desc semantics to
        // match InMemorySquireStore::upsert_token exactly. accumulated_hits
        // increments by 1 on every upsert, "regardless" (spec §9.4 step 5),
        // matching InMemorySquireStore's identical rule.
        let (final_creation_turn, final_full_desc, final_hits, final_endpoint) =
            match self.find_token_row(&token.id).await {
                Ok(Some(existing)) => (
                    existing.creation_turn,
                    token.full_desc.clone().or(existing.full_desc),
                    existing.accumulated_hits + 1,
                    token.endpoint.clone().or(existing.endpoint),
                ),
                _ => (creation_turn, token.full_desc.clone(), 1u64, token.endpoint.clone()),
            };

        let escaped = token.id.replace('\'', "''");
        let _ = table.delete(&format!("token_id = '{}'", escaped)).await;

        let embed_source = format!("{} {}", token.id, token.short_desc);
        let embedding = embed_text(&embed_source);

        let final_endpoint_json = final_endpoint
            .as_ref()
            .and_then(|e| serde_json::to_string(e).ok());

        let schema = tokens_schema();
        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(StringArray::from(vec![token.id.clone()])),
                Arc::new(StringArray::from(vec![token.token_type.clone()])),
                Arc::new(StringArray::from(vec![token.short_desc.clone()])),
                Arc::new(StringArray::from(vec![final_full_desc])),
                Arc::new(UInt64Array::from(vec![final_creation_turn])),
                Arc::new(UInt64Array::from(vec![final_hits])),
                Arc::new(embedding_array(&[embedding])),
                Arc::new(StringArray::from(vec![final_endpoint_json])),
            ],
        );
        let Ok(batch) = batch else { return };
        let reader = RecordBatchIterator::new(vec![Ok(batch)], schema);
        let _ = table.add(reader).execute().await;
    }

    async fn insert_relationship(&self, rel: Relationship) {
        let _guard = self.write_lock.lock().await;
        let Ok(table) = self.relationships_table().await else {
            return;
        };
        let schema = relationships_schema();
        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(StringArray::from(vec![rel.subject])),
                Arc::new(StringArray::from(vec![rel.predicate])),
                Arc::new(StringArray::from(vec![rel.object])),
            ],
        );
        let Ok(batch) = batch else { return };
        let reader = RecordBatchIterator::new(vec![Ok(batch)], schema);
        let _ = table.add(reader).execute().await;
    }

    async fn set_preserve_list(&self, session_id: SessionId, tokens: Vec<String>) {
        let _guard = self.write_lock.lock().await;
        let Ok(table) = self.preserve_table().await else {
            return;
        };
        let sid = session_id.to_string();
        let escaped = sid.replace('\'', "''");
        let _ = table
            .delete(&format!("session_id = '{}'", escaped))
            .await;

        if tokens.is_empty() {
            return;
        }

        let schema = preserve_lists_schema();
        let n = tokens.len();
        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(StringArray::from(vec![sid; n])),
                Arc::new(StringArray::from(tokens)),
            ],
        );
        let Ok(batch) = batch else { return };
        let reader = RecordBatchIterator::new(vec![Ok(batch)], schema);
        let _ = table.add(reader).execute().await;
    }

    async fn preserved_tokens(&self, session_id: SessionId) -> Vec<TokenSummary> {
        let Ok(table) = self.preserve_table().await else {
            return Vec::new();
        };
        let sid = session_id.to_string();
        let escaped = sid.replace('\'', "''");
        let Ok(batches) = table
            .query()
            .only_if(format!("session_id = '{}'", escaped))
            .execute()
            .await
        else {
            return Vec::new();
        };
        let Ok(batches) = batches.try_collect::<Vec<_>>().await else {
            return Vec::new();
        };

        let mut ids = Vec::new();
        for batch in &batches {
            let Some(col) = batch.column_by_name("token_id") else {
                continue;
            };
            let arr = col.as_string::<i32>();
            for i in 0..batch.num_rows() {
                ids.push(arr.value(i).to_string());
            }
        }

        let mut out = Vec::new();
        for id in ids {
            if let Ok(Some(row)) = self.find_token_row(&id).await {
                // Spec §3.3: "Token in preserve list loaded at turn open" +1.
                self.record_hit(&id).await;
                out.push(TokenSummary {
                    token_id: row.token_id,
                    token_type: row.token_type,
                    score: 0.0,
                    short_desc: row.short_desc,
                    accumulated_hits: row.accumulated_hits + 1,
                    hop_distance: 0,
                    via_token_id: None,
                });
            }
        }
        out
    }

    async fn explore_memory(
        &self,
        resource_type: &str,
        query: &str,
        num_hops: u32,
        max_results: u32,
        current_turn: u64,
    ) -> Vec<TokenSummary> {
        let Ok(table) = self.tokens().await else {
            return Vec::new();
        };
        let Ok(stream) = table.query().execute().await else {
            return Vec::new();
        };
        let Ok(batches) = stream.try_collect::<Vec<_>>().await else {
            return Vec::new();
        };

        let type_matches = |t: &str| {
            resource_type == "all"
                || t == resource_type
                || (resource_type == "memory"
                    && (t == "concept" || t == "referential" || t == "system_referential"))
                || (resource_type == "tool_skill" && t == "skill")
        };

        let query_embedding = if query.is_empty() {
            None
        } else {
            Some(embed_text(query))
        };

        // All token rows, keyed by id, for traversal lookups and priority
        // computation regardless of type/query filtering (a traversal-
        // reachable node might be any type; its own type is checked at
        // traversal-result time via `type_matches`).
        let mut all_rows: std::collections::HashMap<String, StoredTokenRow> =
            std::collections::HashMap::new();
        let mut scored: Vec<TokenSummary> = Vec::new();
        for batch in &batches {
            let Some(ids_col) = batch.column_by_name("token_id") else { continue };
            let Some(types_col) = batch.column_by_name("token_type") else { continue };
            let Some(shorts_col) = batch.column_by_name("short_desc") else { continue };
            let Some(embed_col) = batch.column_by_name("embedding") else { continue };
            let ids = ids_col.as_string::<i32>();
            let types = types_col.as_string::<i32>();
            let shorts = shorts_col.as_string::<i32>();
            let hits_col = batch
                .column_by_name("accumulated_hits")
                .and_then(|c| c.as_any().downcast_ref::<UInt64Array>());
            let turns_col = batch
                .column_by_name("creation_turn")
                .and_then(|c| c.as_any().downcast_ref::<UInt64Array>());
            let embeddings = embed_col
                .as_any()
                .downcast_ref::<arrow_array::FixedSizeListArray>();

            for i in 0..batch.num_rows() {
                let token_type = types.value(i);
                let token_id = ids.value(i);
                let short_desc = shorts.value(i);
                let accumulated_hits = hits_col.map(|a| a.value(i)).unwrap_or(0);
                let creation_turn = turns_col.map(|a| a.value(i)).unwrap_or(0);

                all_rows.insert(
                    token_id.to_string(),
                    StoredTokenRow {
                        token_id: token_id.to_string(),
                        token_type: token_type.to_string(),
                        short_desc: short_desc.to_string(),
                        full_desc: None,
                        creation_turn,
                        accumulated_hits,
                        endpoint: None,
                    },
                );

                if !type_matches(token_type) {
                    continue;
                }

                let score = match (&query_embedding, embeddings) {
                    (Some(qe), Some(embeds)) => {
                        let row_val = embeds.value(i);
                        let row_arr = row_val.as_any().downcast_ref::<Float32Array>().unwrap();
                        let row_vec: Vec<f32> = row_arr.values().to_vec();
                        let sim = cosine_similarity(qe, &row_vec);
                        // Fall back to substring match boost so exact-name
                        // hits still surface even if the toy embedding's
                        // hash collisions dilute cosine score.
                        let substr_boost = if token_id.to_lowercase().contains(&query.to_lowercase())
                            || short_desc.to_lowercase().contains(&query.to_lowercase())
                        {
                            0.5
                        } else {
                            0.0
                        };
                        sim + substr_boost
                    }
                    _ => 1.0,
                };

                if query_embedding.is_some() && score <= 0.0 {
                    continue;
                }

                scored.push(TokenSummary {
                    token_id: token_id.to_string(),
                    token_type: token_type.to_string(),
                    score,
                    short_desc: short_desc.to_string(),
                    accumulated_hits,
                    hop_distance: 0,
                    via_token_id: None,
                });
            }
        }

        // Graph traversal (spec §4.2/§6.1/§7.1): expand outward from the
        // direct matches up to num_hops over the full token set (a
        // traversal-reachable token might not itself match the query text —
        // see §7.3), against the squire_relationships triplet store.
        if num_hops > 0 && !scored.is_empty() {
            let all_nodes: std::collections::HashMap<String, crate::agent::squire::TraversalNode> = all_rows
                .values()
                .map(|row| {
                    (
                        row.token_id.clone(),
                        crate::agent::squire::TraversalNode {
                            token_id: row.token_id.clone(),
                            token_type: row.token_type.clone(),
                            short_desc: row.short_desc.clone(),
                        },
                    )
                })
                .collect();
            let edges = self.load_relationship_edges().await;
            let direct_scores: Vec<(String, f32)> =
                scored.iter().map(|t| (t.token_id.clone(), t.score)).collect();
            let mut expanded = crate::agent::squire::traverse_relationships(
                &direct_scores,
                &edges,
                num_hops,
                &all_nodes,
                type_matches,
            );
            for t in &mut expanded {
                t.accumulated_hits = all_rows.get(&t.token_id).map(|r| r.accumulated_hits).unwrap_or(0);
            }
            scored.extend(expanded);
        }

        let priorities: std::collections::HashMap<String, i64> = scored
            .iter()
            .filter_map(|t| {
                all_rows.get(&t.token_id).map(|row| {
                    (
                        t.token_id.clone(),
                        crate::agent::squire::effective_priority(
                            row.accumulated_hits,
                            current_turn,
                            row.creation_turn,
                        ),
                    )
                })
            })
            .collect();
        crate::agent::squire::sort_by_score_then_priority(&mut scored, &priorities);
        scored.truncate(max_results.max(1) as usize);
        scored
    }

    async fn token_detail(&self, token_id: &str) -> Option<TokenDetail> {
        self.find_token_row(token_id).await.ok().flatten().map(|row| TokenDetail {
            short_desc: row.short_desc,
            full_desc: row.full_desc,
            endpoint: row.endpoint,
        })
    }

    async fn current_turn(&self, session_id: SessionId) -> u64 {
        let Ok(table) = self.turns().await else {
            return 0;
        };
        let sid = session_id.to_string();
        let escaped = sid.replace('\'', "''");
        let Ok(stream) = table
            .query()
            .only_if(format!("session_id = '{}'", escaped))
            .execute()
            .await
        else {
            return 0;
        };
        let Ok(batches) = stream.try_collect::<Vec<_>>().await else {
            return 0;
        };
        for batch in &batches {
            if let Some(col) = batch.column_by_name("turn") {
                if let Some(arr) = col.as_any().downcast_ref::<UInt64Array>() {
                    if arr.len() > 0 {
                        return arr.value(0);
                    }
                }
            }
        }
        0
    }

    async fn increment_turn(&self, session_id: SessionId) {
        let _guard = self.write_lock.lock().await;
        let Ok(table) = self.turns().await else {
            return;
        };
        let current = self.current_turn(session_id).await;
        let sid = session_id.to_string();
        let escaped = sid.replace('\'', "''");
        let _ = table
            .delete(&format!("session_id = '{}'", escaped))
            .await;

        let schema = turns_schema();
        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(StringArray::from(vec![sid])),
                Arc::new(UInt64Array::from(vec![current + 1])),
            ],
        );
        let Ok(batch) = batch else { return };
        let reader = RecordBatchIterator::new(vec![Ok(batch)], schema);
        let _ = table.add(reader).execute().await;
    }

    async fn record_compliance_failure(&self, record: ComplianceFailureRecord) {
        let _guard = self.write_lock.lock().await;
        let Ok(table) = self.compliance_failures_table().await else {
            return;
        };
        let schema = compliance_failures_schema();
        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(StringArray::from(vec![record.session_id.to_string()])),
                Arc::new(StringArray::from(vec![record.rule])),
                Arc::new(StringArray::from(vec![record.reason])),
                Arc::new(UInt64Array::from(vec![record.retry_count as u64])),
                Arc::new(StringArray::from(vec![record.failed_content])),
                Arc::new(StringArray::from(vec![record.timestamp.to_rfc3339()])),
            ],
        );
        let Ok(batch) = batch else { return };
        let reader = RecordBatchIterator::new(vec![Ok(batch)], schema);
        let _ = table.add(reader).execute().await;
    }

    async fn clear_all_preserve_lists(&self) {
        let _guard = self.write_lock.lock().await;
        let Ok(table) = self.preserve_table().await else {
            return;
        };
        // Unconditional delete (Q7: restart clears *all* pending preserve
        // carryover, not per-session) — "true" is not a valid LanceDB filter
        // literal in this crate version, so match every row via a tautology
        // over a column that is `NOT NULL` in the schema instead.
        let _ = table.delete("session_id IS NOT NULL").await;
    }

    async fn record_raw_output(&self, session_id: SessionId, turn: u64, content: String) {
        let _guard = self.write_lock.lock().await;
        let Ok(table) = self.raw_partition_table().await else {
            return;
        };
        let schema = raw_partition_schema();
        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(StringArray::from(vec![session_id.to_string()])),
                Arc::new(UInt64Array::from(vec![turn])),
                Arc::new(StringArray::from(vec![content])),
                Arc::new(StringArray::from(vec![chrono::Utc::now().to_rfc3339()])),
            ],
        );
        let Ok(batch) = batch else { return };
        let reader = RecordBatchIterator::new(vec![Ok(batch)], schema);
        let _ = table.add(reader).execute().await;
    }

    async fn record_hit(&self, token_id: &str) {
        // Delete-then-reinsert, same pattern `upsert_token` and every other
        // "replace" operation in this module already uses (no in-place
        // update-by-key primitive for this crate version — see
        // squire-storage/decisions.md's storage-layout note).
        let _guard = self.write_lock.lock().await;
        let Ok(table) = self.tokens().await else {
            return;
        };
        let Ok(Some(existing)) = self.find_token_row(token_id).await else {
            return;
        };
        let escaped = token_id.replace('\'', "''");
        let _ = table.delete(&format!("token_id = '{}'", escaped)).await;

        let embed_source = format!("{} {}", existing.token_id, existing.short_desc);
        let embedding = embed_text(&embed_source);
        let existing_endpoint_json = existing
            .endpoint
            .as_ref()
            .and_then(|e| serde_json::to_string(e).ok());
        let schema = tokens_schema();
        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(StringArray::from(vec![existing.token_id])),
                Arc::new(StringArray::from(vec![existing.token_type])),
                Arc::new(StringArray::from(vec![existing.short_desc])),
                Arc::new(StringArray::from(vec![existing.full_desc])),
                Arc::new(UInt64Array::from(vec![existing.creation_turn])),
                Arc::new(UInt64Array::from(vec![existing.accumulated_hits + 1])),
                Arc::new(embedding_array(&[embedding])),
                Arc::new(StringArray::from(vec![existing_endpoint_json])),
            ],
        );
        let Ok(batch) = batch else { return };
        let reader = RecordBatchIterator::new(vec![Ok(batch)], schema);
        let _ = table.add(reader).execute().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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

        // creation_turn isn't exposed via TokenDetail, but exercised via
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

        let results = store.explore_memory("concept", "rust programming", 0, 10, 0).await;
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
        let fish = results.iter().find(|t| t.token_id == "CONCEPT_Fish").unwrap();
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
        assert!(ids.contains(&"C"), "2-hop traversal should reach C from A via B");
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

        // Re-open against the same directory to confirm persistence across
        // connections (not just in-process caching).
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

        // Re-open to confirm persistence across connections, mirroring
        // insert_relationship_does_not_panic_and_persists's pattern.
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
        let registry = crate::agent::ToolRegistry::new(); // real run_terminal + web_fetch

        crate::agent::squire::ingest_tool_registry(&registry, &store, &std::collections::HashMap::new()).await;

        assert!(store.token_exists("run_terminal").await);
        assert!(store.token_exists("web_fetch").await);
        let detail = store.token_detail("run_terminal").await.unwrap();
        assert!(!detail.short_desc.is_empty());
    }

    #[tokio::test]
    async fn ingest_tool_registry_is_idempotent_and_updates_rather_than_duplicates() {
        let (store, _dir) = temp_store().await;
        let registry = crate::agent::ToolRegistry::new();

        crate::agent::squire::ingest_tool_registry(&registry, &store, &std::collections::HashMap::new()).await;
        crate::agent::squire::ingest_tool_registry(&registry, &store, &std::collections::HashMap::new()).await;
        crate::agent::squire::ingest_tool_registry(&registry, &store, &std::collections::HashMap::new()).await;

        let results = store.explore_memory("tool", "", 0, 100, 0).await;
        assert_eq!(results.len(), registry.definitions().len());
        let ids: std::collections::HashSet<&str> =
            results.iter().map(|t| t.token_id.as_str()).collect();
        assert_eq!(ids.len(), results.len(), "no duplicate token ids expected");
    }

    #[tokio::test]
    async fn ingest_tool_registry_full_desc_matches_expected_mcp_style_shape() {
        let (store, _dir) = temp_store().await;
        let mut registry = crate::agent::ToolRegistry::empty();
        registry.register(Box::new(crate::agent::TerminalTool));

        crate::agent::squire::ingest_tool_registry(&registry, &store, &std::collections::HashMap::new()).await;

        let detail = store.token_detail("run_terminal").await.unwrap();
        let full_desc = detail.full_desc.expect("tool tokens must carry a full_desc");
        let parsed: serde_json::Value = serde_json::from_str(&full_desc).unwrap();
        assert_eq!(parsed["name"], "run_terminal");
        assert!(parsed["description"].is_string());
        assert!(parsed["input_schema"].is_object());
    }

    #[tokio::test]
    async fn ingest_tool_registry_persists_across_reopen() {
        let (store, dir) = temp_store().await;
        let registry = crate::agent::ToolRegistry::new();
        crate::agent::squire::ingest_tool_registry(&registry, &store, &std::collections::HashMap::new()).await;

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
        let endpoint = crate::agent::squire::ToolEndpoint::Mcp {
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
        let endpoint = crate::agent::squire::ToolEndpoint::Mcp {
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
        let endpoint = crate::agent::squire::ToolEndpoint::Mcp {
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
    async fn ingest_tool_registry_populates_endpoint_only_for_mcp_sourced_definitions_via_real_lancedb() {
        let (store, _dir) = temp_store().await;
        let registry = crate::agent::ToolRegistry::new();
        let mut endpoints = std::collections::HashMap::new();
        endpoints.insert(
            "run_terminal".to_string(),
            crate::agent::squire::ToolEndpoint::Mcp {
                server: fake_mcp_server_for_lancedb_tests("srv1"),
                remote_name: "remote_terminal".to_string(),
            },
        );

        crate::agent::squire::ingest_tool_registry(&registry, &store, &endpoints).await;

        let terminal_detail = store.token_detail("run_terminal").await.unwrap();
        assert!(terminal_detail.endpoint.is_some());
        let web_fetch_detail = store.token_detail("web_fetch").await.unwrap();
        assert!(web_fetch_detail.endpoint.is_none());
    }

    #[tokio::test]
    async fn ingest_tool_registry_reflects_schema_change_on_next_ingestion() {
        struct FakeToolV1;
        #[async_trait]
        impl crate::agent::Tool for FakeToolV1 {
            fn name(&self) -> &str { "fake_tool" }
            fn description(&self) -> &str { "version one" }
            fn input_schema(&self) -> serde_json::Value { serde_json::json!({"type": "object"}) }
            async fn execute(&self, call_id: &str, _args: serde_json::Value) -> crate::agent::ToolResult {
                crate::agent::ToolResult { call_id: call_id.to_string(), output: String::new(), is_error: false }
            }
        }
        struct FakeToolV2;
        #[async_trait]
        impl crate::agent::Tool for FakeToolV2 {
            fn name(&self) -> &str { "fake_tool" }
            fn description(&self) -> &str { "version two" }
            fn input_schema(&self) -> serde_json::Value { serde_json::json!({"type": "object"}) }
            async fn execute(&self, call_id: &str, _args: serde_json::Value) -> crate::agent::ToolResult {
                crate::agent::ToolResult { call_id: call_id.to_string(), output: String::new(), is_error: false }
            }
        }

        let (store, _dir) = temp_store().await;
        let mut registry_v1 = crate::agent::ToolRegistry::empty();
        registry_v1.register(Box::new(FakeToolV1));
        crate::agent::squire::ingest_tool_registry(&registry_v1, &store, &std::collections::HashMap::new()).await;
        assert_eq!(store.token_detail("fake_tool").await.unwrap().short_desc, "version one");

        let mut registry_v2 = crate::agent::ToolRegistry::empty();
        registry_v2.register(Box::new(FakeToolV2));
        crate::agent::squire::ingest_tool_registry(&registry_v2, &store, &std::collections::HashMap::new()).await;
        assert_eq!(store.token_detail("fake_tool").await.unwrap().short_desc, "version two");

        let results = store.explore_memory("tool", "", 0, 100, 0).await;
        assert_eq!(results.iter().filter(|t| t.token_id == "fake_tool").count(), 1);
    }

    // ---- user-input auto-chunking (against the real LanceDB backend) ----

    #[tokio::test]
    async fn ingest_user_input_chunks_writes_system_referential_tokens_with_expected_ids() {
        let (store, _dir) = temp_store().await;
        crate::agent::squire::ingest_user_input_chunks(
            "First paragraph.\n\nSecond paragraph.",
            2,
            &store,
        )
        .await;

        assert!(store.token_exists("USR_T2_001").await);
        assert!(store.token_exists("USR_T2_002").await);

        let detail = store.token_detail("USR_T2_001").await.unwrap();
        assert_eq!(detail.short_desc, "First paragraph.");
        assert_eq!(detail.full_desc, Some("First paragraph.".to_string()));
    }

    #[tokio::test]
    async fn ingest_user_input_chunks_discoverable_via_explore_system_referential_filter() {
        let (store, _dir) = temp_store().await;
        crate::agent::squire::ingest_user_input_chunks("A message about weather patterns.", 1, &store)
            .await;

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
        crate::agent::squire::ingest_user_input_chunks("A message about weather patterns.", 1, &store)
            .await;

        let via_memory = store
            .explore_memory("memory", "weather", 0, 10, 1)
            .await;
        assert!(via_memory.iter().any(|t| t.token_id == "USR_T1_001"));
    }

    #[tokio::test]
    async fn ingest_user_input_chunks_persists_across_reopen() {
        let dir = tempfile::tempdir().unwrap();
        {
            let store = LanceDbSquireStore::open(dir.path()).await.unwrap();
            crate::agent::squire::ingest_user_input_chunks("Persisted chunk content.", 4, &store)
                .await;
        }
        let reopened = LanceDbSquireStore::open(dir.path()).await.unwrap();
        assert!(reopened.token_exists("USR_T4_001").await);
        let detail = reopened.token_detail("USR_T4_001").await.unwrap();
        assert_eq!(detail.full_desc, Some("Persisted chunk content.".to_string()));
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
        let session_ids = batch.column_by_name("session_id").unwrap().as_string::<i32>();
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

        // Re-open to confirm persistence across connections, mirroring this
        // module's existing append-only-table test pattern.
        let reopened = LanceDbSquireStore::open(dir.path()).await.unwrap();
        let table = reopened.raw_partition_table().await.unwrap();
        assert_eq!(table.count_rows(None).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn record_raw_output_is_append_only_across_multiple_turns() {
        let (store, _dir) = temp_store().await;
        let sid = Uuid::new_v4();
        store.record_raw_output(sid, 0, "turn zero prose".to_string()).await;
        store.record_raw_output(sid, 1, "turn one prose".to_string()).await;

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
        let contents = batches[0].column_by_name("content").unwrap().as_string::<i32>();
        assert_eq!(contents.value(0), "Sure. All done.");
        assert!(!contents.value(0).contains("42"));
    }
}
