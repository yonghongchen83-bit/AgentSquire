# ADR 0012: Referential Token Ranges — Bookmark-Based Slicing Across USR/RESP Tokens

## Status

Accepted

## Context

Squire chunks both user input (`USR_T{turn}_{NNN}`) and model responses (`RESP_T{turn}_{NNN}`)
using a dumb heuristic: paragraph-then-sentence splitting with a 400-char soft limit. The
resulting tokens have arbitrary boundaries — a logical unit like "the battle scene" or "the
dialogue passage" may be split across multiple chunks, or a single chunk may contain multiple
logical units.

The AI needs a way to define semantic groupings over these raw chunks without duplicating
text. Simply storing the full text again in a concept token is wasteful and defeats the
purpose of chunking.

## Decision

Introduce **bookmark anchors** and **range references** as a first-class mechanism for
defining semantic slices across the token store.

### Bookmarks (`§^`)

A `§^` marker in the model's output text defines a **named position** (a byte offset from
the start of the enclosing `RESP_T*` or `USR_T*` token). Unlike the existing `§^TokenID ...
§^` span syntax (which captures text between markers as a token's `full_desc`), a bare
bookmark with no gap between open and close is just a position marker:

```
§^arena_start§^arena_end
```

Two bookmarks with no content between them — just named positions.

Bookmarks work the same way in both user input and model output:
- In user input: the AI places bookmarks conceptually when creating referential tokens
- In model output: `§^name` in the content text creates a bookmark at that byte offset

### Range references

A referential token carries an optional `ranges` field that defines one or more byte-range
slices:

```json
{
  "new_tokens": [{
    "token_id": "REF_ArenaScene",
    "type": "referential",
    "short_desc": "The arena combat sequence",
    "ranges": [
      {"token": "USR_T1_005", "bookmark": "§^arena_start"},
      {"token": "USR_T1_006", "bookmark": "§^arena_end"}
    ]
  }]
}
```

Each range entry specifies:
- `token` — the chunk token (`USR_T*` or `RESP_T*`) the bookmark lives in
- `bookmark` — the bookmark name
- `offset` (optional, default 0) — additional bytes past the bookmark
- `length` (optional, default = remaining to next bookmark or end of token) — how many bytes

A range `[bookmark + offset, bookmark + offset + length]` resolves to the actual text by:
1. Loading the `full_desc` of the referenced token
2. Finding the bookmark's byte position within that text
3. Applying offset and length

### Resolution at display/explore time

When a referential token with `ranges` is:
- **Displayed via `§!`**: the system resolves the ranges, concatenates the text slices, and
  renders the combined text inline (truncated if very long)
- **Explored via `explore()`**: the `full_desc` is synthesized from the resolved ranges at
  query time

The ranges are stored on the token itself (as a new field on `NewTokenSpec`), not as
separate relationship edges, because a range is a structural property of the token, not a
semantic link between two independent concepts.

## Consequences

### Positive
- AI can define logical groupings over arbitrarily chunked text without duplication
- Bookmarks are lightweight — just names, no token creation overhead
- Same mechanism works for both user input and model output
- Multiple ranges can be combined into one referential token (e.g., a scene that spans
  three chunks)
- Zero cost when not used — existing tokens without `ranges` are unaffected

### Negative
- Range resolution requires reading the source token's `full_desc` and scanning for bookmark
  positions, which is O(n) in the token text size
- Bookmark positions are implicit (byte offset search) rather than explicit stored metadata

### Future considerations
- If bookmark scanning becomes a bottleneck, store bookmark positions as a separate index
- Support for overlapping ranges (same text included in multiple referential tokens)
- Support for "negative" offsets (relative to end of token)
