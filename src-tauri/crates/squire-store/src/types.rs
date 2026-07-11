//! Core data types for the Squire context-mode protocol storage layer.
//!
//! These types are shared between the `SquireStore` trait (defined in
//! [`store`]), the `LanceDbSquireStore` implementation, and the main
//! crate's adapter/protocol/ingestion layers.

use serde::{Deserialize, Serialize};

/// Session identifier — a UUID v4.
pub type SessionId = uuid::Uuid;

// ─────────────────────────── Storage contract types ───────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenSummary {
    pub token_id: String,
    #[serde(rename = "type")]
    pub token_type: String,
    pub score: f32,
    pub short_desc: String,
    /// Hit-count bookkeeping (spec §3.2/§3.3) — strictly additive, never
    /// decremented.
    #[serde(default)]
    pub accumulated_hits: u64,
    /// Graph-traversal provenance (spec §4.2/§6.1/§7.1).
    #[serde(default)]
    pub hop_distance: u32,
    /// For traversal-discovered tokens (`hop_distance > 0`), the direct-match
    /// token that led to this token's discovery (the BFS parent). `None` for
    /// direct matches themselves.
    #[serde(default)]
    pub via_token_id: Option<String>,
}

/// Minimal compatible representation of a Tauri MCP server configuration,
/// used to serialize/deserialize `ToolEndpoint` into LanceDB storage rows.
/// Kept in sync with the main crate's `state::config::McpServerConfig`.
///
/// Conversions between this type and the main crate's `McpServerConfig` are
/// handled by `From` impls in the main crate.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct McpServerConfig {
    pub id: String,
    pub name: String,
    #[serde(default = "default_transport")]
    pub transport: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub env: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub headers: std::collections::HashMap<String, String>,
}

fn default_true() -> bool {
    true
}

fn default_transport() -> String {
    "stdio".to_string()
}

