//! Squire protocol parsing and validation.
//!
//! Sigil handling (§! inline references, §^ span markers, spec §5.1/§5.2)
//! plus `validate_squire_response` which enforces the protocol's turn-close
//! compliance rules (spec §8.3).

use super::types::{ComplianceFailure, SquireResponse};
use crate::agent::squire::SquireStore;
use squire_store::{NewTokenSpec, TokenRange};
use squire_store::SessionId;

/// The character used to separate start and end positions in a range spec.
const RANGE_ARROW: char = '→';

// ─────────────────────────── Sigil parsing ───────────────────────────

/// Valid characters in a token ID: ASCII alphanumeric, underscore, hyphen.
/// Using constrained char set instead of `!c.is_whitespace()` prevents
/// catastrophic token-ID overrun when a closing sigil is malformed and
/// Chinese/non-ASCII text follows (e.g. `§^tech_sovereignty^技术脱钩...`
/// would previously eat the entire paragraph as the token ID).
fn is_valid_token_id_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}

/// Terminated by the first character that is not a valid token ID character,
/// per spec §5.1/§5.2.  This is a **security-critical** parser boundary:
/// using a strict ASCII identifier character set instead of `!c.is_whitespace()`
/// prevents catastrophic token-ID overrun when a closing sigil is malformed
/// and non-ASCII content follows (e.g. Chinese characters, which are not
/// classified as whitespace by Unicode).
pub(crate) fn take_token_id(s: &str) -> String {
    s.chars()
        .take_while(|c| is_valid_token_id_char(*c))
        .collect()
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

// ─────────────────────────── Additional ref extractors ───────────────────────────

/// Extract every token ID referenced by `§^` spans (closed, unclosed, and
/// bare bookmarks) in `content`.  Deduplicated.
///
/// Unlike `extract_inline_refs` which handles `§!TokenID`, this covers the
/// `§^`-style span markers and bare bookmarks.  The caller uses the combined
/// set to ensure every referenced token has a matching `new_tokens` entry.
pub fn extract_span_refs(content: &str) -> Vec<String> {
    let mut ids = std::collections::BTreeSet::new();
    // Closed & unclosed spans
    let (spans, unclosed) = extract_spans(content);
    for (id, _) in &spans {
        if !id.is_empty() {
            ids.insert(id.clone());
        }
    }
    if let Some(id) = unclosed {
        if !id.is_empty() {
            ids.insert(id);
        }
    }
    // Bare bookmarks
    for (name, _) in extract_bare_bookmarks(content) {
        if !name.is_empty() {
            ids.insert(name);
        }
    }
    ids.into_iter().collect()
}

// ─────────────────────────── Validation (spec §8.3) ───────────────────────────

/// Check for stray `§` characters in content that are not part of valid
/// protocol sigils (`§!`, `§^`, `§#`).
///
/// Catches cases like `§ai_game_changer§^` (missing `^` after opening `§`)
/// which are silently ignored by the permissive bookmark-protocol parser.
pub fn check_malformed_sigils(content: &str) -> Result<(), ComplianceFailure> {
    let mut chars = content.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if c == '§' {
            match chars.peek() {
                Some((_, '!')) | Some((_, '^')) | Some((_, '#')) => {
                    // Valid sigil prefix — consume the second char and continue
                    chars.next();
                }
                Some((_, next)) => {
                    return Err(ComplianceFailure {
                        reason: format!(
                            "malformed sigil: §{} at position {} — expected §!/§^/§#",
                            next, i
                        ),
                    });
                }
                None => {
                    return Err(ComplianceFailure {
                        reason: format!(
                            "malformed sigil: bare § at end of content at position {}",
                            i
                        ),
                    });
                }
            }
        }
    }
    Ok(())
}

