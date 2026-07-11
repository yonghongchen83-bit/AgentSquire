# ADR 0012: Referential Token Ranges — Bookmark-Based Slicing Within Namespaces

## Status

Accepted

## Context

Squire chunks both user input (`USR_T{turn}_{NNN}`) and model responses (`RESP_T{turn}_{NNN}`)
into separate namespaces using a dumb heuristic: paragraph-then-sentence splitting with a
400-char soft limit. The resulting chunks have arbitrary boundaries — a logical unit like
"the battle scene" or "the dialogue passage" may be split across multiple namespaces, or a
single namespace may contain multiple logical units.

The AI needs a way to define semantic groupings over these raw namespaces without duplicating
text. Simply storing the full text again in a concept token is wasteful and defeats the
purpose of chunking.

## Decision

Introduce **bookmark anchors** and **range references** as a first-class mechanism for
defining semantic slices within a namespace.

### Bookmarks (`§^`)

A `§^` marker in the model's output text defines a **named position** (a byte offset from
the start of the enclosing `RESP_T*` or `USR_T*` namespace). Unlike the existing `§^TokenID ... §^` span syntax (which captures text between markers as a token's `full_desc`), a bare
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
slices within a single namespace:

```json
{
  "new_tokens": [{
    "token_id": "REF_ArenaScene",
    "type": "referential",
    "short_desc": "The arena combat sequence",
    "ranges": [
      {"namespace": "USR_T1_005", "bookmark": "arena_start", "offset": 0, "length": 150}
    ]
  }]
}
```

Each range entry specifies:
- `namespace` — the storage namespace identifier
- `bookmark` — the bookmark name (without §^)
- `offset` — additional bytes past the bookmark position (default 0)
- `length` — number of bytes to include from the offset

A range resolves to actual text by:
1. Loading the stored text of the namespace
2. Finding the bookmark's byte position within that text
3. Applying offset and taking `length` bytes

All ranges in a referential token belong to the same namespace.

## Consequences

### Positive

- AI can define logical groupings over arbitrarily chunked text without duplication
- Bookmarks are lightweight — just names, no token creation overhead
- Same mechanism works for both user input and model output
- Multiple ranges can be combined into one referential token (e.g., multiple extracts from the same namespace)
- Zero cost when not used — existing tokens without `ranges` are unaffected

### Negative

- Range resolution requires reading the namespace's `full_desc` and scanning for bookmark
  positions, which is O(n) in the stored text size
- Bookmark positions are implicit (byte offset search) rather than explicit stored metadata

### Future considerations

- If bookmark scanning becomes a bottleneck, store bookmark positions as a separate index
- Support for overlapping ranges (same text included in multiple referential tokens)
- Support for "negative" offsets (relative to end of content)
