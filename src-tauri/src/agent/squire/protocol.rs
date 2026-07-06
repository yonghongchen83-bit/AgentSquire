//! Squire protocol parsing and validation.
//!
//! Sigil handling (§! inline references, §^ span markers, spec §5.1/§5.2)
//! plus `validate_squire_response` which enforces the protocol's turn-close
//! compliance rules (spec §8.3).

use super::types::{ComplianceFailure, SquireResponse};
use crate::agent::squire::SquireStore;
use squire_store::TokenRange;

// ─────────────────────────── Sigil parsing ───────────────────────────

/// Terminated by whitespace or the next `§`, per spec §5.1/§5.2.
pub(crate) fn take_token_id(s: &str) -> String {
    s.chars().take_while(|c| !c.is_whitespace()).collect()
}

/// `§!TokenID` occurrences in `content` (spec §5.1).
pub fn extract_inline_refs(content: &str) -> Vec<String> {
    content
        .split('§')
        .skip(1)
        .filter_map(|part| part.strip_prefix('!'))
        .map(take_token_id)
        .filter(|id| !id.is_empty())
        .collect()
}

/// Extract bare bookmarks (`§^name§^` with no content between markers).
/// Returns a list of `(bookmark_name, byte_offset)` pairs sorted by offset.
/// A bare bookmark is `§^` followed by a name, immediately followed by `§^`
/// — meaning the two markers are adjacent with only the name between them.
pub fn extract_bare_bookmarks(content: &str) -> Vec<(String, usize)> {
    let mut bookmarks = Vec::new();
    let chars: Vec<char> = content.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        // Look for §^
        if i + 1 < chars.len() && chars[i] == '§' && chars[i + 1] == '^' {
            let start = i + 2; // past §^
            // Read the bookmark name
            let name_end = chars[start..]
                .iter()
                .position(|c| c.is_whitespace() || *c == '§')
                .map(|pos| start + pos)
                .unwrap_or(chars.len());
            let name: String = chars[start..name_end].iter().collect();
            if !name.is_empty() {
                // Check if next character after name is §^ (bare bookmark — no content)
                if name_end + 1 < chars.len()
                    && chars[name_end] == '§'
                    && chars[name_end + 1] == '^'
                {
                    // Bare bookmark: offset is right after the closing §^
                    let offset = name_end + 2;
                    bookmarks.push((name, offset));
                    i = offset;
                    continue;
                }
                // Otherwise it's a span open — skip to closing §^
                if let Some(close) = chars[name_end..].windows(2).position(|w| w == ['§', '^']) {
                    i = name_end + close + 2;
                    continue;
                }
            }
        }
        i += 1;
    }
    bookmarks
}

/// `§^TokenID content §^` spans (spec §5.2). Returns the closed spans found
/// and, if the content ends mid-span, the token id of the unclosed one.
pub fn extract_spans(content: &str) -> (Vec<(String, String)>, Option<String>) {
    let parts: Vec<&str> = content.split("§^").collect();
    let mut spans = Vec::new();
    let mut unclosed = None;
    let mut i = 1;
    while i < parts.len() {
        let opening = parts[i];
        let token_id = take_token_id(opening);
        if token_id.is_empty() {
            // Bare `§^` with nothing pending open — not a valid open tag; skip.
            i += 1;
            continue;
        }
        let rest = &opening[token_id.len()..];
        if i + 1 < parts.len() {
            spans.push((token_id, rest.trim().to_string()));
            i += 2;
        } else {
            unclosed = Some(token_id);
            i += 1;
        }
    }
    (spans, unclosed)
}

pub(crate) fn strip_span_markers(content: &str) -> String {
    let parts: Vec<&str> = content.split("§^").collect();
    let mut out = String::new();
    out.push_str(parts[0]);
    let mut i = 1;
    while i < parts.len() {
        let opening = parts[i];
        let token_id = take_token_id(opening);
        if token_id.is_empty() {
            i += 1;
            continue;
        }
        out.push_str(opening[token_id.len()..].trim());
        i += 1;
        if i < parts.len() {
            out.push_str(parts[i]);
            i += 1;
        }
    }
    out
}

