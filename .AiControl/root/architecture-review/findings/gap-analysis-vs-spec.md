# Gap Analysis: Current Implementation vs `squire-architecture-spec.md`

**Date:** 2026-07-11  
**Scope:** Compare the shipped Squire implementation (17 completed nodes, 223 tests) against the latest `squire-architecture-spec.md`.

---

## Summary

The uploaded spec is a **significant architectural revision** from `context_squire_spec_v2.md` (the design document the current implementation was built against). It simplifies the token model, restructures the turn lifecycle, and introduces several new primitives. Of the ~11 major architectural areas in the spec:

| Area | Status |
|------|--------|
| Token Model (§2) | **Roles conflated with types** — 7 of 10 current `token_type` values are hardcoded roles, not real types. Spec fixes this: 3 types (source, referential, concept) + roles via relationships. Schema change is trivial (just add `"source"`); real work is refactoring 5 modules that hardcode roles as types. |
| Retrieval / explore() (§3) | **Partial** — missing `rdf()`, batch syntax, batch cap |
| Context Assembly / Short+Long Lists (§4) | **Partial** — current prefetch/preserve model exists but differs in algorithm |
| Token Activeness / hitcount (§5) | **Complete** — correctly implemented |
| Tools as Tokens (§6) | **Partial** — tool-token ingestion exists, but live-registry shortcut still active |
| Turn Lifecycle / Formatter Pass (§7) | **Major gap** — different architecture (Phase 2 vs Formatter) |
| Workflow Governance (§8) | **Not implemented** — workflows discoverable but no formal governance |
| Compaction /compact (§9) | **Not implemented** — deferred by spec |
| Namespace scoping (§10 item 1) | **Deferred by spec** — session-scoped only |
| Provenance/trust (§10 item 2) | **Deferred by spec** — known security gap |
| Token invalidation (§10 item 3) | **Deferred by spec** |

---

## Detailed Gap Inventory

### GAP-A: Token Model — Roles vs Types (§2)

**Spec says:** Squire recognizes exactly 4 token primitives:
- **Source** — raw ingested text, chunked into addressable segments
- **Referential** — points into a range within a **single source token** (never duplicates text)
- **Concept** — pure semantic node, short+long description, no physical text
- **Relationship** — triplets (subject, predicate, object) connecting any two tokens

Token type is NOT the same as role. Roles are graph-assigned via relationships, not hardcoded as a type field. A raw text file is a source token; a relationship triplet asserting `{token_id IS_A_WORKFLOW WorkflowRegistry}` gives it the role of "workflow" without changing its type.

**Current implementation:** 10 values in the `token_type` string field, conflating types with roles:

| `token_type` value | What it actually represents | Should be (per spec) |
|---|---|---|
| `"concept"` | A pure semantic node | `concept` (matches spec) |
| `"referential"` | Pointer to a chunk | `referential` (matches spec) |
| `"system_referential"` | Auto-chunked raw text | `source` (this IS the source type) |
| `"user"` | Edge case in bookmark parsing | `source` (also raw text) |
| `"tool"` | A tool's MCP metadata | role via relationship, type = `source` |
| `"skill"` | Skill document text | role via relationship, type = `source` |
| `"workflow"` | Workflow document text | role via relationship, type = `source` |
| `"decision"` | Decision tree node | role via relationship, type = `concept`/`referential` |
| `"assumption"` | Assumption in decision tree | role via relationship, type = `concept`/`referential` |
| `"todo"` | Todo tree item | role via relationship, type = `concept`/`referential` |

**The real gap is not structural — it's that 7 of the 10 values are hardcoded roles, not real Squire types.** The `StoredToken` schema already has a plain-string `token_type` field and a separate `ranges` field; adding a `"source"` type value is trivial. The work is in changing the modules that currently hardcode non-standard types:

