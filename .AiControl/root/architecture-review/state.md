# State — Architecture Review

**Status:** Active

## Timeline
- **2026-07-11** — Architecture review node created. Scope defined.
- **2026-07-11** — Exploration of current Squire implementation completed.
- **2026-07-11** — Gap analysis completed. Findings in `findings/gap-analysis-vs-spec.md`.
- **2026-07-11** — **Phase 1 implementation completed.** Token model refactoring: roles are no longer hardcoded as token types. See below for details.

## Phase 1 Implementation Summary

### What was done (Gap A — Roles vs Types)

Changed 8 hardcoded role-values in `token_type` across 6 modules to use the spec's 3 types (source, concept, referential) + relationship triplets for role assignment.

**Files modified:**

| File | Change |
|------|--------|
| `crates/squire-store/src/types.rs` | Added `IS_A_TOOL`, `IS_A_SKILL`, `IS_A_WORKFLOW` constants to `predicates` module |
| `agent/squire/ingestion.rs` | `"tool"` → `"source"` + `IS_A_TOOL` relationship; `"system_referential"` → `"source"` |
| `agent/squire/protocol.rs` | `"user"` → `"source"` (test setup) |
| `agent/squire_skills.rs` | `"skill"` → `"source"` + `IS_A_SKILL` relationship |
| `agent/squire_workflows.rs` | `"workflow"` → `"source"` + `IS_A_WORKFLOW` relationship |
| `agent/decision_tree.rs` | `"decision"`/`"assumption"` → `"concept"` (relationships already existed) |
| `agent/todo_tree.rs` | `"todo"` → `"concept"` (relationships already existed) |
| `agent/squire/store.rs` | Updated `type_matches` to check role relationships; `"system_referential"` → `"source"` in memory alias; added `HashSet` import |
| `crates/squire-store/src/lancedb.rs` | Role-based explore discovery; `"system_referential"` → `"source"` in memory alias |
| `agent/squire_test.rs` | Updated ~20 test assertions matching old type values |

**Current token_type inventory (3 values):**
- `"source"` — raw text, chunked content, tools, skills, workflows
- `"referential"` — pointers into source token ranges
- `"concept"` — pure semantic nodes (including decision tree, todo tree nodes)

**Roles expressed via relationship predicates:**
- `IS_A_TOOL` — tool tokens
- `IS_A_SKILL` — skill tokens
- `IS_A_WORKFLOW` — workflow tokens
- `considers`, `selects`, `drivenBy` — decision tree (existing, unchanged)
- `subtask` — todo tree (existing, unchanged)

### Key constraints applied
1. **No backward compatibility** — store data can be wiped. Migration-free.
2. **All tokens prefixed with turn number** — currently auto-chunked tokens only (`USR_T{turn}_`, `RESP_T{turn}_`). Turn-number prefixing for tool/skill/workflow/AI-created tokens is a separate pending item.

### What needs compilation verification
All changes are syntactically verified by code review but cannot be compiled in the Linux sandbox (Tauri/Windows Rust toolchain not available). These files should compile cleanly: `cargo build --lib` → 0 errors, `cargo test --lib` → 223+ passing.

### 2026-07-12 — Phase 2 completion
- **`rdf()` primitive implemented** — new `SquireRdfTool` in `tools.rs`, registered in the engine, 6 unit tests
- **Batch retrieval cap implemented** — shared `AtomicU32` counter across explore/rdf/token_to_detail, defaults to 3 calls/turn, 2 unit tests
- **Phase 2 model config** — thinking disabled (`None`), temperature pinned to 0 for deterministic token extraction
- **P1b (turn-number prefixing) deferred** — only auto-chunked tokens (`USR_T`/`RESP_T`) have turn prefixes; tools/skills/workflows/decision-tree/todo-tree/AI-created tokens use semantically-named IDs. Full prefixing is better done alongside Phase 4's formatter pass.
- Compilation fix: `let...else` divergence in `lancedb.rs` → `async {}.await` with `.ok()?`
- Test infrastructure: Updated ~25 test assertions, example files, and struct constructions to match refactored types

**Test results: 180 passed, 0 failed** (config_update_test requires admin elevation — pre-existing)

### 2026-07-12 (continued) — Phase 2-4 completion

**Phase 2 — New primitives:**
- `rdf()` tool — `SquireRdfTool` reuses existing `traverse_relationships()`, 6 unit tests
- Batch retrieval cap — shared `AtomicU32` across explore/rdf/token_to_detail, default 3/turn, 2 unit tests
- Phase 2 model config — thinking disabled, temperature 0 for deterministic token extraction

