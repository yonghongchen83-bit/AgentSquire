//! `squire-store` — LanceDB-backed persistent storage for the Squire
//! context-mode protocol.
//!
//! This crate isolates the heaviest native dependencies (lancedb → arrow,
//! fastembed → ONNX Runtime) so that the rest of the application compiles
//! faster — changes to application logic don't re-trigger compilation of
//! these heavy third-party crates.
//!
//! # Crate structure
//!
//! - [`types`] — shared data types (`TokenSummary`, `NewTokenSpec`, etc.)
//! - [`store`] — the `SquireStore` trait and backend-agnostic helpers
//! - [`lancedb`] — `LanceDbSquireStore`, the production implementation
//! - [`embedding`] — text embeddings via fastembed (ONNX model)
//! - [`trace`] — JSONL debug tracing for the retrieval loop

pub mod embedding;
pub mod lancedb;
pub mod store;
pub mod trace;
pub mod types;

// Re-exports for convenience
pub use lancedb::LanceDbSquireStore;
pub use store::{effective_priority, sort_by_score_then_priority, SquireStore, StoredToken, TraversalNode, traverse_relationships};
pub use types::{
    ActiveProcessState, ComplianceFailureRecord, LastDecision, McpServerConfig, NewTokenSpec,
    RawPartitionRecord, Relationship, SessionId, TokenDetail, TokenRange, TokenSummary,
    ToolEndpoint,
};
pub use types::predicates;
