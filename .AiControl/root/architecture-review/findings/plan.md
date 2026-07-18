# Action Plan: Squire Architecture Gap Closure

**Date:** 2026-07-11  
**Based on:** Gap analysis in `findings/gap-analysis-vs-spec.md`

---

## Key Fact: Gap A Is Much Smaller Than Initially Assessed

The spec's 4 token types (source, referential, concept, relationship) vs current 10 values is **a refactoring task, not a schema redesign**. After stakeholder clarification:

- 3 of the 10 current values already match spec types: `"concept"` → concept, `"referential"` → referential, `"system_referential"` → source (just needs renaming)
- 7 values are **hardcoded roles** being used as types: `"tool"`, `"skill"`, `"workflow"`, `"decision"`, `"assumption"`, `"todo"`, `"user"`. These should be source/concept tokens + relationship triplets (e.g. `IS_A_WORKFLOW`, `HAS_ASSUMPTION`)
- The `StoredToken` schema already has a plain-string `token_type` field and a separate relationship store; the change is in **how modules assign type values**, not in the storage layer itself

Combined with the two stakeholder constraints (wipe-on-deploy + turn-number prefix), the gap closure is straightforward — no architectural pivot needed between "options." The question is just sequencing.

---

## Key Constraints

- **No backward compatibility required** — store data can be wiped. No migration code, no schema versioning.
- **All tokens prefixed with turn number** — every token ID carries `T{turn}_`, making duplicates structurally impossible.
- **Squire recognizes 4 token types** — source, referential, concept, relationship. Everything else (workflow, skill, tool, decision, assumption, todo) is a ROLE expressed via relationship triplets, not a type field.

---

## Phase 1: Core Refactoring (Estimated: 1-2 weeks)

The foundational change: straighten out the token model so Squire's 4 types are the only types.

### P1.1 — Fix Token Types: Roles Are Not Types (Gap A)

- **Effort:** 1 week
- **What:** Change every non-compliant `token_type` assignment to use source/concept + relationship triplets
- **How:**

  | Module | Current hardcoded type | Should become |
  |--------|----------------------|---------------|
  | `ingestion.rs` line 234 | `"system_referential"` | `"source"` |
  | `ingestion.rs` line 97 | `"tool"` | `"source"` + `IS_A_TOOL` relationship |
  | `squire_skills.rs` line 143 | `"skill"` | `"source"` + `IS_A_SKILL` relationship |
  | `squire_workflows.rs` line 163 | `"workflow"` | `"source"` + `IS_A_WORKFLOW` relationship |
  | `decision_tree.rs` lines 355, 411 | `"decision"` | `"concept"` + `HAS_DECISION` relationship |
  | `decision_tree.rs` line 477 | `"assumption"` | `"concept"` + `HAS_ASSUMPTION` relationship |
  | `todo_tree.rs` line 629 | `"todo"` | `"concept"` + `IS_A_TODO` relationship |
  | `protocol.rs` line 708 | `"user"` | `"source"` |
  | `protocol.rs` lines 770, 778 | `"referential"`, `"concept"` | No change (already correct) |

- **Turn-number prefixing:** Add `T{turn}_` prefix to every token created outside the auto-chunking path (tools, workflows, skills, concepts created by AI via bookmark protocol)
- **Tests:** Update hardcoded `token_type` assertions in existing test files (~40 locations in `squire_test.rs`). Most are trivial renames (`"workflow"` → `"source"`, `"tool"` → `"source"`, etc.)
- **Store wipe:** Drop existing LanceDB tables before tests (no migration needed)
- **Depends on:** Nothing

---

## Phase 1b: Turn-Number Prefixing (Integrated with P1.1)

- **Effort:** 2-3 days (done alongside P1.1)
- **What:** Ensure every token ID carries `T{turn}_`
- **Currently correct:** `ingest_text_chunks` → `USR_T{turn}_{NNN}_{session}`, `RESP_T{turn}_{NNN}_{session}`
- **Needs fixing:**
  - `ingest_tool_registry` — tools get the registry name verbatim (e.g. `web_fetch`). Should become `T0_web_fetch` (tools are turn-0, pre-session)
  - AI-created tokens via bookmark protocol — currently any ID the model picks (e.g. `CONCEPT_UserGoal`). Adapter must prefix with current turn before upsert
  - Workflow/skill ingestion — same pattern as tools (turn-0 prefix)
  - `tell-phase2-tokens.rs` and `DecisionTreeTool` / `TodoTreeTool` — must prefix with turn number at creation time
- **Enforcement:** Validate token ID format at the store boundary (SquireStore trait) or in the adapter layer

---

