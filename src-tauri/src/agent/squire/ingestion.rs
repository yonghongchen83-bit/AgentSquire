//! Tool-token and text-chunk ingestion into any `SquireStore`.
//!
//! Three backend-agnostic ingestion functions:
//! - `ingest_tool_registry` — upserts one `tool`-typed `SquireStore` token
//!   per entry in the app's real `ToolRegistry` (local built-ins + MCP tools)
//! - `ingest_user_input_chunks` — splits the latest user message into
//!   `USR_T{turn}_{NNN}` `system_referential`-typed tokens
//! - `ingest_response_chunks` — splits the model's response into
//!   `RESP_T{turn}_{NNN}` `system_referential`-typed tokens
//!
//! Both chunking paths use the same dumb heuristic (100-200 chars, sentence
//! boundaries if possible). The AI can then place `§^bookmark` references at
//! byte offsets within these chunks and create referential tokens that
//! define semantic ranges — see `/ArchitecturePlanning/adr/0012-referential-token-ranges.md`.

use std::collections::HashMap;

use super::types::{NewTokenSpec, ToolEndpoint};
use super::SquireStore;
use crate::agent::{ToolDefinition, ToolRegistry};
use crate::storage::conversation_store::SessionId;

// ─────────────────────────── Tool-token ingestion (ss-9) ───────────────────────────

/// Deterministic token id for a tool discovered via the live `ToolRegistry`:
/// the registry name itself, unprefixed. Local built-ins have fixed,
/// hardcoded names; MCP tools get a stable `mcp_{server_id}_{tool_id}` local
/// name assigned once per discovery pass by `streaming_cmd.rs`'s existing
/// sanitization scheme — as long as neither a server's configured id nor a
/// remote tool's advertised name changes, this is stable across repeated
/// ingestion calls (an unprefixed id, matching the registry name exactly,
/// ensures that a token discovered via `explore(resource_type="tool_skill")`
/// and then called directly by name resolves consistently).
pub fn tool_token_id(registry_name: &str) -> String {
    registry_name.to_string()
}

/// Full description for an ingested tool token, matching spec §3.1's
/// type-enforced format for tools ("Standard MCP tool schema — name,
/// description, input schema") and, byte-for-byte, the same JSON shape
/// `SquireTokenToDetailTool::execute`'s existing `detail_level == "full"`
/// branch already returns for registry-sourced tools — so a caller sees the
/// same "full tool description" shape whether it came from the live
/// registry or from an ingested store row.
fn tool_full_desc(def: &ToolDefinition) -> String {
    serde_json::json!({
        "name": def.name,
        "description": def.description,
        "input_schema": def.input_schema,
    })
    .to_string()
}

/// Ingests the app's real tool registry into `store` as `tool`-typed tokens
/// (ss-9: `squire-storage/todo.json`'s flagged follow-up — "a write path
/// that turns MCP/local tool discovery into persisted, invocable SquireStore
/// token rows"). Backend-agnostic: calls only `SquireStore::upsert_token`, so
/// it works unmodified against both `InMemorySquireStore` and
/// `LanceDbSquireStore`, and against any future implementation of the trait.
///
/// `creation_turn` is passed as `0` for every tool — tool discovery is not
/// scoped to any one session's turn counter (see decisions.md's
/// "`creation_turn`" section for the full rationale); `upsert_token`'s
/// existing "preserve creation_turn on update" semantics mean this only
/// matters for a token's very first ingestion.
///
/// Intended call site: once per turn, immediately after `ToolRegistry` is
/// fully assembled (local built-ins + MCP discovery) in
/// `commands::streaming_cmd::send_message_impl` — the one point in the
/// codebase where "the full, current set of available tools" is known, for
/// both Legacy and Squire mode sessions (see decisions.md's "Trigger point"
/// section for why this is the correct, and only real, trigger point today).
///
/// `endpoints` (added by `token-detail-endpoint`): an optional side-channel
/// map from a tool's registry name (the same name `tool_token_id` uses as
/// the token id) to the `ToolEndpoint` needed to re-dispatch it purely from
/// stored metadata. `ToolDefinition` itself erases MCP-vs-local origin once a
/// tool is registered, so callers that know the origin (today, only
/// `streaming_cmd.rs`'s MCP-discovery loop, which still holds the
/// `McpServerConfig`/remote tool name at registration time) pass it here
/// instead. Absent from the map (or an empty/`None` map, e.g. every existing
/// caller/test predating this node) means `endpoint: None` is written —
/// exactly today's pre-`token-detail-endpoint` behavior — so this parameter
/// is purely additive. See decisions.md's "Ingestion call-site threading"
/// section for why this isn't instead recovered by parsing the registry name.
pub async fn ingest_tool_registry(
    registry: &ToolRegistry,
    store: &dyn SquireStore,
    endpoints: &HashMap<String, ToolEndpoint>,
) {
    let global_session = SessionId::nil();
    for def in registry.definitions() {
        store
            .upsert_token(
                NewTokenSpec {
                    id: tool_token_id(&def.name),
                    token_type: "tool".to_string(),
                    short_desc: def.description.clone(),
                    full_desc: Some(tool_full_desc(&def)),
                    endpoint: endpoints.get(&def.name).cloned(),
                    ranges: vec![],
                },
                0,
                global_session,
            )
            .await;
    }
}

// ─────────────────────────── User-input chunking (spec §3.1/§4.3/§9.1/§11) ───────────────────────────

/// Soft size cap (characters) above which a paragraph is further split on
/// sentence boundaries. Not a spec-derived value — spec §15's configuration
/// table has no chunk-size constant — chosen as a documented judgment call;
/// see decisions.md's "(2) What 'chunk' means" section for the rationale.
pub(crate) const CHUNK_SOFT_LIMIT_CHARS: usize = 400;

