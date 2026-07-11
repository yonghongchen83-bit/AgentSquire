//! Real, LanceDB-backed implementation of `SquireStore`.
//!
//! One LanceDB directory holds all tables (tokens/relationships/turns/
//! preserve-lists/compliance-failures/raw-partition) — LanceDB has no
//! notion of a single "database" file the way SQLite does, so the directory
//! itself is the unit of storage.

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

use crate::embedding::{embed_text, EMBED_DIM};
use crate::store::{
    effective_priority, sort_by_score_then_priority, SquireStore, TraversalNode,
    traverse_relationships,
};
use crate::trace;
use crate::types::predicates;
use crate::types::{
    ComplianceFailureRecord, NewTokenSpec, Relationship, TokenDetail, TokenRange, TokenSummary,
    ToolEndpoint,
};
use crate::types::SessionId;

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
        Field::new("accumulated_hits", DataType::UInt64, false),
        Field::new(
            "embedding",
            DataType::FixedSizeList(
                Arc::new(Field::new("item", DataType::Float32, true)),
                EMBED_DIM as i32,
            ),
            false,
        ),
        Field::new("endpoint", DataType::Utf8, true),
        Field::new("ranges", DataType::Utf8, true),
        Field::new("session_id", DataType::Utf8, true),
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

/// LanceDB-backed `SquireStore`. One LanceDB directory holds all tables.
pub struct LanceDbSquireStore {
    conn: Connection,
    write_lock: Mutex<()>,
}