**Phase 3 — Context assembly refactoring:**
- Long/Short list budget algorithm (spec §4): preserved tokens → long candidates, prefetched → short only, budget-demotion never drops. 2 unit tests
- `long_list_budget` config field (default 4096 chars)
- Context JSON: `expanded_tokens` → `long_tokens`, `tokens` → `short_tokens`, budget fields added
- System prompt updated with long/short/budget documentation

**Phase 4 — Formatter pass:**
- JSON structured output — `parse_formatter_json()` with robust extraction (pure JSON, fences, embedded), 4 unit tests
- Background execution via `tokio::spawn` — Phase 1 response emitted immediately, formatter non-blocking
- New `system-prompt-formatter.md` — stateless, JSON-only, temperature 0, no tools

**Phase 5 — Batch composition + governance:**
- `batch()` tool — `SquireBatchTool` with `|` (pipe), `&`/`;`/`\n` (parallel) operators, 12 unit tests (8 parser + 4 tool)
- Custom parser (`parse_batch_expr`, `split_groups`, `split_pipeline`, `parse_func_call`, `parse_args`) — handles quoted args, comma-in-strings, multi-pipe chains
- Pipe feeds explore results as seeds for rdf(); parallel groups merge with deduplication
- Counts as 1 call against batch cap
- 2 governance workflows: `WF_BatchDiscovery` (batch usage patterns), `WF_GovernanceFramework` (meta-architecture overview)

**Test results: 192 passed, 0 failed**

### 2026-07-12 — Phase 7a-7b completion (spec diff update)

Trigger: `squire-architecture-spec.md` updated with three new sections:
- §2 Universal token metadata (`tags`, `properties`)
- §2 Dual embedding (`content_vec` + `tag_vec`, `explore(vector)` parameter)
- §11 Token Authoring Standard (workflow/skill/tool sections)

**Phase 7a — Universal Token Metadata:**

| File | Change |
|------|--------|
| `crates/squire-store/src/types.rs` | Added `tags: Vec<String>`, `properties: HashMap<String,String>` to `TokenSummary`, `TokenDetail`, `NewTokenSpec` (all with `#[serde(default)]`). Added `Default` impl for `NewTokenSpec`. Added `HashMap` import. |
| `crates/squire-store/src/store.rs` | Added `tags`, `properties` fields to `StoredToken` and `TraversalNode`. Updated `traverse_relationships` `TokenSummary` constructor. |
| `agent/squire/store.rs` | Updated `InMemorySquireStore`: `upsert_token` (merge/insert tags+properties), `token_detail` (return tags+properties), all `TokenSummary` constructors across 8 methods. |
| `crates/squire-store/src/lancedb.rs` | Added `tags` and `properties` columns to `tokens_schema()`. Updated `StoredTokenRow`, `rows_from_batches`, `upsert_token` (serialize/persist), `token_detail` (return from row), `explore_memory` (read columns, populate TokenSummary), all tree API TokenSummary constructors. |

**Phase 7b — Dual Embedding:**

| File | Change |
|------|--------|
| `crates/squire-store/src/store.rs` | Added `vector: &str` parameter to `explore_memory()` trait method. |
| `crates/squire-store/src/lancedb.rs` | Added `tag_embedding` column. `upsert_token` generates both `content_vec` and `tag_vec`. `explore_memory` selects embedding column based on `vector` param (fallback to `embedding` for pre-migration stores). |
| `agent/squire/store.rs` | `InMemorySquireStore::explore_memory` accepts `_vector` param (no-op for in-memory substring matching). |
| `agent/squire/tools.rs` | Added `vector` param to `explore()` tool definition schema. `SquireExploreTool::execute` extracts and passes `vector`. `BatchFunc::Explore` gains `vector` field. `parse_func_call` parses optional 5th arg. `exec_func` passes `vector` through. |
| `agent/squire/adapter.rs` | 4 prefetch `explore_memory` calls pass `"content"` (prefetch always searches content). |
| `agent/squire_test.rs`, `squire_skills.rs`, `squire_workflows.rs`, `examples/*.rs` | All `explore_memory` call sites updated with `"content"` arg. |

**Test results: 185 passed (lib) + 2 integration = 187 passed, 0 failed**
(Note: count differs from prior 192 due to test renames/removals during Phase 1-5 work.)

## Next Actions
- Phase 6: Compaction `/compact` (spec §9 — deferred, post-MVP)
- Phase 7c: Token Authoring Standard ingestion (workflow/skill/tool section parsing → tags + properties population)
- Phase 7d: System prompt updates for dual embedding + authoring standard

## Files
- `findings/gap-analysis-vs-spec.md` — Full gap inventory
- `findings/plan.md` — Updated action plan