/// Validity rules from spec §8.3, extended with additional checks.
///
/// ## Current checks (in order)
///
/// 1. `ask_user` and `content` are mutually exclusive (§8.3)
/// 2. `ask_user` alone is always valid (short-circuits)
/// 3. Empty close response (nothing to store)
/// 4. No stray/malformed `§` sigils in content
/// 5. Every `§!TokenID` resolves to a `new_tokens` entry or store token (§8.3)
/// 6. No unclosed `§^` spans (§8.3) — checked before span-ref resolution
/// 7. Every `§^TokenID` span reference resolves (closed spans and bare bookmarks)
/// 8. Every preserved token exists in the store or is newly defined
/// 9. Every relationship subject/object exists in the store or is newly defined
pub fn validate_squire_response(
    resp: &SquireResponse,
    token_known: impl Fn(&str) -> bool,
) -> Result<(), ComplianceFailure> {
    // ── Check 1: ask_user and content mutually exclusive ──
    if !resp.ask_user.is_empty() && !resp.content.is_empty() {
        return Err(ComplianceFailure {
            reason: "ask_user and content cannot coexist".to_string(),
        });
    }

    // ── Check 2: ask_user alone is always valid ──
    if !resp.ask_user.is_empty() {
        return Ok(());
    }

    // ── Check 3: empty close response ──
    if resp.content.is_empty()
        && resp.new_tokens.is_empty()
        && resp.relationships.is_empty()
        && resp.preserve.is_empty()
    {
        return Err(ComplianceFailure {
            reason: "empty close response".to_string(),
        });
    }

    // ── Check 4: malformed/stray § sigils ──
    check_malformed_sigils(&resp.content)?;

    // ── Check 5: §! inline refs must resolve ──
    for token_id in extract_inline_refs(&resp.content) {
        let defined_inline = resp.new_tokens.iter().any(|t| t.id == token_id);
        if !defined_inline && !token_known(&token_id) {
            return Err(ComplianceFailure {
                reason: format!("undisplayable token §!{}", token_id),
            });
        }
    }

    // ── Check 6: unclosed §^ spans (checked before span ref resolution
    //    because a structurally unclosed span is a worse problem) ──
    let (_, unclosed) = extract_spans(&resp.content);
    if let Some(token_id) = unclosed {
        return Err(ComplianceFailure {
            reason: format!("unclosed §^ span {}", token_id),
        });
    }

    // ── Check 7: §^ span refs must resolve (closed spans and bare bookmarks) ──
    for token_id in extract_span_refs(&resp.content) {
        let defined_inline = resp.new_tokens.iter().any(|t| t.id == token_id);
        if !defined_inline && !token_known(&token_id) {
            return Err(ComplianceFailure {
                reason: format!("undisplayable span reference §^{}", token_id),
            });
        }
    }

    // ── Check 8: preserved tokens must exist ──
    for id in &resp.preserve {
        let defined_inline = resp.new_tokens.iter().any(|t| &t.id == id);
        if !defined_inline && !token_known(id) {
            return Err(ComplianceFailure {
                reason: format!("preserved token does not exist: {}", id),
            });
        }
    }

    // ── Check 9: relationship subjects/objects must exist ──
    for rel in &resp.relationships {
        for token_id in [&rel.subject, &rel.object] {
            let defined_inline = resp.new_tokens.iter().any(|t| &t.id == token_id);
            if !defined_inline && !token_known(token_id) {
                return Err(ComplianceFailure {
                    reason: format!("relationship references unknown token: {}", token_id),
                });
            }
        }
    }

    Ok(())
}