impl LanceDbSquireStore {
    /// Opens (creating if necessary) a LanceDB store at `dir`. All tables
    /// are created empty on first use if they don't already exist.
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
            } else if Self::tokens_has_column(&conn, "session_id").await != Some(true) {
                log::warn!(
                    "Squire storage: existing '{TOKENS_TABLE}' table missing 'session_id' \
                     column; dropping and recreating it \
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

    async fn tokens_embedding_dim(conn: &Connection) -> Option<usize> {
        let table = conn.open_table(TOKENS_TABLE).execute().await.ok()?;
        let schema = table.schema().await.ok()?;
        let field = schema.field_with_name("embedding").ok()?;
        match field.data_type() {
            DataType::FixedSizeList(_, dim) => Some(*dim as usize),
            _ => None,
        }
    }

    async fn tokens_has_column(conn: &Connection, column: &str) -> Option<bool> {
        let table = conn.open_table(TOKENS_TABLE).execute().await.ok()?;
        let schema = table.schema().await.ok()?;
        Some(schema.field_with_name(column).is_ok())
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
    ranges: Vec<TokenRange>,
    session_id: String,
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
        let endpoints = batch
            .column_by_name("endpoint")
            .map(|c| c.as_string::<i32>().clone());
        let session_ids = batch
            .column_by_name("session_id")
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
                ranges: batch
                    .column_by_name("ranges")
                    .map(|c| c.as_string::<i32>())
                    .and_then(|r| {
                        if r.is_null(i) {
                            None
                        } else {
                            serde_json::from_str::<Vec<TokenRange>>(r.value(i)).ok()
                        }
                    })
                    .unwrap_or_default(),
                session_id: session_ids
                    .as_ref()
                    .and_then(|s| {
                        if s.is_null(i) { None } else { Some(s.value(i).to_string()) }
                    })
                    .unwrap_or_default(),
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

    async fn upsert_token(&self, token: NewTokenSpec, creation_turn: u64, session_id: SessionId) {
        let _guard = self.write_lock.lock().await;
        let Ok(table) = self.tokens().await else {
            return;
        };

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

        let sid = session_id.to_string();

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
                Arc::new(StringArray::from(vec![serde_json::to_string(&token.ranges).unwrap_or_default()])),
                Arc::new(StringArray::from(vec![sid])),
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
        session_id: SessionId,
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

        // Build role index from relationships — when resource_type is a role
        // name (tool/workflow/skill), check relationships since the token_type
        // is now always "source" for role-bearing tokens (spec §2: roles are
        // graph-assigned, not hardcoded types).
        let role_token_ids: Option<std::collections::HashSet<String>> =
            if matches!(resource_type, "tool" | "skill" | "workflow" | "tool_skill") {
                async {
                    let rt = self.relationships_table().await.ok()?;
                    let rs = rt.query().execute().await.ok()?;
                    let rb = rs.try_collect::<Vec<_>>().await.ok()?;
                    let mut ids = std::collections::HashSet::new();
                    for batch in &rb {
                        let preds = batch.column_by_name("predicate")?;
                        let subs = batch.column_by_name("subject")?;
                        let preds = preds.as_string::<i32>();
                        let subs = subs.as_string::<i32>();
                        for i in 0..batch.num_rows() {
                            let pred = preds.value(i);
                            let sub = subs.value(i);
                            match (resource_type, pred) {
                                ("tool", p) if p == "IS_A_TOOL" => { ids.insert(sub.to_string()); }
                                ("skill", p) if p == "IS_A_SKILL" => { ids.insert(sub.to_string()); }
                                ("workflow", p) if p == "IS_A_WORKFLOW" => { ids.insert(sub.to_string()); }
                                ("tool_skill", p) if p == "IS_A_TOOL" || p == "IS_A_SKILL" => { ids.insert(sub.to_string()); }
                                _ => {}
                            }
                        }
                    }
                    Some(ids)
                }.await
            } else {
                None
            };

        let type_matches = |t: &str| {
            resource_type == "all"
                || t == resource_type
                || (resource_type == "memory"
                    && (t == "concept" || t == "referential" || t == "source"))
                || (resource_type == "tool_skill" && t == "skill")
        };

        // Session filter: include tokens whose session_id is the current
        // session OR nil (global). Tokens from OTHER sessions are excluded
        // from explore results (conversation-isolation).
        let sid = session_id.to_string();
        let nil = uuid::Uuid::nil().to_string();
        let session_matches = |row_sid: &str| row_sid.is_empty() || row_sid == sid || row_sid == nil;

        let query_embedding = if query.is_empty() {
            None
        } else {
            Some(embed_text(query))
        };

        let mut all_rows: std::collections::HashMap<String, StoredTokenRow> =
            std::collections::HashMap::new();
        let mut scored: Vec<TokenSummary> = Vec::new();
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

            // Read session_id column (nullable — pre-migration stores won't have it)
            let sid_col = batch.column_by_name("session_id").map(|c| c.as_string::<i32>());

            for i in 0..batch.num_rows() {
                let token_type = types.value(i);
                let token_id = ids.value(i);
                let short_desc = shorts.value(i);
                let accumulated_hits = hits_col.map(|a| a.value(i)).unwrap_or(0);
                let creation_turn = turns_col.map(|a| a.value(i)).unwrap_or(0);

                // Read row-level session_id, defaulting to empty (global)
                let row_sid = sid_col
                    .as_ref()
                    .and_then(|c| if c.is_null(i) { None } else { Some(c.value(i)) })
                    .unwrap_or("");

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
                        ranges: vec![],
                        session_id: row_sid.to_string(),
                    },
                );

                // Apply session-scope filter: skip tokens from other sessions
                if !session_matches(row_sid) {
                    continue;
                }

                let by_type = type_matches(token_type);
                let by_role = role_token_ids
                    .as_ref()
                    .map_or(false, |ids| ids.contains(token_id));
                if !by_type && !by_role {
                    continue;
                }

                let mut sim_component: Option<f32> = None;
                let mut boost_component: Option<f32> = None;
                let score = match (&query_embedding, embeddings) {
                    (Some(qe), Some(embeds)) => {
                        let row_val = embeds.value(i);
                        let row_arr = row_val.as_any().downcast_ref::<Float32Array>().unwrap();
                        let row_vec: Vec<f32> = row_arr.values().to_vec();
                        let sim = cosine_similarity(qe, &row_vec);
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

        if num_hops > 0 && !scored.is_empty() {
            let all_nodes: std::collections::HashMap<String, TraversalNode> =
                all_rows
                    .values()
                    .map(|row| {
                        (
                            row.token_id.clone(),
                            TraversalNode {
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
            let mut expanded = traverse_relationships(
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
                        effective_priority(
                            row.accumulated_hits,
                            current_turn,
                            row.creation_turn,
                        ),
                    )
                })
            })
            .collect();
        sort_by_score_then_priority(&mut scored, &priorities);

        let keep = max_results.max(1) as usize;
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

        if trace::trace_enabled() {
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
                "embedding_backend": crate::embedding::active_backend(),
                "results": results,
                "near_misses": near_misses,
            });
            trace::trace_explore(current_turn, None, payload);
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
                ranges: row.ranges,
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
        let sid = if existing.session_id.is_empty() {
            None
        } else {
            Some(existing.session_id.as_str())
        };

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
                Arc::new(StringArray::from(vec![serde_json::to_string(&existing.ranges).unwrap_or_default()])),
                Arc::new(StringArray::from(vec![sid])),
            ],
        );
        let Ok(batch) = batch else { return };
        let reader = RecordBatchIterator::new(vec![Ok(batch)], schema);
        let _ = table.add(reader).execute().await;
    }

    async fn get_relationships(
        &self,
        subject: Option<&str>,
        predicate: Option<&str>,
        object: Option<&str>,
    ) -> Vec<Relationship> {
        let Ok(table) = self.relationships_table().await else {
            return Vec::new();
        };
        let Ok(stream) = table.query().execute().await else {
            return Vec::new();
        };
        let Ok(batches) = stream.try_collect::<Vec<_>>().await else {
            return Vec::new();
        };
        let mut results = Vec::new();
        for batch in &batches {
            let Some(s_col) = batch.column_by_name("subject") else {
                continue;
            };
            let Some(p_col) = batch.column_by_name("predicate") else {
                continue;
            };
            let Some(o_col) = batch.column_by_name("object") else {
                continue;
            };
            let s_arr = s_col.as_string::<i32>();
            let p_arr = p_col.as_string::<i32>();
            let o_arr = o_col.as_string::<i32>();
            for i in 0..batch.num_rows() {
                let s = s_arr.value(i);
                let p = p_arr.value(i);
                let o = o_arr.value(i);
                if let Some(ref sub) = subject {
                    if s != *sub {
                        continue;
                    }
                }
                if let Some(ref pre) = predicate {
                    if p != *pre {
                        continue;
                    }
                }
                if let Some(ref obj) = object {
                    if o != *obj {
                        continue;
                    }
                }
                results.push(Relationship {
                    subject: s.to_string(),
                    predicate: p.to_string(),
                    object: o.to_string(),
                });
            }
        }
        results
    }

    async fn list_token_ids(&self) -> Vec<String> {
        let Ok(table) = self.tokens().await else {
            return Vec::new();
        };
        let Ok(stream) = table.query().execute().await else {
            return Vec::new();
        };
        let Ok(batches) = stream.try_collect::<Vec<_>>().await else {
            return Vec::new();
        };
        let mut ids = Vec::new();
        for batch in &batches {
            let Some(id_col) = batch.column_by_name("token_id") else {
                continue;
            };
            let arr = id_col.as_string::<i32>();
            for i in 0..batch.num_rows() {
                ids.push(arr.value(i).to_string());
            }
        }
        ids
    }

    async fn list_token_ids_by_session(&self, session_id: SessionId) -> Vec<String> {
        let Ok(table) = self.tokens().await else {
            return Vec::new();
        };
        let sid = session_id.to_string();
        let nil = uuid::Uuid::nil().to_string();
        let escaped_sid = sid.replace('\'', "''");
        let escaped_nil = nil.replace('\'', "''");
        // Return tokens matching current session OR global (nil) session.
        // Empty-string session_id handles pre-migration rows.
        let Ok(stream) = table
            .query()
            .only_if(format!(
                "session_id IS NULL OR session_id = '' OR session_id = '{}' OR session_id = '{}'",
                escaped_sid, escaped_nil
            ))
            .execute()
            .await
        else {
            return Vec::new();
        };
        let Ok(batches) = stream.try_collect::<Vec<_>>().await else {
            return Vec::new();
        };
        let mut ids = Vec::new();
        for batch in &batches {
            let Some(id_col) = batch.column_by_name("token_id") else {
                continue;
            };
            let arr = id_col.as_string::<i32>();
            for i in 0..batch.num_rows() {
                ids.push(arr.value(i).to_string());
            }
        }
        ids
    }

    // ── Tree API convenience methods ──

    async fn add_relationship(&self, rel: Relationship) {
        // Auto-mirror HasParent ↔ Contains
        if rel.predicate == predicates::CONTAINS {
            // Contains edges must not be inserted directly — use HasParent
            return;
        }
        self.insert_relationship(rel.clone()).await;
        if rel.predicate == predicates::HAS_PARENT {
            self.insert_relationship(Relationship {
                subject: rel.object.clone(),
                predicate: predicates::CONTAINS.to_string(),
                object: rel.subject.clone(),
            })
            .await;
        }
    }

    async fn get_children(&self, token_id: &str) -> Vec<TokenSummary> {
        let rels = self
            .get_relationships(Some(token_id), Some(predicates::HAS_PARENT), None)
            .await;
        let mut children = Vec::with_capacity(rels.len());
        for rel in rels {
            if let Some(row) = self.find_token_row(&rel.object).await.unwrap_or(None) {
                children.push(TokenSummary {
                    token_id: row.token_id,
                    token_type: row.token_type,
                    score: 0.0,
                    short_desc: row.short_desc,
                    accumulated_hits: row.accumulated_hits,
                    hop_distance: 0,
                    via_token_id: None,
                });
            }
        }
        children
    }

    async fn get_parent(&self, token_id: &str) -> Option<TokenSummary> {
        let rels = self
            .get_relationships(None, Some(predicates::HAS_PARENT), Some(token_id))
            .await;
        let parent_id = rels.first()?.subject.clone();
        let row = self.find_token_row(&parent_id).await.unwrap_or(None)?;
        Some(TokenSummary {
            token_id: row.token_id,
            token_type: row.token_type,
            score: 0.0,
            short_desc: row.short_desc,
            accumulated_hits: row.accumulated_hits,
            hop_distance: 0,
            via_token_id: None,
        })
    }

    async fn get_ancestors(&self, token_id: &str, max_depth: u32) -> Vec<TokenSummary> {
        let mut ancestors = Vec::new();
        let mut current = token_id.to_string();
        for _ in 0..max_depth {
            let rels = self
                .get_relationships(None, Some(predicates::HAS_PARENT), Some(&current))
                .await;
            let Some(parent_rel) = rels.first() else {
                break;
            };
            current = parent_rel.subject.clone();
            if let Some(row) = self.find_token_row(&current).await.unwrap_or(None) {
                ancestors.push(TokenSummary {
                    token_id: row.token_id,
                    token_type: row.token_type,
                    score: 0.0,
                    short_desc: row.short_desc,
                    accumulated_hits: row.accumulated_hits,
                    hop_distance: 0,
                    via_token_id: None,
                });
            }
        }
        ancestors
    }

    async fn get_type(&self, token_id: &str) -> Option<String> {
        let row = self.find_token_row(token_id).await.unwrap_or(None)?;
        Some(row.token_type)
    }

    async fn get_instances(&self, token_type: &str) -> Vec<TokenSummary> {
        let all_ids = self.list_token_ids().await;
        let mut results = Vec::new();
        for id in &all_ids {
            if let Some(row) = self.find_token_row(id).await.unwrap_or(None) {
                if row.token_type == token_type {
                    results.push(TokenSummary {
                        token_id: row.token_id,
                        token_type: row.token_type,
                        score: 0.0,
                        short_desc: row.short_desc,
                        accumulated_hits: row.accumulated_hits,
                        hop_distance: 0,
                        via_token_id: None,
                    });
                }
            }
        }
        results
    }

    async fn get_root(&self, token_id: &str) -> Option<TokenSummary> {
        let mut current = token_id.to_string();
        loop {
            let rels = self
                .get_relationships(None, Some(predicates::HAS_PARENT), Some(&current))
                .await;
            let Some(parent_rel) = rels.first() else {
                // No parent — current is the root
                let row = self.find_token_row(&current).await.unwrap_or(None)?;
                return Some(TokenSummary {
                    token_id: row.token_id,
                    token_type: row.token_type,
                    score: 0.0,
                    short_desc: row.short_desc,
                    accumulated_hits: row.accumulated_hits,
                    hop_distance: 0,
                    via_token_id: None,
                });
            };
            current = parent_rel.subject.clone();
        }
    }
}
