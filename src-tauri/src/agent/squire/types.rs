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
            // Match "§#new_tokens", "§#new_tokens:", "§#new_tokens:" (colon),
            // or "#new_tokens" / "#new_tokens:" (missing § — common model mistake)
            let patterns = [
                format!("§#{}", key),
                format!("§#{}:", key),
                format!("#{}", key),
                format!("#{}:", key),
            ];
            if patterns.iter().any(|p| trimmed_line == *p || trimmed_line.starts_with(p)) {
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
                        ranges: vec![], tags: vec![], properties: std::collections::HashMap::new(),
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

// ─────────────────────────── Formatter JSON parsing ───────────────────────

/// Typed formatter JSON output — Phase 4 formatter pass.
/// The formatter model is instructed to output JSON matching this schema.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct FormatterJsonToken {
    pub id: String,
    #[serde(rename = "type")]
    pub token_type: String,
    pub short_desc: String,
    #[serde(default)]
    pub full_desc: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct FormatterJsonRelationship {
    pub subject: String,
    pub predicate: String,
    pub object: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct FormatterOutput {
    #[serde(default)]
    pub new_tokens: Vec<FormatterJsonToken>,
    #[serde(default)]
    pub relationships: Vec<FormatterJsonRelationship>,
    #[serde(default)]
    pub preserve: Vec<String>,
}

/// Best-effort JSON extraction from a text blob that might contain markdown
/// fences or extra commentary. Returns `None` if no JSON object is found.
fn try_extract_json(text: &str) -> Option<String> {
    let text = text.trim();
    // Case 1: pure JSON
    if text.starts_with('{') {
        return Some(text.to_string());
    }
    // Case 2: JSON inside ``` fences
    if let Some(start) = text.find("```json") {
        let after_fence = &text[start + 7..];
        if let Some(end) = after_fence.find("```") {
            return Some(after_fence[..end].trim().to_string());
        }
        return Some(after_fence.trim().to_string());
    }
    if let Some(start) = text.find("```") {
        let after_fence = &text[start + 3..];
        if let Some(end) = after_fence.find("```") {
            return Some(after_fence[..end].trim().to_string());
        }
    }
    // Case 3: JSON somewhere in the text — find first { and last }
    if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            let candidate = &text[start..=end];
            if candidate.len() > 10 {
                return Some(candidate.to_string());
            }
        }
    }
    None
}

/// Parse the formatter model's output into a `SquireResponse` suitable
/// for `finalize_phase2`. Supports both Bookmark Protocol (§# sections,
/// pipe-delimited) and legacy JSON format.
///
/// The formatter prompt was updated to use Bookmark Protocol format
/// (more robust, no JSON escaping issues). Legacy JSON is still accepted
/// via `detect_and_parse` for backward compatibility.
pub fn parse_formatter_json(text: &str) -> Result<SquireResponse, String> {
    // If it looks like Bookmark Protocol (starts with §# or text), use
    // the robust detect_and_parse which auto-detects format.
    let trimmed = text.trim();
    if !trimmed.starts_with('{') {
        return detect_and_parse(text);
    }

    // Legacy JSON format — try to extract and parse, with repair.
    let json_str = match try_extract_json(text) {
        Some(s) => s,
        None => {
            // JSON extraction failed — maybe has markdown fences or commentary.
            // Fall back to detect_and_parse which handles bookmarks too.
            return detect_and_parse(text);
        }
    };

    let cleaned = crate::commands::utils::clean_deepseek_json(&json_str);

    match serde_json::from_str::<FormatterOutput>(&cleaned) {
        Ok(output) => {
            let mut resp = SquireResponse::default();
            resp.content = String::new();
            resp.ask_user = String::new();
            for ft in &output.new_tokens {
                resp.new_tokens.push(NewTokenSpec {
                    id: ft.id.clone(),
                    token_type: ft.token_type.clone(),
                    short_desc: ft.short_desc.clone(),
                    full_desc: ft.full_desc.clone(),
                    endpoint: None,
                    ranges: vec![], tags: vec![], properties: std::collections::HashMap::new(),
                });
            }
            for fr in &output.relationships {
                resp.relationships.push(Relationship {
                    subject: fr.subject.clone(),
                    predicate: fr.predicate.clone(),
                    object: fr.object.clone(),
                });
            }
            resp.preserve = output.preserve.clone();
            Ok(resp)
        }
        Err(json_err) => {
            // JSON parsing failed — try Bookmark Protocol as fallback.
            // The model may have ignored the JSON instruction and used
            // the more natural Bookmark Protocol format instead.
            match detect_and_parse(text) {
                Ok(resp) => Ok(resp),
                Err(_) => Err(format!(
                    "Formatter output parse failed: JSON error: {}, also not valid Bookmark Protocol",
                    json_err
                )),
            }
        }
    }
}