/// Resolve a referential token's `ranges` into the concatenated text.
/// Loads each referenced namespace's `full_desc`, finds the bookmark
/// position within it (by scanning for `§^name§^` in the stored text),
/// applies offset and length, and concatenates the slices.
pub async fn resolve_ranges(
    ranges: &[TokenRange],
    store: &dyn SquireStore,
) -> String {
    let mut result = String::new();
    for range in ranges {
        let detail = store.token_detail(&range.namespace).await;
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

// ─────────────────────────── Range spec parsing (bookend positions) ───────────

/// A single position in a range spec: `[namespace:]bookmark[:offset]`.
/// `namespace` identifies the storage (e.g. USR_T1_001 for a user input chunk,
/// a file path, or any scoped memory). Empty means current-turn context.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RangePosition {
    /// Storage namespace (e.g. USR_T1_001). Empty = current turn (auto-discovered).
    pub namespace: String,
    /// Bookmark name within the token's text.
    pub bookmark: String,
    /// Character offset from the bookmark position (default 0).
    pub offset: usize,
}

/// A parsed range spec: text from `start` position to `end` position.
/// Both positions must be within the same source token (for now).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RangeSpec {
    pub start: RangePosition,
    pub end: RangePosition,
}

/// Parse a single position string.
///
/// Accepts two formats:
///   `bookmark[:offset]`                — bare bookmark (current-turn context)
///   `namespace:bookmark[:offset]`      — explicit namespace + bookmark
///
/// Examples: `chunk_0`, `chunk_0:10`, `USR_T1_001:chunk_0:10`
fn parse_range_position(s: &str) -> Option<RangePosition> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    // If the second colon-separated piece is a number, the format is
    // bookmark:offset (NOT namespace:bookmark).
    let colon_count = s.chars().filter(|&c| c == ':').count();
    let mut parts = s.splitn(3, ':');

    match colon_count {
        0 => {
            // bare bookmark: "chunk_0"
            let bookmark = parts.next()?.to_string();
            if bookmark.is_empty() { return None; }
            Some(RangePosition { namespace: String::new(), bookmark, offset: 0 })
        }
        1 => {
            // Could be "bookmark:10" (bookmark+offset) or "namespace:bookmark"
            let first = parts.next()?.to_string();
            let second = parts.next()?.to_string();
            if first.is_empty() || second.is_empty() { return None; }

            // If second parses as a number, it's an offset — treat first as bookmark
            if let Ok(offset) = second.parse::<usize>() {
                Some(RangePosition { namespace: String::new(), bookmark: first, offset })
            } else {
                // Otherwise it's namespace:bookmark
                Some(RangePosition { namespace: first, bookmark: second, offset: 0 })
            }
        }
        _ => {
            // namespace:bookmark:offset — 3 parts
            let namespace = parts.next()?.to_string();
            let bookmark = parts.next()?.to_string();
            let offset: usize = parts.next().and_then(|o| o.parse().ok()).unwrap_or(0);
            if namespace.is_empty() || bookmark.is_empty() { return None; }
            Some(RangePosition { namespace, bookmark, offset })
        }
    }
}

/// Parse a `→`-delimited range spec: `namespace:bookmark[:offset]→namespace:bookmark[:offset]`.
/// Returns `None` if the string doesn't contain `→` or positions are invalid.
pub(crate) fn parse_range_spec(s: &str) -> Option<RangeSpec> {
    let s = s.trim();
    let arrow_pos = s.find(RANGE_ARROW)?;
    let start_str = &s[..arrow_pos];
    let end_str = &s[arrow_pos + RANGE_ARROW.len_utf8()..];

    let start = parse_range_position(start_str)?;
    let end = parse_range_position(end_str)?;

    Some(RangeSpec { start, end })
}

/// Check whether a string looks like a range spec (contains `→`).
pub(crate) fn is_range_spec(s: &str) -> bool {
    s.contains(RANGE_ARROW)
}

