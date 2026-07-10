# State — JSON Refactoring

## Current Status

2026-07-10: Initial analysis complete. Direction established: **Bookmark Protocol**.

## Design Principle

**Zero matching pairs.** Every structural element follows the bookmark principle:
only open, no close needed. Sections end implicitly at next section or EOF.
Lines are self-contained — a bad line doesn't break others.

## Adopted Format

- Content = free text with §!/§^ sigils (unchanged)
- Metadata sections: `new_tokens`, `relationships`, `preserve`, `ask_user`
- Each section is a line-start keyword, no closing needed
- `|` separator for fields within lines (no quotes, no commas)
- `preserve`: one token ID per line
- `ask_user`: standalone section, replaces content

## Next Steps

1. Prototype parser in Rust (`parse_bookmark_protocol`)
2. Replace `finalize_turn` JSON path with new parser
3. Update system-prompt.md
4. Test with DeepSeek
