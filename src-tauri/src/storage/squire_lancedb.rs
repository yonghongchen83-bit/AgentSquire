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
//! Embedding note: vector search here is powered by a real local text
//! embedding model (fastembed `BGESmallENV15`, 384-dim) via
//! `crate::storage::embedding::embed_text`, with a deterministic
//! bag-of-words hash fallback if the model can't initialize (offline first
//! run / download failure). The table schema is a fixed `EMBED_DIM`-dim
//! float32 vector; if an on-disk tokens table was created with a different
//! embedding width (e.g. the historical 64-dim toy embedding), `open()`
//! drops and recreates it (data loss is acceptable — the store is
//! per-conversation / dev-scoped).

use std::sync::Arc;

use arrow_array::{
    cast::AsArray, Array, Float32Array, RecordBatch, RecordBatchIterator, StringArray, UInt64Array,
};
use arrow_schema::{DataType, Field, Schema as ArrowSchema};
use async_trait::async_trait;
use futures::TryStreamExt;
use lancedb::query::{ExecutableQuery, QueryBase};
use lancedb::{Connection, Table};
use tokio::sync::Mutex;

use crate::agent::squire::{
    ComplianceFailureRecord, NewTokenSpec, Relationship, SquireStore, TokenDetail, TokenSummary,
    ToolEndpoint,
};
use crate::storage::conversation_store::SessionId;
use crate::storage::embedding::{embed_text, EMBED_DIM};