/// Resolve a `RangeSpec` into one or more `TokenRange` entries by loading
/// source tokens' `full_desc` and computing byte-level offset+length.
///
/// When the range position has no token ID (empty string), searches
/// all chunk tokens in the session for the bookmark name. This supports
/// the shorthand format `chunk_0→chunk_1` where the current-turn token
/// is implicit.
///
/// For cross-token ranges (start and end in different chunk tokens), returns an error.
pub(crate) async fn resolve_range_spec(
    spec: &RangeSpec,
    store: &dyn SquireStore,
    session_id: SessionId,
) -> Result<Vec<TokenRange>, String> {
    let start_has_ns = !spec.start.namespace.is_empty();
    let end_has_ns = !spec.end.namespace.is_empty();

    // If both have explicit namespaces, they must match
    if start_has_ns && end_has_ns && spec.start.namespace != spec.end.namespace {
        return Err(format!(
            "cross-namespace ranges not yet supported: {}→{}",
            spec.start.namespace, spec.end.namespace
        ));
    }

    // Resolve the source namespace — if empty, search for the bookmark
    let source_ns = if start_has_ns {
        spec.start.namespace.clone()
    } else if end_has_ns {
        spec.end.namespace.clone()
    } else {
        // Both empty — search all session tokens for the start bookmark
        find_token_with_bookmark(&spec.start.bookmark, store, session_id).await?
    };

    let detail = store.token_detail(&source_ns).await;
    let text = detail
        .and_then(|d| d.full_desc)
        .ok_or_else(|| format!("namespace {} not found or has no full_desc", source_ns))?;

    // Find start bookmark position
    let start_tag = format!("§^{}§^", spec.start.bookmark.trim_start_matches("§^"));
    let start_pos = match text.find(&start_tag) {
        Some(pos) => pos + start_tag.len(),
        None => {
            return Err(format!(
                "bookmark {} not found in namespace {}",
                spec.start.bookmark, source_ns
            ));
        }
    };

    // Find end bookmark position
    let end_tag = format!("§^{}§^", spec.end.bookmark.trim_start_matches("§^"));
    let end_pos = match text.find(&end_tag) {
        Some(pos) => pos + end_tag.len(),
        None => {
            return Err(format!(
                "bookmark {} not found in namespace {}",
                spec.end.bookmark, source_ns
            ));
        }
    };

    let mut range_start = start_pos + spec.start.offset;
    let mut range_end = end_pos + spec.end.offset;

    // Special case: same bookmark on both sides with no meaningful span.
    // The model wrote `chunk_0→chunk_0` meaning "the full chunk" but the
    // resolver sees two identical positions (both point just past the
    // same §^bookmark§^ marker).  Treat this as "from bookmark to end of
    // text" instead of rejecting it as empty.
    if range_end <= range_start
        && spec.start.bookmark == spec.end.bookmark
        && spec.start.namespace == spec.end.namespace
    {
        range_end = text.len();
    }

    if range_end <= range_start {
        return Err(format!(
            "empty or negative range: start={} end={}",
            range_start, range_end
        ));
    }

    let length = range_end - range_start;

    Ok(vec![TokenRange {
        namespace: source_ns,
        bookmark: spec.start.bookmark.clone(),
        offset: spec.start.offset,
        length: Some(length),
    }])
}

/// Search all namespaces in the given session (and global) for one whose
/// `full_desc` contains the given bookmark marker. Returns the namespace ID.
async fn find_token_with_bookmark(
    bookmark_name: &str,
    store: &dyn SquireStore,
    session_id: SessionId,
) -> Result<String, String> {
    let tag = format!("§^{}§^", bookmark_name.trim_start_matches("§^"));
    let mut candidates = store.list_token_ids_by_session(session_id).await;
    // Also search global tokens
    let global = store.list_token_ids_by_session(SessionId::nil()).await;
    candidates.extend(global);
    candidates.sort();
    candidates.dedup();

    for tid in &candidates {
        if let Some(detail) = store.token_detail(tid).await {
            if let Some(ref desc) = detail.full_desc {
                if desc.contains(&tag) {
                    return Ok(tid.clone());
                }
            }
        }
    }
    Err(format!("bookmark '{}' not found in any namespace in this session", bookmark_name))
}