#[cfg(test)]
mod formatter_tests {
    use super::*;

    #[test]
    fn parse_pure_json() {
        let json = r#"{"new_tokens":[{"id":"REF_X","type":"referential","short_desc":"desc","full_desc":"chunk_0→chunk_1"}],"relationships":[{"subject":"REF_X","predicate":"References","object":"USR_T1"}],"preserve":["REF_X"]}"#;
        let resp = parse_formatter_json(json).unwrap();
        assert_eq!(resp.new_tokens.len(), 1);
        assert_eq!(resp.new_tokens[0].id, "REF_X");
        assert_eq!(resp.new_tokens[0].token_type, "referential");
        assert_eq!(resp.relationships.len(), 1);
        assert_eq!(resp.preserve, vec!["REF_X"]);
    }

    #[test]
    fn parse_json_in_fence() {
        let text = "Here's the output:\n```json\n{\"new_tokens\":[],\"relationships\":[],\"preserve\":[\"KEEP_ME\"]}\n```\nDone.";
        let resp = parse_formatter_json(text).unwrap();
        assert_eq!(resp.preserve, vec!["KEEP_ME"]);
    }

    #[test]
    fn parse_returns_error_on_garbage() {
        assert!(parse_formatter_json("not json at all").is_err());
    }

    #[test]
    fn parse_defaults_empty_arrays() {
        let resp = parse_formatter_json("{}").unwrap();
        assert!(resp.new_tokens.is_empty());
        assert!(resp.relationships.is_empty());
        assert!(resp.preserve.is_empty());
    }
}

