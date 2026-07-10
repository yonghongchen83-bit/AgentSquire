# Decisions — JSON Refactoring / Alternative Format Migration

## Problem

LLMs (especially DeepSeek) frequently produce malformed JSON in the `SquireResponse` output.
Current mitigations (safe-json-repair, serde(default), 3 retries) are insufficient — failures still occur.

## Root Cause Analysis

The Squire protocol forces the LLM to output a JSON *container* around its narrative content.
The content part is what users see. The rest (new_tokens, relationships, preserve, ask_user)
is pure protocol machinery. DeepSeek is unreliable at generating valid JSON for this container,
even with repair heuristics.

## Options Evaluated

### Option 1: Grammar-Constrained Decoding (ChatGPT's suggestion)
**Technical approach**: Constrain the LLM's token generation at inference time to match a grammar
**Provider feasibility**:
| Provider | Support |
|----------|---------|
| OpenAI API | ✅ `response_format: {type: "json_schema"}` + `strict: true` |
| Anthropic API | ❌ No equivalent |
| DeepSeek API | ❌ **No support at API level** |
| Local (llama.cpp) | ✅ GBNF grammar |
| Local (vLLM) | ✅ Guided decoding (outlines) |
**Verdict**: Best theoretical solution but **not viable for DeepSeek API**. If primary provider is DeepSeek API, this is a dead end.

---

### Option 2: Full Tool-Call Protocol
**Technical approach**: Define `SquireResponse` as a tool/function the LLM "calls" instead of outputting JSON in content.
Provider SDK handles JSON parsing natively.
**Pros**: Highest JSON reliability; works with all providers
**Cons**: Major architectural rewrite; breaks content-streaming UX; content-as-tool-argument is unnatural
**Verdict**: Over-engineered — content should flow as text, not as a function argument.

---

### Option 3: YAML Format
**Technical approach**: Replace JSON with YAML in system prompt; use serde_yaml for parsing
**Pros**: More whitespace tolerant; supports comments; LLMs sometimes generate YAML more reliably
**Cons**: YAML ambiguities (yes/no booleans, special chars); whitespace-sensitive indentation; tabs vs spaces
**Verdict**: Marginal improvement at best. YAML has its own error class.

---

### Option 4: XML-Style Tagged Format
**Technical approach**: LLM outputs XML-like tags:
```xml
<content>Response with §!T_001</content>
<new_token id="T_001" type="concept">...</new_token>
<relationship subject="A" predicate="RespondsTo" object="B"/>
```
**Pros**: LLMs excellent at matching tags (trained on HTML/docs); self-describing; no quoting issues
**Cons**: Verbose; deep nesting is hard; LLMs forget closing tags; custom parser needed
**Verdict**: Viable but adds parsing complexity.

---

### Option 5: Line-Oriented Protocol
**Technical approach**: Key-value directives, one per line:
```
@content: narrative text...
@preserve: T_001
@new_token: id=T_001 type=concept short_desc=...
```
**Pros**: Extremely simple; hard to get wrong; line-by-line parsing; easy error recovery
**Cons**: Complex nested structures (ranges, endpoints) are very awkward
**Verdict**: Good for flat metadata, poor for nested token specs.

---

### Option 6: Two-Phase Protocol
**Technical approach**: Content first (free text), then `---METADATA---` separator, then structured block
**Pros**: Content quality unaffected; easier partial salvage; metadata can use simpler format
**Cons**: Reliable separator detection needed; LLM may skip separator
**Verdict**: Moderate improvement for salvage behavior.

---

### Option 7: Enhanced Safe-Json-Repair
**Technical approach**: Multi-strategy JSON extractor — find JSON objects anywhere in text,
try partial parsing, try multiple schemas, fuzzy field matching
**Pros**: Backward compatible; incremental improvement
**Cons**: Diminishing returns; fighting symptoms
**Verdict**: Worth doing as safety net regardless.

---

### Option 8: Hybrid Content + Tool Metadata (RECOMMENDED)
**Technical approach**: Split SquireResponse into two channels:
- **Content channel**: LLM outputs free-text narrative with sigils (no JSON wrapping)
- **Tool channel**: LLM calls a `squire_meta` function carrying metadata (new_tokens, relationships, preserve, ask_user)

**Flow**:
```
LLM output:
  <streaming text content with §! and §^ sigils>
  + tool_call: squire_meta({ new_tokens: [...], relationships: [...], ... })

Orchestration loop:
  text chunks → stream to frontend (unchanged)
  squire_meta tool call → extract metadata
  Finalize: combine text + metadata → save to store
```

**Why this works**:
- Content is the LLM's real output — streamed naturally
- Metadata is protocol machinery — packaged as a tool call (its natural form)
- Provider SDK parses tool args JSON natively — much more reliable than content-JSON
- DeepSeek supports OpenAI-compatible function calling — works today
- Minimal Rust code changes: ~add tool def, ~handle tool call in orchestration, ~remove JSON parsing in finalize_turn

**Edge cases**:
- Text only, no tool call → fallback to basic response with empty metadata
- Tool call only, no text → treat as ask_user
- Multiple squire_meta calls → merge (last-write-wins preserve, union for tokens/relationships)
- Tool call parse failure → retry same as today

**Verdict**: **RECOMMENDED** — Best balance of reliability gain vs. architectural disruption.

---

### Option 9: Sigil-Only Protocol (No Separate Metadata)
**Technical approach**: Express everything through content sigils — eliminate JSON entirely.
`§!TokenID` = implicit preserve, `§^TokenID ... §^` = implicit new_token definition.
**Pros**: Zero JSON required; sigil parsing already exists
**Cons**: No explicit relationships; no explicit preserve control; no ask_user mechanism
**Verdict**: Interesting long-term direction but loses protocol expressiveness.

---

## Final Recommendation

| Priority | Approach | When |
|----------|----------|------|
| 🥇 **Primary** | **Bookmark Protocol (零JSON)** | Implement now |
| 🥈 Safety net | **Option 7: Enhanced JSON Repair** | Keep as backward compat fallback |

### Implementation order for Bookmark Protocol

1. Write `parse_bookmark_protocol()` in `adapter.rs` (or new file `protocol_v2.rs`)
2. Replace `finalize_turn` JSON parsing with bookmark protocol call
3. Update `system-prompt.md` — remove JSON format, explain `§^` section markers
4. Add backward-compat detection: if input starts with `{`, fall back to old JSON path
5. After validation: remove `safe-json-repair` dep, remove `validate_squire_response`, simplify `adapter.rs`