/// Scan all new_tokens in a parsed response. For any token whose full_desc
/// is a range spec (`namespace:bookmark[:offset]→namespace:bookmark[:offset]`),
/// resolve it by loading source namespaces, computing offset+length, and
/// populating `ranges`. The full_desc is cleared after successful resolution
/// (the content will be reconstructed from ranges at display time).
///
/// Tokens whose full_desc is NOT a range spec are left untouched.
pub async fn resolve_all_range_specs(
    tokens: &mut [NewTokenSpec],
    store: &dyn SquireStore,
    session_id: SessionId,
) -> Result<(), String> {
    for token in tokens.iter_mut() {
        let Some(ref desc) = token.full_desc else {
            continue;
        };
        if !is_range_spec(desc) {
            continue;
        }
        let spec = parse_range_spec(desc).ok_or_else(|| {
            format!("invalid range spec in token {}: {}", token.id, desc)
        })?;
        let ranges = resolve_range_spec(&spec, store, session_id).await?;
        token.ranges = ranges;
        token.full_desc = None; // content will be resolved from ranges at display time
    }
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::squire::InMemorySquireStore;
    use squire_store::NewTokenSpec;

    // ─── parse_range_position ───

    #[test]
    fn test_parse_range_position_token_only() {
        let pos = parse_range_position("USR_T1_005:chunk_0").unwrap();
        assert_eq!(pos.namespace, "USR_T1_005");
        assert_eq!(pos.bookmark, "chunk_0");
        assert_eq!(pos.offset, 0);
    }

    #[test]
    fn test_parse_range_position_with_offset() {
        let pos = parse_range_position("USR_T1_005:chunk_0:20").unwrap();
        assert_eq!(pos.namespace, "USR_T1_005");
        assert_eq!(pos.bookmark, "chunk_0");
        assert_eq!(pos.offset, 20);
    }

    #[test]
    fn test_parse_range_position_empty_namespace() {
        assert!(parse_range_position(":chunk_0").is_none());
    }

    #[test]
    fn test_parse_range_position_empty_bookmark() {
        assert!(parse_range_position("USR_T1_005:").is_none());
    }

    // ─── parse_range_spec ───

    #[test]
    fn test_parse_range_spec_simple() {
        let spec = parse_range_spec("USR_T1_005:chunk_0→USR_T1_005:chunk_1").unwrap();
        assert_eq!(spec.start.namespace, "USR_T1_005");
        assert_eq!(spec.start.bookmark, "chunk_0");
        assert_eq!(spec.start.offset, 0);
        assert_eq!(spec.end.namespace, "USR_T1_005");
        assert_eq!(spec.end.bookmark, "chunk_1");
        assert_eq!(spec.end.offset, 0);
    }

    #[test]
    fn test_parse_range_spec_with_offsets() {
        let spec =
            parse_range_spec("USR_T1_005:chunk_0:20→USR_T1_005:chunk_1:3").unwrap();
        assert_eq!(spec.start.namespace, "USR_T1_005");
        assert_eq!(spec.start.offset, 20);
        assert_eq!(spec.end.namespace, "USR_T1_005");
        assert_eq!(spec.end.offset, 3);
    }

    #[test]
    fn test_parse_range_spec_no_arrow() {
        assert!(parse_range_spec("USR_T1_005:chunk_0").is_none());
    }

    #[test]
    fn test_is_range_spec() {
        assert!(is_range_spec("a:b→c:d"));
        assert!(!is_range_spec("a:b-c:d"));
    }

    // ─── resolve_range_spec (unit) ───

    /// Helper to set up an in-memory store with one source token whose
    /// full_desc contains two chunk bookmarks using ASCII-friendly markers.
    async fn setup_store_with_chunks() -> InMemorySquireStore {
        let store = InMemorySquireStore::new();
        // Use <BMK0> and <BMK1> as simple ASCII placeholder markers (7 bytes each)
        // so we don't have to worry about multi-byte UTF-8 in byte-offset arithmetic.
        store
            .upsert_token(
                NewTokenSpec {
                    id: "USR_T1_005".into(),
                    token_type: "source".into(),
                    short_desc: "Test chunk".into(),
                    full_desc: Some(
                        "Some leading text.§^<BMK0>§^Hello World This is a test.§^<BMK1>§^And more text here."
                            .into(),
                    ),
                    endpoint: None,
                    ranges: vec![],
                },
                1,
                uuid::Uuid::nil(),
            )
            .await;
        store
    }

    #[tokio::test]
    async fn test_resolve_range_spec_basic() {
        let store = setup_store_with_chunks().await;
        // Use <BMK0> and <BMK1> as bookmark names (matching the stored text above)
        let spec = parse_range_spec("USR_T1_005:<BMK0>→USR_T1_005:<BMK1>").unwrap();
        let ranges = resolve_range_spec(&spec, &store, SessionId::nil()).await.unwrap();
        assert_eq!(ranges.len(), 1);
        let r = &ranges[0];
        assert_eq!(r.namespace, "USR_T1_005");
        assert_eq!(r.bookmark, "<BMK0>");
        assert_eq!(r.offset, 0);
        // "Some leading text.§^<BMK0>§^" = 31 bytes
        // Then "Hello World This is a test.§^<BMK1>§^" = 39 bytes in range
        assert_eq!(r.length, Some(39));
    }

    #[tokio::test]
    async fn test_resolve_range_spec_with_offset() {
        let store = setup_store_with_chunks().await;
        // From <BMK0>+6 to <BMK1> → skip "Hello " (6 chars), start at "World"
        let spec =
            parse_range_spec("USR_T1_005:<BMK0>:6→USR_T1_005:<BMK1>").unwrap();
        let ranges = resolve_range_spec(&spec, &store, SessionId::nil()).await.unwrap();
        assert_eq!(ranges.len(), 1);
        let r = &ranges[0];
        assert_eq!(r.offset, 6);
        // start=31+6=37, end=70, length=70-37=33 → "World This is a test."
        assert_eq!(r.length, Some(33));
    }

    #[tokio::test]
    async fn test_resolve_range_spec_multi_token_rejected() {
        let store = setup_store_with_chunks().await;
        let spec =
            parse_range_spec("USR_T1_005:<BMK0>→OTHER_TOKEN:<BMK1>").unwrap();
        let result = resolve_range_spec(&spec, &store, SessionId::nil()).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("cross-namespace"));
    }

    #[tokio::test]
    async fn test_resolve_all_range_specs_integration() {
        let store = setup_store_with_chunks().await;
        let mut tokens = vec![
            NewTokenSpec {
                id: "REF_Scene".into(),
                token_type: "referential".into(),
                short_desc: "The combat scene".into(),
                full_desc: Some("USR_T1_005:<BMK0>→USR_T1_005:<BMK1>".into()),
                endpoint: None,
                ranges: vec![],
            },
            NewTokenSpec {
                id: "CON_Theme".into(),
                token_type: "concept".into(),
                short_desc: "Theme of story".into(),
                full_desc: None,
                endpoint: None,
                ranges: vec![],
            },
        ];

        resolve_all_range_specs(&mut tokens, &store, SessionId::nil()).await.unwrap();

        // REF_Scene should have ranges resolved and full_desc cleared
        let ref_token = &tokens[0];
        assert!(!ref_token.ranges.is_empty(), "ranges should be populated");
        assert_eq!(ref_token.full_desc, None, "full_desc should be cleared");
        let range = &ref_token.ranges[0];
        assert_eq!(range.namespace, "USR_T1_005");
        assert_eq!(range.bookmark, "<BMK0>");
        assert_eq!(range.length, Some(39));

        // CON_Theme should be untouched
        let concept_token = &tokens[1];
        assert!(concept_token.ranges.is_empty());
        assert_eq!(concept_token.short_desc, "Theme of story");
    }
}