/// Raw-partition extraction (spec §4.1/§4.3: "if the AI does not mark a
/// span, it is stored only in the raw partition"). Returns the portion of
/// `content` that falls OUTSIDE every closed `§^...§^` span — the text the
/// AI produced but did not promote into a structured, addressable memory
/// token. A close sibling of `strip_span_markers` (same `split("§^")`
/// traversal shape), but where that function *keeps* span bodies (for clean
/// display prose) and discards only the markers, this function discards the
/// span bodies too, keeping only the text outside them. Segments are joined
/// with a single space and the result is trimmed, so a response that is
/// entirely one closed span (nothing before or after it) correctly yields
/// an empty string — see `finalize_turn`'s call site for why callers should
/// skip persisting an empty result rather than write a pointless empty row.
pub(crate) fn unmarked_residual(content: &str) -> String {
    let parts: Vec<&str> = content.split("§^").collect();
    let mut segments: Vec<&str> = vec![parts[0]];
    let mut i = 1;
    while i < parts.len() {
        let opening = parts[i];
        let token_id = take_token_id(opening);
        if token_id.is_empty() {
            // Bare `§^` with nothing pending open — not a valid open tag;
            // its trailing text (up to the next marker, if any) is outside
            // any span, so it counts as unmarked residual.
            segments.push(opening);
            i += 1;
            continue;
        }
        // `opening[token_id.len()..]` is the span body (if closed) — never
        // pushed to `segments`, since it belongs to the structured
        // partition, not the raw one.
        i += 1;
        if i < parts.len() {
            // parts[i] is the text after this span's closing `§^` marker,
            // up to the next marker (or end of content) — outside any span.
            segments.push(parts[i]);
            i += 1;
        }
        // If `i >= parts.len()` here, the span was unclosed — in practice
        // unreachable at finalize_turn's call site, since
        // validate_squire_response already rejects unclosed spans before
        // this function is ever called on a compliant response.
    }
    segments
        .into_iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

// ─────────────────────────── Validation (spec §8.3) ───────────────────────────

/// Validity rules from spec §8.3.
pub fn validate_squire_response(
    resp: &SquireResponse,
    token_known: impl Fn(&str) -> bool,
) -> Result<(), ComplianceFailure> {
    if !resp.ask_user.is_empty() && !resp.content.is_empty() {
        return Err(ComplianceFailure {
            reason: "ask_user and content cannot coexist".to_string(),
        });
    }

    if !resp.ask_user.is_empty() {
        return Ok(());
    }

    if resp.content.is_empty()
        && resp.new_tokens.is_empty()
        && resp.relationships.is_empty()
        && resp.preserve.is_empty()
    {
        return Err(ComplianceFailure {
            reason: "empty close response".to_string(),
        });
    }

    for token_id in extract_inline_refs(&resp.content) {
        let defined_inline = resp.new_tokens.iter().any(|t| t.id == token_id);
        if !defined_inline && !token_known(&token_id) {
            return Err(ComplianceFailure {
                reason: format!("undisplayable token §!{}", token_id),
            });
        }
    }

    let (_, unclosed) = extract_spans(&resp.content);
    if let Some(token_id) = unclosed {
        return Err(ComplianceFailure {
            reason: format!("unclosed §^ span {}", token_id),
        });
    }

    Ok(())
}

/// Resolve a referential token's `ranges` into the concatenated text.
/// Loads each referenced chunk token's `full_desc`, finds the bookmark
/// position within it (by scanning for `§^name§^` in the stored text),
/// applies offset and length, and concatenates the slices.
pub async fn resolve_ranges(
    ranges: &[TokenRange],
    store: &dyn SquireStore,
) -> String {
    let mut result = String::new();
    for range in ranges {
        let detail = store.token_detail(&range.token).await;
        let Some(text) = detail.and_then(|d| d.full_desc) else {
            continue;
        };
        // Scan for the bookmark in the stored text
        let bookmark_tag = format!("§^{}§^", range.bookmark.trim_start_matches("§^"));
        let bookmark_pos = match text.find(&bookmark_tag) {
            Some(pos) => pos + bookmark_tag.len(), // position is right after the closing §^
            None => continue,
        };
        let start = bookmark_pos + range.offset;
        let end = match range.length {
            Some(len) => start + len,
            None => text.len(),
        };
        if start < text.len() {
            let end = end.min(text.len());
            result.push_str(&text[start..end]);
            // Add a separator between ranges for readability
            result.push('\n');
        }
    }
    result.trim().to_string()
}
