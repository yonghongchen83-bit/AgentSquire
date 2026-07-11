//! Squire context mode — submodule root.
//!
//! This module is split into focused sub-modules:
//! - [`types`] — protocol data types and re-exports from `squire-store`
//! - [`store`] — `InMemorySquireStore` test double (the trait is in `squire-store`)
//! - [`protocol`] — sigil parsing (§! inline refs, §^ span markers) and turn-close validation
//! - [`ingestion`] — tool-token and user-input-chunk ingestion
//! - [`tools`] — built-in Squire protocol tools (`explore`, `token_to_detail`)
//! - [`adapter`] — `SquireContextAdapter` + `ContextManagerAdapter` impl

pub mod adapter;
pub mod ingestion;
pub mod protocol;
pub mod store;
pub mod tools;
pub mod types;

// Re-exports ──────────────────────────────────────────────────────────
// Protocol types (SquireResponse stays local; storage types re-exported
// via types.rs from squire-store)
pub use types::{
    ComplianceFailureRecord, FormatterOutput, NewTokenSpec, RawPartitionRecord, Relationship,
    SquireResponse, TokenDetail, TokenSummary, ToolEndpoint, parse_formatter_json,
};

// Store — only InMemorySquireStore is local; trait & helpers are in squire-store
pub use store::InMemorySquireStore;
pub use squire_store::{
    effective_priority, sort_by_score_then_priority, SquireStore, TraversalNode,
    traverse_relationships,
};

// Protocol
pub use protocol::{extract_inline_refs, extract_spans, validate_squire_response};
#[cfg(test)]
pub(crate) use protocol::{strip_span_markers, unmarked_residual};
#[cfg(test)]
pub(crate) use ingestion::{CHUNK_SOFT_LIMIT_CHARS, first_sentence};

// Ingestion
pub use ingestion::{chunk_user_input, ingest_tool_registry, ingest_user_input_chunks, tool_token_id};

// Tools
pub use tools::{built_in_tool_definitions, parse_batch_expr, SquireBatchTool, SquireExploreTool, SquireInvokeTool, SquireRdfTool, SquireTokenToDetailTool};

// Adapter
pub use adapter::SquireContextAdapter;
#[cfg(test)]
pub(crate) use adapter::classify_rejection_rule;

// Context adapter types needed by test code
pub use crate::agent::context_adapter::TurnOutcome;

// Test declaration — the old squire_test.rs uses `use super::*;` which
// pulls in everything above, so no import changes needed there.
#[cfg(test)]
#[path = "squire_test.rs"]
mod tests;