/// Parse a response, auto-detecting Bookmark Protocol vs legacy JSON.
///
/// If the text starts with `{` it's treated as legacy JSON (with repair);
/// otherwise the normalized Bookmark Protocol parser is used.
pub fn detect_and_parse(text: &str) -> Result<SquireResponse, String> {
    let trimmed = text.trim();
    if trimmed.starts_with('{') {
        // Legacy JSON fallback with repair
        let cleaned = crate::commands::utils::clean_deepseek_json(trimmed);
        serde_json::from_str(&cleaned)
            .map_err(|e| format!("response is not valid Squire protocol JSON: {}", e))
    } else {
        let normalized = normalize_bookmark_protocol(text);
        Ok(parse_bookmark_protocol(&normalized))
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Pre-parse normalization — fixes common model syntax errors before
// the bookmark protocol parser sees the text.  Only fixes unambiguous
// mistakes; anything we can't resolve still surfaces as a parse/rejection
// error.
// ═══════════════════════════════════════════════════════════════════════

/// Apply all normalization passes to raw model output.
fn normalize_bookmark_protocol(raw: &str) -> String {
    let mut text = raw.to_string();
    text = fix_unclosed_spans(&text);
    text = fix_concatenated_rel_lines(&text);
    text = normalize_section_headers(&text);
    text = strip_empty_predicates(&text);
    text
}

/// ── Fix 1: auto-close unclosed §^ spans ─────────────────────────────
///
/// When the model writes `§^REF_wire_format` on its own line without a
/// closing `§^`, the bookmarked content is everything from the marker to
/// the end of the line.  Auto-close by appending `§^` at end-of-line.
///
/// Uses a stateful scan across the entire text (not line-by-line): tracks
/// whether we are inside a span, and if the text ends mid-span, closes it.
fn fix_unclosed_spans(text: &str) -> String {
    let count = text.match_indices("§^").count();
    // If §^ count is even, all spans are balanced — nothing to fix.
    if count % 2 == 0 {
        return text.to_string();
    }

    // Odd count: exactly one span is unclosed.  Find the last §^ and
    // close after it — the content from that marker to end of text is
    // the span body.  Append a closing §^ at the end.
    if let Some(last_pos) = text.rfind("§^") {
        // Check if the last §^ was followed by content.  If it was, the
        // span has a body (e.g. `§^name rest of line...`) and needs a
        // closer appended.  If it was at the very end, it's a bare
        // trailing §^ with no body — strip it.
        let after = text[last_pos + "§^".len()..].trim();
        if after.is_empty() {
            // Trailing bare §^ — just strip the marker
            let before = text[..last_pos].trim_end().to_string();
            return before;
        }
        // Span has body content after the last §^ — close it
        return format!("{}§^", text);
    }

    text.to_string()
}

/// ── Fix 2: de-concatenate relationship lines ──────────────────────────
///
/// When the model runs two relationship entries together on one line:
///   `CON_A | HasParent | wire_detailCON_B | HasParent | stream_detail`
/// we look for token-ID boundaries (snake_case → CAPITAL_camelCase transition
/// or consecutive `|` patterns with >2 pipes) and split them onto separate
/// lines.
fn fix_concatenated_rel_lines(text: &str) -> String {
    let mut result = String::with_capacity(text.len() + 128);

    for line in text.lines() {
        let trimmed = line.trim();
        let pipe_count = trimmed.matches('|').count();

        if pipe_count <= 2 {
            // Normal line — pass through
            result.push_str(line);
            result.push('\n');
            continue;
        }

        // Line has >2 pipes — likely two relationship entries concatenated.
        // Strategy: split the line at points where a token ID boundary is
        // detectable, using the pattern `[token] [token]` where two
        // identifiers abut with a lowercase→uppercase transition marker.
        let mut fixed_lines: Vec<String> = Vec::new();
        let mut remaining = trimmed.to_string();

        while remaining.matches('|').count() > 2 {
            // Find the boundary: after a triple pipe-set, the next token
            // starts.  A complete relationship looks like:
            //   TokenID | predicate | TokenID
            // So 2 pipes make one relationship.  Split after the 2nd pipe.
            if let Some(split_at) = remaining.char_indices()
                .filter(|(_, c)| *c == '|')
                .nth(1)
                .map(|(i, _)| {
                    // Scan forward from after the 2nd | to find the start of
                    // the next token (first non-whitespace).
                    let after = &remaining[i + 1..];
                    i + 1 + after.len() - after.trim_start().len()
                })
            {
                let first = remaining[..split_at].trim().to_string();
                if !first.is_empty() {
                    fixed_lines.push(first);
                }
                remaining = remaining[split_at..].trim().to_string();
            } else {
                break;
            }
        }

        if !remaining.is_empty() && remaining.matches('|').count() <= 2 {
            fixed_lines.push(remaining);
        }

        if fixed_lines.is_empty() {
            result.push_str(line);
            result.push('\n');
        } else {
            for fl in &fixed_lines {
                result.push_str(fl);
                result.push('\n');
            }
        }
    }

    result.trim_end().to_string()
}

/// ── Fix 3: normalize section headers ─────────────────────────────────
///
/// Models often write `### §#new_tokens` (markdown header) or
/// `### Step 2 — Define referential tokens` (instruction) instead of the
/// bare `§#new_tokens` that the parser expects.  Map common patterns to
/// the canonical forms.
fn normalize_section_headers(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let lines: Vec<&str> = text.lines().collect();

    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();

        // Case 1: `### §#keyword` or `## §#keyword` → `§#keyword`
        if let Some(stripped) = trimmed
            .strip_prefix("### ")
            .or_else(|| trimmed.strip_prefix("## "))
            .or_else(|| trimmed.strip_prefix("# "))
        {
            if stripped.starts_with("§#") {
                result.push_str(stripped);
                result.push('\n');
                i += 1;
                continue;
            }
        }

        // Case 2: `### Step N — Define referential tokens` or
        //         `Step 2 — Define referential tokens` without any §# prefix.
        // Look ahead for content that starts a §# section (rows of id|type|desc)
        // and insert the missing §#new_tokens header.
        if (trimmed.starts_with("### Step ") || trimmed.starts_with("Step "))
            && !trimmed.contains("§#")
        {
            // Skip the markdown instruction line — insert §# header instead
            if trimmed.contains("referential") {
                result.push_str("§#new_tokens\n");
            } else if trimmed.contains("relationship") {
                result.push_str("§#relationships\n");
            } else if trimmed.contains("preserve") {
                result.push_str("§#preserve\n");
            } else if trimmed.contains("concept") {
                result.push_str("§#new_tokens\n");
            } else {
                // Can't determine — keep original line
                result.push_str(line);
                result.push('\n');
            }
            i += 1;
            continue;
        }

        // Case 3: `§#new_tokens:` (trailing colon) → `§#new_tokens`
        if let Some(rest) = trimmed.strip_prefix("§#") {
            let rest = rest.trim_end_matches(':').trim();
            if ["new_tokens", "relationships", "preserve", "ask_user"].contains(&rest) {
                result.push_str(&format!("§#{}\n", rest));
                i += 1;
                continue;
            }
        }

        // Case 4: Bare keyword `new_tokens` or `relationships` at column 0
        // (model omitted the §# prefix entirely)
        if trimmed == "new_tokens"
            || trimmed == "relationships"
            || trimmed == "preserve"
            || trimmed == "ask_user"
        {
            result.push_str(&format!("§#{}\n", trimmed));
            i += 1;
            continue;
        }

        // Case 5: `### Preserve` or `### Relationships` without §#
        if (trimmed == "### Preserve" || trimmed == "### Relationships"
            || trimmed == "### New Tokens" || trimmed == "### New tokens")
        {
            let keyword = match trimmed {
                "### Preserve" => "preserve",
                "### Relationships" => "relationships",
                "### New Tokens" | "### New tokens" => "new_tokens",
                _ => "new_tokens",
            };
            result.push_str(&format!("§#{}\n", keyword));
            i += 1;
            continue;
        }

        result.push_str(line);
        result.push('\n');
        i += 1;
    }

    result.trim_end().to_string()
}

/// ── Fix 4: strip empty predicate fields ─────────────────────────────
///
/// Lines like `token | | CONCEPT_X` have an empty middle field.
/// Collapse to `token | CONCEPT_X` (the parser will treat it as a
/// 2-field line and skip it, but at least it won't produce a malformed
/// 3-field entry with an empty predicate).
fn strip_empty_predicates(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    for line in text.lines() {
        let trimmed = line.trim();
        let parts: Vec<&str> = trimmed.split('|').collect();
        if parts.len() == 3 && parts[1].trim().is_empty() {
            let fixed = format!("{} | {}", parts[0].trim(), parts[2].trim());
            result.push_str(&fixed);
        } else {
            result.push_str(line);
        }
        result.push('\n');
    }
    result.trim_end().to_string()
}

#[cfg(test)]
mod normalization_tests {
    use super::*;

    #[test]
    fn auto_closes_unclosed_span_at_end_of_line() {
        let input = "§^REF_wire_format\n\nNext paragraph.";
        let fixed = normalize_bookmark_protocol(input);
        assert!(fixed.matches("§^").count() % 2 == 0,
            "should have even number of §^ markers");
    }

    #[test]
    fn preserves_already_closed_spans() {
        let input = "§^intro\nHello world\n§^\n\nMore text.";
        let fixed = normalize_bookmark_protocol(input);
        assert!(fixed.contains("§^intro\nHello world\n§^"));
    }

    #[test]
    fn strips_markdown_header_from_section() {
        let input = "### §#new_tokens\nTOKEN | concept | desc\n§#";
        let fixed = normalize_bookmark_protocol(input);
        assert!(fixed.contains("§#new_tokens"));
        assert!(!fixed.contains("### §#new_tokens"));
    }

    #[test]
    fn maps_step_instruction_to_section_header() {
        let input = "### Step 2 — Define referential tokens\nREF_X | referential | desc | range\n§#";
        let fixed = normalize_bookmark_protocol(input);
        assert!(fixed.contains("§#new_tokens"));
    }

    #[test]
    fn maps_step_for_relationships() {
        let input = "### Step 4 — Define relationships\nA | pred | B\n§#";
        let fixed = normalize_bookmark_protocol(input);
        assert!(fixed.contains("§#relationships"));
    }

    #[test]
    fn maps_preserve_step() {
        let input = "### Preserve\nTOKEN_A\n§#";
        let fixed = normalize_bookmark_protocol(input);
        assert!(fixed.contains("§#preserve"));
    }

    #[test]
    fn strips_empty_predicate_field() {
        let input = "CON_A | | CON_B";
        let fixed = normalize_bookmark_protocol(input);
        assert_eq!(fixed, "CON_A | CON_B");
    }

    #[test]
    fn leaves_valid_relationship_alone() {
        let input = "CON_A | Contains | CON_B";
        let fixed = normalize_bookmark_protocol(input);
        assert_eq!(fixed, "CON_A | Contains | CON_B");
    }

    #[test]
    fn integration_scenario() {
        let input = concat!(
            "### Step 2 — Define referential tokens\n",
            "REF_intro | referential | Introduction | chunk_0\n\n",
            "CON_Decision | concept | Decision matrix | A framework\n\n",
            "### Step 4 — Define relationships\n",
            "CON_Decision | Contains | CON_Migration\n",
            "CON_Migration | | CON_Strategy\n\n",
            "### Preserve\n",
            "CON_Decision\n",
            "CON_Migration\n",
        );
        let fixed = normalize_bookmark_protocol(input);
        assert!(fixed.contains("§#new_tokens"));
        assert!(fixed.contains("§#relationships"));
        assert!(fixed.contains("§#preserve"));
        assert!(!fixed.contains("| |"));
        assert!(fixed.contains("CON_Migration | CON_Strategy"));
        assert!(!fixed.contains("###"));
    }
}