- `decision_tree.rs` — currently creates tokens with `token_type: "decision"` / `"assumption"` → should create `concept`/`referential` tokens + `HAS_ASSUMPTION` / `HAS_DECISION` relationship triplets
- `todo_tree.rs` — currently creates tokens with `token_type: "todo"` → should create `concept`/`referential` tokens + `IS_A_TODO` relationship
- `squire_skills.rs` — currently creates tokens with `token_type: "skill"` → should create `source` tokens + `IS_A_SKILL` relationship
- `squire_workflows.rs` — currently creates tokens with `token_type: "workflow"` → should create `source` tokens + `IS_A_WORKFLOW` relationship
- `ingest_tool_registry` — currently creates tokens with `token_type: "tool"` → should create `source` tokens + `IS_A_TOOL` relationship
- `ingest_text_chunks` — currently creates `"system_referential"` → should create `"source"` tokens (this is the closest thing to the spec's source type already)
- `protocol.rs` Phase 2 — currently creates `"referential"` / `"concept"` → these match spec and need no change (but token IDs need turn-number prefixing)

**Key constraints from stakeholder:**
- **No backward compatibility required** — store data can be wiped. This trivializes the schema change: drop existing LanceDB tables, recreate with the new type system. No migration code needed.
- **All tokens prefixed with turn number** — every token ID carries `T{turn}_` so duplicates are structurally impossible. Currently only auto-chunked tokens do this (`USR_T1_001`); needs to extend to AI-created tokens (concept, referential) and system-created tokens (tools, workflows, skills).

**Gap severity:** LOW-MEDIUM (much simpler than initially assessed — storage schema barely changes)  
**Implementation effort:** ~1-2 weeks  
**Impact:** Primarily a refactoring task across 5 Rust modules + 1 schema enum. The storage layer (LanceDB tables, InMemorySquireStore, SquireStore trait) barely changes — just trimming the valid `token_type` values to 3 + a new `"source"` variant.

---

### GAP-B: `rdf()` Primitive (§3)

**Spec says:** Two separate retrieval primitives:
```
explore(text, type, num_results) -> seed tokens, via vector search
rdf(token, hops)                  -> related tokens, via triplet walk
```
`rdf` is a mechanical, non-judgmental walk of triplet edges. The AI decides which edges matter.

**Current implementation:** Single `explore(resource_type, query, num_hops, max_results)` that bundles both vector search AND graph traversal. No separate `rdf()` tool.

**Gap severity:** MEDIUM  
**Implementation effort:** ~3-5 days  
**Impact:** New tool definition, wire through SquireExploreTool/SquireStore trait, add to both backends. Existing `traverse_relationships` function can be reused.

---

### GAP-C: Batch Composition Syntax (§3)

**Spec says:** Narrow shell-syntax subset for composing retrieval calls in a single round trip:
- `|` (pipe) — output tokens of left call become inputs to right call
- `&` / `;` / newline — independent calls bundled together
- Explicitly out of scope: `&&`, `||`, redirection, subshells, cmd substitution

**Current implementation:** No batch composition. Each `explore()`/`token_to_detail()` call is independent. The model makes serial round trips.

**Gap severity:** MEDIUM (nice-to-have for MVP, important for real use)  
**Implementation effort:** ~1-2 weeks  
**Impact:** New batch-parsing layer. Changes how the adapter receives the response (would need to handle batched results). Could be layered on top of existing infrastructure.

---

### GAP-D: Context Assembly — Short List / Long List with Budget Algorithm (§4)

**Spec says:**
- Two lists assembled every turn: **short** (ID + short desc, needs `token_to_detail()` to expand) and **long** (full content inlined)
- Budget-based algorithm with LONG_LIST_BUDGET, demotion to short (never dropped)
- Sources: Squire prefetch + formatter carry-forward
- Prefetch: one search vs workflows/tools (top 3), one vs memory (top 10, distance-floored)

**Current implementation:** Has `prefetched_tokens` and `preserved_tokens` lists plus an `expanded_tokens` concept in the system prompt
- Builds prefetch via `build_turn_input` calling `explore_memory` with hardcoded config
- Preserves tokens from previous turn via `SquireStore::preserved_tokens()`
- No budget algorithm — prefetch is simpler (top-N results from explore)
- No formal "demotion" concept

**Gap severity:** MEDIUM  
**Implementation effort:** ~1 week  
**Impact:** Refactor `build_turn_input` to implement the budget algorithm. Add config for `LONG_LIST_BUDGET`. The existing prefetch and preserve data flow provides the inputs; only the assembly algorithm changes.

---

### GAP-E: Formatter Pass Architecture (§7)

**Spec says turn lifecycle:**
1. User input arrives → chunked
2. Squire prefetch (2 semantic searches)
3. Context assembly (short/long lists)
4. Main AI receives lists + user input, explores manually (bounded), generates response
5. Response returned to user
6. **Formatter pass (background)** — a second AI (chosen for speed/structured-output reliability) receives the (user request, AI response) pair. It:
   - Mints new referential tokens highlighting important pieces
   - Mints relationship tokens (triplets) describing relationships
   - May mint concept tokens for organizational/graph-building
   - Curates next turn's preserved short-list and long-list candidates
   - Operates **statelessly per turn** (sees only current turn's request/response, not existing graph)
7. DB write — new tokens and relationships persisted; hitcount updates; turn counter increments

**Current implementation:** Two-phase protocol (Phase 1 + Phase 2):
- Phase 1: Main AI generates response with bookmarks, new_tokens, relationships, preserve
- Phase 2: Same/optionally-different model receives Phase 1 response + user request, generates referential tokens + concept tokens + relationships using `system-prompt-phase2.md`
- Phase 2 happens inline (blocks the turn close), not as background
- Phase 2 uses the same provider infrastructure as Phase 1
- Phase 2 generates bookmark-protocol-format output, which `finalize_turn` parses

**Gap severity:** HIGH (fundamental architecture change)  
**Implementation effort:** ~2-4 weeks  
**Impact:** Major restructuring of turn lifecycle. The current Phase 2 would be replaced by a separate, asynchronous formatter call with a different system prompt and no tools. The formatter operates differently (stateless, per-turn only, mints tokens vs parses bookmarks).

---

### GAP-F: Tools as Tokens — Full Cutover (§6)

**Spec says:** ALL tools (MCP, builtin, skills, shell, file, DB, sub-invocation) are ingested as tokens. No separate tool-calling interface for the AI. Tools discovered exclusively via `explore()`/`rdf()`, expanded via `token_to_detail()`. Asking for clarification is itself a discoverable builtin tool.

**Current implementation:** 
- `tool-token-ingestion` node: writes tools into SquireStore via `ingest_tool_registry`
- `tool-token-registry` node (planned): aims to unify built-in/MCP registration onto SquireStore schema, route tool discovery through store's semantic path instead of live-registry substring filter
- `SquireExploreTool::execute` still has a live-registry substring-fallback for `resource_type="tool"|"tool_skill"` — tools NOT found via this path
- `SquireInvokeTool` routes through real registry, falls back to store endpoint for MCP tools
- `token-detail-endpoint` node added real MCP dispatch for stored endpoints

**Gap severity:** MEDIUM (tool-token-registry node partially addresses this)  
**Implementation effort:** ~1-2 weeks remaining  
**Impact:** `tool-token-registry` is already planned and designed. Need to complete it, then verify the live-registry substring shortcut is fully removed for tool/tool_skill.

---

### GAP-G: Batch Retrieval Cap (§3 — Bounding)

**Spec says:** The AI is instructed (via workflow) to manually construct necessary context. To prevent unbounded retrieval loops, the number of batches issuable per turn is capped (implementation parameter, e.g. 2-3).

**Current implementation:** No batch cap. The model can call `explore()`/`token_to_detail()`/`invoke()` any number of times per turn (limited only by the LLM's token budget and tool-call iteration limits in streaming_cmd.rs).

**Gap severity:** LOW  
**Implementation effort:** ~2-3 days  
**Impact:** Add batch counter to SquireContextAdapter, reject/prohibit further explore calls after cap reached. Simple instrumentation.

---

### GAP-H: Workflow Governance (§8)

**Spec says:** Workflows are ordinary tokens (typically source tokens with a relationship asserting "IS-A workflow"). They govern: style, process, security/validation, persona/domain adaptation, meta-orchestration, sandboxed procedures. Squire's only role is to make workflow tokens discoverable. Multi-agent behavior is entirely a convention in workflow content.

**Current implementation:** Workflow tokens exist and are discoverable via `explore(resource_type="workflow", ...)`. The system prompt (§WORKFLOWS) tells the AI to check workflow short_desc and use token_to_detail if matching. But there is no formal governance framework — no workflow lifecycle, no meta-orchestration, no sandboxed procedures.

**Gap severity:** LOW-MEDIUM  
**Implementation effort:** ~1-2 weeks  
**Impact:** This is fundamentally content work (writing workflow documents), not code. The current discovery infrastructure already supports it. Need to author actual workflow content and possibly a workflow meta-workflow for arbitration. Some code changes may be needed for sandboxed procedures.

---

### GAP-I: Compaction `/compact` (§9)

**Spec says:** Detailed procedure:
1. `/compact` invoked → **freeze** live session DB
2. Copy frozen DB to ramdisk working copy
3. `git init` + commit (baseline)
4. AI runs compaction script against working copy (query, update, delete, merge)
5. Validation gate
6. Sync working copy back over live DB
7. Destroy ramdisk
8. Unfreeze → resume

**Current implementation:** NOT IMPLEMENTED (and spec explicitly says "Deferred to future detailed design").

**Gap severity:** LOW (deferred by spec explicitly)  
**Implementation effort:** ~2-3 weeks  
**Impact:** New `/compact` command/endpoint. Freeze/unfreeze coordination. Ramdisk management. Git-based safety. This is explicitly post-MVP per the spec.

---

### GAP-J: Source Token Ingestion / Chunking (§2, §7 step 2)

**Spec says:** Raw text (files, user requests, AI responses, tool output, URLs) is ingested as **source tokens**, chunked into addressable segments. Referential tokens point into these segments.

**Current implementation:** User input is chunked into `system_referential` tokens (`USR_TN_NNN`). AI response spans become `referential` tokens. No general "source" token type. No general raw-text ingestion pipeline.

**Gap severity:** MEDIUM (depends on whether source tokens are needed for MVP)  
**Implementation effort:** ~1-2 weeks  
**Impact:** New token type, storage schema update (both backends), chunking pipeline. The existing `chunk_user_input` function provides a foundation.

---

### GAP-K: Referential Token Range Resolution (§2, §3)

**Spec says:** Referential tokens point into a range within a SINGLE source token (cannot cross sources). Uses `start..end` syntax. No text duplication.

**Current implementation:** Referential tokens point to LanceDB chunk IDs. No range-based pointing. The `ranges` field exists on `StoredToken` and `TokenDetail` but is used for range specification resolution (e.g. chunk_0→chunk_1 from Phase 2).

**Gap severity:** MEDIUM  
**Implementation effort:** ~1 week  
**Impact:** Range resolution logic already exists in `resolve_ranges`. Would need to be adapted to work with source token ranges instead of bookmark spans.

---

## Summary of Effort

| Gap | Severity | Effort | Dependencies |
|-----|----------|--------|-------------|
| A. Roles vs Types | LOW-MED | 1-2w | Schema is trivial; work is refactoring 5 modules |
| B. `rdf()` Primitive | MEDIUM | 3-5d | None (reuses existing graph traversal) |
| C. Batch Composition Syntax | MEDIUM | 1-2w | Requires batch-parsing infrastructure |
| D. Short/Long List Budget | MEDIUM | 1w | Current prefetch/preserve provides inputs |
| E. Formatter Pass Arch | HIGH | 2-4w | Requires separate model call infrastructure |
| F. Tools-as-Tokens Cutover | MEDIUM | 1-2w | `tool-token-registry` already planned |
| G. Batch Retrieval Cap | LOW | 2-3d | Simple counter |
| H. Workflow Governance | LOW-MED | 1-2w | Mostly content work |
| I. Compaction | LOW | 2-3w | Deferred by spec explicitly |
| J. Source Token Ingestion | MEDIUM | 1-2w | Depends on token model decision |
| K. Referential Range Pointing | MEDIUM | 1w | Depends on token model decision |

## Critical Architectural Decision Required

The highest-impact decision is whether to **pivot to the new spec's architecture** (particularly the token model redesign + formatter pass) or to **continue with the current v2 architecture** and treat the new spec as an aspirational reference. These two paths diverge significantly and the choice affects every subsequent implementation decision.