## Phase 2: New Primitives (Estimated: 1-2 weeks)

### P2.1 — Add `rdf()` Primitive (Gap B)

- **Effort:** 3-5 days
- **What:** Add `rdf(token_id, hops)` as a fourth built-in tool alongside explore/token_to_detail/invoke
- **How:** New `SquireRdfTool` struct in `tools.rs`. Reuses existing `traverse_relationships()` function from `squire-store`. Wire through the engine (register in Squire mode tool list) and `built_in_tool_definitions()`
- **Tests:** Unit tests in both InMemorySquireStore and LanceDbSquireStore
- **Depends on:** P1.1 (foundation for clean types)

### P2.2 — Add Batch Retrieval Cap (Gap G)

- **Effort:** 2-3 days
- **What:** Implement per-turn batch counter (configurable, default 3)
- **How:** Add counter to adapter state. Increment on each `explore()`/`rdf()`/`token_to_detail()` call. Return tool error when cap reached. Surface via system prompt.
- **Depends on:** Nothing

---

## Phase 3: Context Assembly (Estimated: 1-2 weeks)

### P3.1 — Short/Long List Budget Algorithm (Gap D)

- **Effort:** 1 week
- **What:** Refactor `build_turn_input` to implement budget-based assembly per spec §4
- **How:** LONG_LIST_BUDGET config param → allocate long list first → demote overflow to short → update system prompt
- **Depends on:** P1.1 (clean type system), ideally after Phase 2

### P3.2 — Source Token Lifecycle: User Input + Response Chunking (Gap J)

- **Effort:** 1 week
- **What:** Rename `"system_referential"` to `"source"`, make chunking the primary turn-0 step
- **Note:** This already mostly works — `ingest_text_chunks` already creates turn-prefixed, chunked tokens. The gap is just the type rename and making this the canonical "source ingestion" path that the formatter operates on.
- **Depends on:** P1.1 (rename), P3.1 (context assembly feeds from source tokens)

---

## Phase 4: Formatter Pass (Estimated: 2-3 weeks)

### P4.1 — Replace Phase 2 with Formatter (Gap E)

- **Effort:** 2-3 weeks
- **What:** Replace the current inline Phase 2 (same model, bookmark protocol, blocking) with a separate formatter model call (cheaper model, stateless per-turn, no tools, structured output)
- **Key differences from current Phase 2:**
  - Formatter runs asynchronously (doesn't block user response)
  - Operates statelessly (sees only current turn's user request + AI response, not existing graph)
  - Curates preserved short/long list candidates for next turn (replaces current `preserve`-list mechanism)
  - Uses the 4-type system exclusively (source → referential/concept + relationships)
- **Depends on:** Phase 1-3 (new type system must be in place first)

---

## Phase 5: Governance & Batch Syntax (Estimated: 2-3 weeks)

### P5.1 — Workflow Governance Content (Gap H)

- **Effort:** 1-2 weeks
- **What:** Author workflow documents as source tokens with IS_A_WORKFLOW relationships. Framework for meta-orchestration.
- **Depends on:** P1.1 (workflow-as-role infrastructure)

### P5.2 — Batch Composition Syntax (Gap C)

- **Effort:** 1-2 weeks
- **What:** Implement `|` pipe and `&`/`;` batch operators for retrieval calls
- **Depends on:** P4.1 (formatter pass changes how responses are parsed)

---

## Phase 6: Deferred (per spec)

### P6.1 — Compaction `/compact` (Gap I)

- **Effort:** 2-3 weeks  
- **When:** Post-MVP, per spec §9's explicit deferral

---

## Summary Timeline

| Phase | Items | Est. Effort |
|-------|-------|-------------|
| **1** | Token type cleanup + turn-number prefixing | **1-2 weeks** |
| **2** | `rdf()` + batch cap | **1-2 weeks** |
| **3** | Short/Long list budget + source token lifecycle | **1-2 weeks** |
| **4** | Formatter pass replacement | **2-3 weeks** |
| **5** | Workflow governance + batch syntax | **2-3 weeks** |
| **6** | Compaction (deferred) | — |

**Total implementation:** ~7-12 weeks for full spec compliance  
**MVP (Phases 1-3):** ~3-6 weeks for the core architectural cleanup + `rdf()` + budget assembly  

---

## Immediate Next Steps

1. **Start P1.1 immediately** — the type cleanup is the foundation everything else depends on, and it's well-understood refactoring work
2. **Start P1b (turn-number prefixing) concurrently** — integrated with P1.1 since many of the same files change
3. **Proceed to P2.1 (rdf())** once P1.1 tests pass — reuses existing graph traversal, minimal new code