const TOKENS_TABLE: &str = "squire_tokens";
const RELATIONSHIPS_TABLE: &str = "squire_relationships";
const TURNS_TABLE: &str = "squire_turns";
const COMPLIANCE_FAILURES_TABLE: &str = "squire_compliance_failures";
const RAW_PARTITION_TABLE: &str = "squire_raw_partition";

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

        let existing = conn
            .table_names()
            .execute()
            .await
            .map_err(|e| e.to_string())?;

        if existing.iter().any(|n| n == TOKENS_TABLE) {
            // Dimension migration: a tokens table created under an older
            // embedding width (e.g. the historical 64-dim toy embedding) is
            // incompatible with the current `EMBED_DIM`-dim schema — writes
            // and vector reads would fail. Detect a mismatch and drop+recreate
            // the table (data loss accepted: this store is per-conversation /
            // dev-scoped, and rows self-heal on the next ingestion).
            if Self::tokens_embedding_dim(&conn).await != Some(EMBED_DIM) {
                log::warn!(
                    "Squire storage: existing '{TOKENS_TABLE}' table embedding dimension \
                     does not match EMBED_DIM ({EMBED_DIM}); dropping and recreating it \
                     (token rows will repopulate on next ingestion)"
                );
                conn.drop_table(TOKENS_TABLE)
                    .await
                    .map_err(|e| e.to_string())?;
                conn.create_empty_table(TOKENS_TABLE, tokens_schema())
                    .execute()
                    .await
                    .map_err(|e| e.to_string())?;
            }
        } else {
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

    /// Reads the fixed-size-list width of the existing tokens table's
    /// `embedding` column, or `None` if the table/column can't be read (in
    /// which case `open()` treats it as a mismatch and recreates the table).
    async fn tokens_embedding_dim(conn: &Connection) -> Option<usize> {
        let table = conn.open_table(TOKENS_TABLE).execute().await.ok()?;
        let schema = table.schema().await.ok()?;
        let field = schema.field_with_name("embedding").ok()?;
        match field.data_type() {
            DataType::FixedSizeList(_, dim) => Some(*dim as usize),
            _ => None,
        }
    }

    async fn tokens(&self) -> Result<Table, String> {
        self.conn
            .open_table(TOKENS_TABLE)
            .execute()
            .await
            .map_err(|e| e.to_string())
    }

    pub(crate) async fn relationships_table(&self) -> Result<Table, String> {
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

    pub(crate) async fn compliance_failures_table(&self) -> Result<Table, String> {
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
            let Some(subjects) = batch.column_by_name("subject") else {
                continue;
            };
            let Some(objects) = batch.column_by_name("object") else {
                continue;
            };
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
        let types = batch
            .column_by_name("token_type")
            .unwrap()
            .as_string::<i32>();
        let shorts = batch
            .column_by_name("short_desc")
            .unwrap()
            .as_string::<i32>();
        let fulls = batch
            .column_by_name("full_desc")
            .unwrap()
            .as_string::<i32>();
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
                _ => (
                    creation_turn,
                    token.full_desc.clone(),
                    1u64,
                    token.endpoint.clone(),
                ),
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
        let _ = table.delete(&format!("session_id = '{}'", escaped)).await;

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
        // obs-2: capture the per-candidate scoring breakdown for tracing.
        // `included_detail` holds cosine/substr_boost keyed by token_id for
        // candidates that survived the score<=0 cut (direct matches only —
        // traversal-discovered tokens have no cosine of their own).
        // `near_misses` holds the candidates dropped by the score<=0 filter,
        // which are invisible in the returned Vec. This is pure observation
        // and does not alter scoring/filtering below.
        let mut included_detail: std::collections::HashMap<String, (f32, f32)> =
            std::collections::HashMap::new();
        let mut near_misses: Vec<serde_json::Value> = Vec::new();
        for batch in &batches {
            let Some(ids_col) = batch.column_by_name("token_id") else {
                continue;
            };
            let Some(types_col) = batch.column_by_name("token_type") else {
                continue;
            };
            let Some(shorts_col) = batch.column_by_name("short_desc") else {
                continue;
            };
            let Some(embed_col) = batch.column_by_name("embedding") else {
                continue;
            };
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

                // Score components captured separately for tracing (obs-2);
                // `sim`/`substr_boost` are None for the empty-query path where
                // score is fixed at 1.0 with no semantic/lexical component.
                let mut sim_component: Option<f32> = None;
                let mut boost_component: Option<f32> = None;
                let score = match (&query_embedding, embeddings) {
                    (Some(qe), Some(embeds)) => {
                        let row_val = embeds.value(i);
                        let row_arr = row_val.as_any().downcast_ref::<Float32Array>().unwrap();
                        let row_vec: Vec<f32> = row_arr.values().to_vec();
                        let sim = cosine_similarity(qe, &row_vec);
                        // Substring match boost so exact-name hits still
                        // surface even when semantic similarity alone ranks
                        // them low (a query that lexically names a token but
                        // isn't semantically close to its short_desc).
                        let substr_boost =
                            if token_id.to_lowercase().contains(&query.to_lowercase())
                                || short_desc.to_lowercase().contains(&query.to_lowercase())
                            {
                                0.5
                            } else {
                                0.0
                            };
                        sim_component = Some(sim);
                        boost_component = Some(substr_boost);
                        sim + substr_boost
                    }
                    _ => 1.0,
                };

                if query_embedding.is_some() && score <= 0.0 {
                    // obs-2: record the dropped candidate (a near-miss). Capped
                    // later at emit time to keep JSONL lines a sane size.
                    near_misses.push(serde_json::json!({
                        "token_id": token_id,
                        "token_type": token_type,
                        "cosine": sim_component,
                        "substr_boost": boost_component,
                        "score": score,
                        "included": false,
                    }));
                    continue;
                }

                if let (Some(s), Some(b)) = (sim_component, boost_component) {
                    included_detail.insert(token_id.to_string(), (s, b));
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
            let all_nodes: std::collections::HashMap<String, crate::agent::squire::TraversalNode> =
                all_rows
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
            let direct_scores: Vec<(String, f32)> = scored
                .iter()
                .map(|t| (t.token_id.clone(), t.score))
                .collect();
            let mut expanded = crate::agent::squire::traverse_relationships(
                &direct_scores,
                &edges,
                num_hops,
                &all_nodes,
                type_matches,
            );
            for t in &mut expanded {
                t.accumulated_hits = all_rows
                    .get(&t.token_id)
                    .map(|r| r.accumulated_hits)
                    .unwrap_or(0);
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

        let keep = max_results.max(1) as usize;
        // obs-2: candidates that scored/traversed in but got cut by the top-N
        // truncation are also near-misses. Record them (with their score
        // breakdown when they were direct matches) before truncating.
        if scored.len() > keep {
            for t in &scored[keep..] {
                let (cosine, boost) = included_detail
                    .get(&t.token_id)
                    .map(|(s, b)| (Some(*s), Some(*b)))
                    .unwrap_or((None, None));
                near_misses.push(serde_json::json!({
                    "token_id": t.token_id,
                    "token_type": t.token_type,
                    "cosine": cosine,
                    "substr_boost": boost,
                    "score": t.score,
                    "hop_distance": t.hop_distance,
                    "via_token_id": t.via_token_id,
                    "included": false,
                }));
            }
        }

        scored.truncate(keep);

        // obs-2/obs-3: emit the RETRIEVAL trace for the store branch. This is
        // pure observation — `scored` (the return value) is unchanged. The
        // tool_call_id is not plumbed into the SquireStore trait (correlation
        // is by `turn` at this level); the substring branch in
        // SquireExploreTool traces its own call_id.
        if crate::storage::squire_trace::trace_enabled() {
            let results: Vec<serde_json::Value> = scored
                .iter()
                .map(|t| {
                    let (cosine, boost) = included_detail
                        .get(&t.token_id)
                        .map(|(s, b)| (Some(*s), Some(*b)))
                        .unwrap_or((None, None));
                    serde_json::json!({
                        "token_id": t.token_id,
                        "token_type": t.token_type,
                        "cosine": cosine,
                        "substr_boost": boost,
                        "score": t.score,
                        "hop_distance": t.hop_distance,
                        "via_token_id": t.via_token_id,
                        "included": true,
                    })
                })
                .collect();
            near_misses.truncate(20);
            let payload = serde_json::json!({
                "branch": "store_semantic",
                "resource_type": resource_type,
                "query": query,
                "num_hops": num_hops,
                "max_results": max_results,
                "embedding_backend": crate::storage::embedding::active_backend(),
                "results": results,
                "near_misses": near_misses,
            });
            crate::storage::squire_trace::trace_explore(current_turn, None, payload);
        }

        scored
    }

    async fn token_detail(&self, token_id: &str) -> Option<TokenDetail> {
        self.find_token_row(token_id)
            .await
            .ok()
            .flatten()
            .map(|row| TokenDetail {
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
        let _ = table.delete(&format!("session_id = '{}'", escaped)).await;

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
#[path = "squire_lancedb_test.rs"]
mod tests;