/// Enough connection/dispatch info to re-invoke a tool purely from stored
/// metadata.
///
/// SECURITY: `McpServerConfig` can carry `env`/`headers`, which may include
/// secrets (e.g. an API key for an authenticated MCP server). This type must
/// never be exposed to the model.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ToolEndpoint {
    Mcp {
        server: McpServerConfig,
        remote_name: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenDetail {
    pub short_desc: String,
    pub full_desc: Option<String>,
    #[serde(default)]
    pub endpoint: Option<ToolEndpoint>,
    /// Referential ranges, if this is a referential token. Resolved at
    /// display time by the protocol layer.
    #[serde(default)]
    pub ranges: Vec<TokenRange>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NewTokenSpec {
    /// Token ID. Accepts both `id` and `token_id` in JSON for consistency
    /// with other token references in the protocol.
    #[serde(alias = "token_id")]
    pub id: String,
    /// Token type for discoverability filtering.
    /// Defaults to `"concept"` when omitted — the model should not need to
    /// worry about this for most tokens. Explicit types (`"todo"`,
    /// `"decision"`, `"assumption"`, `"workflow"`, `"skill"`, `"tool"`)
    /// are set by the tools/workflows that create them.
    #[serde(rename = "type", default = "default_token_type")]
    pub token_type: String,
    pub short_desc: String,
    #[serde(default)]
    pub full_desc: Option<String>,
    #[serde(default)]
    pub endpoint: Option<ToolEndpoint>,
    /// Optional range references: slices of USR_T* or RESP_T* tokens that
    /// this referential token represents. Resolved at display/explore time
    /// by loading the source token's full_desc and applying bookmark offsets.
    /// See ADR 0012.
    #[serde(default)]
    pub ranges: Vec<TokenRange>,
}

/// A byte-range slice within a chunk token, defined by bookmark + optional
/// offset/length. The range `[bookmark_pos + offset, bookmark_pos + offset + length]`
/// is resolved from the source token's `full_desc` text.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TokenRange {
    /// The storage namespace (e.g. `USR_T1_005` or `RESP_T2_003`).
    pub namespace: String,
    /// The bookmark name as placed by the model via `§^` in its output.
    pub bookmark: String,
    /// Additional byte offset from the bookmark position (default 0).
    #[serde(default)]
    pub offset: usize,
    /// Number of bytes to include (default: rest of token to next bookmark
    /// or end of text).
    #[serde(default)]
    pub length: Option<usize>,
}

impl NewTokenSpec {
    pub fn is_invocable(&self) -> bool {
        self.endpoint.is_some()
    }
}

fn default_token_type() -> String {
    "concept".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Relationship {
    pub subject: String,
    pub predicate: String,
    pub object: String,
}

/// Reserved predicate constants for the process-tree relationship vocabulary.
/// Used by both the Todo Tree (subtask, marked_done) and Decision Tree
/// (considers, selects, drivenBy, etc.) to ensure consistency across the
/// token store.
pub mod predicates {
    /// Todo Tree: parent → child — the child is required for the parent to
    /// be considered complete.
    pub const SUBTASK: &str = "subtask";
    /// Todo Tree: todo node → TRT_ marker — the AI has signalled this node's
    /// own work as finished. `is_done(node)` = has marked_done AND all
    /// subtask children are done.
    pub const MARKED_DONE: &str = "marked_done";
    /// Decision Tree: parent → child — this option was identified as a
    /// plausible branch. Does not imply it was chosen.
    pub const CONSIDERS: &str = "considers";
    /// Decision Tree: parent → child — this is the currently active path.
    pub const SELECTS: &str = "selects";
    /// Decision Tree: child → assumption — the assumption that justified
    /// this selection. Mandatory on every `selects`.
    pub const DRIVEN_BY: &str = "drivenBy";
    /// Decision Tree: assumption → evidence — the concrete evidence that
    /// supported the assumption.
    pub const CONFIRMED_BY: &str = "confirmedBy";
    /// Decision Tree: assumption → evidence — the concrete evidence that
    /// broke the assumption.
    pub const INVALIDATED_BY: &str = "invalidatedBy";
    /// Decision Tree: child → assumption — marks a previously-selected
    /// branch as no longer active, pointing at the assumption that failed.
    pub const ABANDONED: &str = "abandoned";
    /// Decision Tree: child → root — marks a branch as the terminus that
    /// actually solved the original problem.
    pub const RESOLVES: &str = "resolves";
    /// Cross-tree: Todo leaf → Decision Tree root — links a todo that
    /// required investigation to the decision tree opened to resolve it.
    pub const INVESTIGATED_VIA: &str = "investigatedVia";
    /// Generic hierarchy: parent → child — the child is contained within
    /// the parent. Auto-mirrored with `Contains` (inserting a HasParent
    /// edge automatically creates the inverse Contains edge).
    pub const HAS_PARENT: &str = "HasParent";
    /// Generic hierarchy inverse: container → contained. Auto-mirrored
    /// from `HasParent` — never inserted directly.
    pub const CONTAINS: &str = "Contains";

    /// Role assignment: token → role — the source token functions as this
    /// role, assigned by predicate rather than by hardcoded token_type.
    /// These are the spec §2 "roles are graph-assigned" constants.
    pub const IS_A_TOOL: &str = "IS_A_TOOL";
    pub const IS_A_SKILL: &str = "IS_A_SKILL";
    pub const IS_A_WORKFLOW: &str = "IS_A_WORKFLOW";
}

/// Represents the currently-active process tree state for Squire bootstrap
/// injection. Injected at turn-open into the user message alongside
/// `prefetched_tokens` / `preserved_tokens` while any Todo Tree or Decision
/// Tree has open nodes — prevents objective drift by reminding the AI what
/// it was working on.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct ActiveProcessState {
    /// Root of the active Todo Tree, if any descendant is not done.
    pub todo_root: Option<String>,
    /// Computed frontier of incomplete work — leaves where `is_done()` is
    /// false. Keeps the reminder short even for a deep tree.
    pub open_leaves: Vec<String>,
    /// Root of the active Decision Tree, if any branch is unresolved.
    pub dt_root: Option<String>,
    /// The most recently made selects + drivenBy pair not yet followed by
    /// confirmedBy or invalidatedBy.
    pub last_decision: Option<LastDecision>,
}

/// One pending decision in an active Decision Tree — the assumption the AI
/// is currently testing, before evidence has come back.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LastDecision {
    pub node: String,
    pub assumption: String,
    #[serde(rename = "status")]
    pub status: String,
}

/// Structured diagnostic record for a compliance failure that exhausted the
/// retry budget (Q6).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ComplianceFailureRecord {
    pub session_id: SessionId,
    pub rule: String,
    pub reason: String,
    pub retry_count: u32,
    pub failed_content: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Raw-partition audit-log record (spec §4.1/§4.3/§9.4 step 4/§11).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawPartitionRecord {
    pub session_id: SessionId,
    pub turn: u64,
    pub content: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}
