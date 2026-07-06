//! Core data types for the Squire context-mode protocol.
//!
//! Storage-level types (`TokenSummary`, `NewTokenSpec`, etc.) are now
//! defined in the `squire-store` crate. This module only keeps
//! protocol-level types that are not part of the storage contract.

use serde::Deserialize;

// Re-export storage types from squire-store for convenience.
pub use squire_store::{
    ComplianceFailureRecord, McpServerConfig, NewTokenSpec, RawPartitionRecord, Relationship,
    SessionId, TokenDetail, TokenSummary, ToolEndpoint,
};

// ─────────────────────────── Protocol types ───────────────────────────

#[derive(Debug, Clone, Deserialize, Default, PartialEq)]
#[serde(default)]
pub struct SquireResponse {
    pub ask_user: String,
    pub content: String,
    pub preserve: Vec<String>,
    pub new_tokens: Vec<NewTokenSpec>,
    pub relationships: Vec<Relationship>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ComplianceFailure {
    pub reason: String,
}