/// Splits `text` into "natural language structure" chunks (spec §4.3's exact
/// phrase; decisions.md's "(2) What 'chunk' means" documents the judgment
/// call this resolves to): first by blank-line paragraph boundaries, then,
/// for any paragraph longer than `CHUNK_SOFT_LIMIT_CHARS`, further split on
/// sentence-ending punctuation (`.`/`!`/`?` followed by whitespace or
/// end-of-string). A short single-paragraph message (the common chat-message
/// case) produces exactly one chunk. Never returns an empty chunk string;
/// returns an empty `Vec` only for whitespace-only/empty input.
pub fn chunk_user_input(text: &str) -> Vec<String> {
    let paragraphs: Vec<&str> = text
        .split("\n\n")
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .collect();

    let mut chunks = Vec::new();
    for para in paragraphs {
        if para.len() <= CHUNK_SOFT_LIMIT_CHARS {
            chunks.push(para.to_string());
            continue;
        }
        chunks.extend(split_into_sentences(para));
    }
    chunks
}

/// Sentence-boundary split used only for paragraphs exceeding
/// `CHUNK_SOFT_LIMIT_CHARS` (see `chunk_user_input`). Splits after a
/// `.`/`!`/`?` that is followed by whitespace or end-of-string. Known,
/// accepted imprecision: does not special-case abbreviations ("Dr.") or
/// decimal numbers ("3.14") — see decisions.md.
fn split_into_sentences(paragraph: &str) -> Vec<String> {
    let mut sentences = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = paragraph.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        current.push(c);
        if matches!(c, '.' | '!' | '?') {
            let next_is_boundary = chars.get(i + 1).map(|n| n.is_whitespace()).unwrap_or(true);
            if next_is_boundary {
                let trimmed = current.trim();
                if !trimmed.is_empty() {
                    sentences.push(trimmed.to_string());
                }
                current = String::new();
            }
        }
        i += 1;
    }
    let trimmed = current.trim();
    if !trimmed.is_empty() {
        sentences.push(trimmed.to_string());
    }
    if sentences.is_empty() {
        vec![paragraph.to_string()]
    } else {
        sentences
    }
}

/// First sentence of `chunk` (spec §3.1's worked example field comment:
/// "short_desc: (first sentence of the chunk)") — up to the first
/// `.`/`!`/`?` followed by whitespace/end-of-string, or a newline, or the
/// whole chunk if none is found.
pub(crate) fn first_sentence(chunk: &str) -> String {
    let bytes_end = chunk
        .char_indices()
        .find(|(idx, c)| {
            if *c == '\n' {
                return true;
            }
            if matches!(c, '.' | '!' | '?') {
                let rest = &chunk[idx + c.len_utf8()..];
                return rest.chars().next().map(|n| n.is_whitespace()).unwrap_or(true);
            }
            false
        })
        .map(|(idx, c)| idx + c.len_utf8());

    match bytes_end {
        Some(end) => chunk[..end].trim().to_string(),
        None => chunk.trim().to_string(),
    }
}

/// Ingests a text (user input or model response) as `{prefix}_T{turn}_{NNN}_{session_short}`-id
/// `system_referential` tokens. Same chunking algorithm regardless of source.
///
/// `prefix` should be `"USR"` for user input or `"RESP"` for model responses.
/// `session_id` is embedded in the token ID to prevent cross-session collisions —
/// two different sessions never produce the same token ID, even at the same turn.
///
/// Returns the list of token IDs created, in order (same order as `chunk_user_input`).
pub async fn ingest_text_chunks(
    text: &str,
    turn: u64,
    prefix: &str,
    store: &dyn SquireStore,
    session_id: SessionId,
) -> Vec<String> {
    let session_short = &session_id.simple().to_string()[..8];
    let mut ids = Vec::new();
    for (i, chunk) in chunk_user_input(text).into_iter().enumerate() {
        let id = format!("{}_T{}_{:03}_{}", prefix, turn, i + 1, session_short);
        // Embed a bare bookmark at the start of each chunk so the AI can
        // create referential tokens via new_tokens with a `ranges` entry
        // pointing to this bookmark (spec §5.2, ADR 0012).  The chunk
        // content after the bookmark IS the token's text — the AI sees
        // the same bookmark in user_request and can correlate.
        let bookmarked = format!("§^chunk_{}§^{}", i, chunk);
        store
            .upsert_token(
                NewTokenSpec {
                    id: id.clone(),
                    token_type: "system_referential".to_string(),
                    short_desc: first_sentence(&chunk),
                    full_desc: Some(bookmarked),
                    endpoint: None,
                    ranges: vec![],
                },
                turn,
                session_id,
            )
            .await;
        ids.push(id);
    }
    ids
}

/// Backward-compat alias: ingests user input as `USR_T{turn}_{NNN}_{session_short}` tokens.
///
/// Returns the list of token IDs created, in order.
pub async fn ingest_user_input_chunks(
    text: &str,
    turn: u64,
    store: &dyn SquireStore,
    session_id: SessionId,
) -> Vec<String> {
    ingest_text_chunks(text, turn, "USR", store, session_id).await
}

/// Ingests model response as `RESP_T{turn}_{NNN}_{session_short}` tokens.
pub async fn ingest_response_chunks(
    text: &str,
    turn: u64,
    store: &dyn SquireStore,
    session_id: SessionId,
) {
    let _ = ingest_text_chunks(text, turn, "RESP", store, session_id).await;
}
