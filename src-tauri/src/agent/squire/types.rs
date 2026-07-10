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

// ─────────────────────────── Bookmark Protocol parsing ───────────────────────

/// Parse a Bookmark Protocol response into a `SquireResponse`.
///
/// The protocol uses `§#keyword` section markers with `|`-delimited data lines.
/// Zero paired delimiters, zero quotes, zero commas — designed for DeepSeek
/// tolerance.
pub fn parse_bookmark_protocol(text: &str) -> SquireResponse {
    const SECTION_KEYS: &[&str] = &["new_tokens", "relationships", "preserve", "ask_user"];

    let mut resp = SquireResponse::default();
    let mut content_lines: Vec<&str> = Vec::new();
    let mut current_section: Option<&str> = None;

    for line in text.lines() {
        let trimmed_line = line.trim();
        let mut is_header = false;
        for key in SECTION_KEYS {
            if trimmed_line == format!("§#{}", key) {
                current_section = Some(key);
                is_header = true;
                break;
            }
        }
        if is_header {
            continue;
        }

        match current_section {
            None => {
                content_lines.push(line);
            }
            Some("new_tokens") => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let parts: Vec<&str> = trimmed.split('|').collect();
                if parts.len() >= 3 {
                    resp.new_tokens.push(NewTokenSpec {
                        id: parts[0].trim().to_string(),
                        token_type: parts[1].trim().to_string(),
                        short_desc: parts[2].trim().to_string(),
                        full_desc: parts.get(3).map(|s| s.trim().to_string()),
                        endpoint: None,
                        ranges: vec![],
                    });
                }
            }
            Some("relationships") => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let parts: Vec<&str> = trimmed.split('|').collect();
                if parts.len() >= 3 {
                    resp.relationships.push(Relationship {
                        subject: parts[0].trim().to_string(),
                        predicate: parts[1].trim().to_string(),
                        object: parts[2].trim().to_string(),
                    });
                }
            }
            Some("preserve") => {
                let id = line.trim();
                if !id.is_empty() {
                    resp.preserve.push(id.to_string());
                }
            }
            Some("ask_user") => {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    resp.ask_user.push_str(trimmed);
                    resp.ask_user.push('\n');
                }
            }
            _ => {}
        }
    }

    resp.content = content_lines.join("\n").trim().to_string();
    resp.ask_user = resp.ask_user.trim().to_string();
    resp
}

/// Parse a response, auto-detecting Bookmark Protocol vs legacy JSON.
///
/// If the text starts with `{` it's treated as legacy JSON (with repair);
/// otherwise the Bookmark Protocol parser is used (always-tolerant).
pub fn detect_and_parse(text: &str) -> Result<SquireResponse, String> {
    let trimmed = text.trim();
    if trimmed.starts_with('{') {
        // Legacy JSON fallback with repair
        let cleaned = crate::commands::utils::clean_deepseek_json(trimmed);
        serde_json::from_str(&cleaned)
            .map_err(|e| format!("response is not valid Squire protocol JSON: {}", e))
    } else {
        Ok(parse_bookmark_protocol(text))
    }
}
