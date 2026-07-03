# Env

Backfilled 2026-07-03 from 17 completed child nodes' own `env.md`/`decisions.md` files, per the
updated `aicontrol-node` agent's write-back obligation (durable, epic-wide facts belong here,
not only in leaf nodes — this file was found empty despite the epic being substantially
complete, which is the failure mode that rule now exists to prevent).

## Stable facts — architecture

- **Two `SquireStore` implementations must be kept at parity.** `InMemorySquireStore` and
  `LanceDbSquireStore` (`src-tauri/src/storage/squire_lancedb.rs`) both implement the
  `SquireStore` trait (`src-tauri/src/agent/squire.rs`). Every trait method, and every new
  method added to it, needs an implementation and real unit tests against **both** backends —
  this has been the norm since `squire-storage` and is treated as non-negotiable epic-wide
  convention, not a per-node choice.
- **LanceDB backend has six tables** as of `raw-partition-storage`/`token-detail-endpoint`:
  `squire_tokens` (structured partition, incl. vector-search embedding column and, as of
  `token-detail-endpoint`, a nullable JSON `endpoint` column), `squire_relationships`
  (triplet store), `squire_turns` (per-session turn counter), `squire_preserve_lists`,
  `squire_compliance_failures` (Q6 diagnostics), `squire_raw_partition` (unmarked-output audit
  log). If you add a new column to `tokens_schema()`, check **every** `RecordBatch::try_new`
  call site against it, not just the one you're actively modifying —
  `LanceDbSquireStore::record_hit` builds its own `RecordBatch` independently of
  `upsert_token`'s and silently became a no-op for 7 tests when `token-detail-endpoint` added
  a column and missed updating it.
- **Vector search now uses a real local embedding model.** CORRECTED 2026-07-03 (by
  `squire-observability`) — this bullet previously said vector search used "a deterministic
  hash-based embedding, not a real embedding model," which was accurate at the time it was
  written but is now stale. `tool-token-registry` landed the keystone swap: `embed_text`
  (`src-tauri/src/storage/embedding.rs`) now defaults to `fastembed`'s `BGESmallENV15` model
  (384-dim, ONNX, CPU-only, encoder-only — no generative LLM), logging `"Squire embedding:
  initialized fastembed BGESmallENV15 (384-dim) for semantic search"` on success. The original
  toy bag-of-words hash still exists as a fallback for offline/model-init-failure, always
  producing the same `EMBED_DIM`-wide vector so the LanceDB schema never varies by path — it is
  no longer the default. Any node instrumenting or reasoning about retrieval quality must check
  which path produced a given score (real-model vs. fallback), since a word-overlap hit looks
  identical under both but only the real model can match on meaning with zero shared words
  (see `tool-token-registry/decisions.md`'s "make html" worked example).
- **The Q5 strict tool boundary**: Squire mode exposes exactly three built-in tools —
  `explore(resource_type, query, num_hops, max_results)`, `token_to_detail(token_id,
  detail_level)`, `invoke(token_id, params)`. All other capabilities are discovered via
  `explore(resource_type="tool_skill", ...)` and dispatched through `invoke()` — the model
  never calls external services directly.
- **`context_mode` (legacy|squire) is immutable by construction**, fixed at conversation
  creation and never changed afterward (`session-mode`). Do not add a "change mode later"
  capability — every UX node in this epic (`session-creation-ux`, `session-ux-polish`)
  deliberately preserved this guarantee rather than working around it.
- **Squire's `§!`/`§^` sigils are an AI-output-only convention.** `§!TokenID` (inline
  reference) and `§^...§^` (span marking for memory promotion) are never applied to parse the
  user's own raw input — `user-input-chunking` explicitly confirmed this and chose ordinary
  `system_referential`-typed tokens instead of sigil-parsing for the user's own message
  content.
- **`ToolRegistry` is rebuilt fresh every single turn** (`streaming_cmd.rs`, both Legacy and
  Squire mode) — local built-ins plus a fresh MCP `tools/list` discovery pass per enabled
  server. There is no separate connect/disconnect event system; this is the one real trigger
  point for anything that needs to react to "what tools currently exist."
- **MCP tool dispatch (`crate::mcp::call_tool`) is stateless and one-off per call** — every MCP
  tool invocation, live or not, spins up a fresh `StdioMcpClient`, connects, calls the one
  tool, and disconnects (`Drop` kills the child process). There is no persistent MCP session
  anywhere in this codebase. This turned a feature the epic initially worried would need new
  "session/connection lifecycle infrastructure" (`token-detail-endpoint`) into a tractable,
  proportionate one — check this fact before assuming any new MCP-adjacent feature needs new
  connection-management machinery.
- **Local/built-in tools are always registered unconditionally** (`ToolRegistry::new()`, no
  config/enablement gate) — a local tool can never be "ingested but not currently live." That
  state is only possible for MCP-sourced tools.
- **`effective_priority = accumulated_hits - (current_turn - creation_turn)`** (spec §3.3's
  exact formula, `retrieval-fidelity`) — used as a near-tie secondary sort key in `explore()`
  results, not the primary ranking signal (score remains primary).
- **Graph traversal in `explore()` is an undirected BFS** over `squire_relationships` up to
  `num_hops`, since relationship predicates have no enforced vocabulary and the spec frames
  traversal as reaching "connected memory," not strictly following edge direction
  (`retrieval-fidelity`).

## Real vs. fallback config location — a corrected assumption every node should know

**The real, running app reads config from `app.path().app_config_dir()`** (Tauri-managed,
`%APPDATA%\com.squirecli.app\config.toml` on Windows), set via `config::set_config_dir()` in
`setup_app_impl` (`src-tauri/src/commands/setup_cmd.rs`) before `load_config()` is ever called.

**`src-tauri/.squirecli/config.toml` (the git-tracked file in the repo) is a dead-weight
`dirs_fallback()` path** (`src-tauri/src/state/config.rs`) — it is only read if
`set_config_dir` is *never* called first, which does not happen in the real app or in the
example harnesses this epic built (`ask_user_e2e.rs` reads provider config from env vars
instead). Do not configure a test provider there expecting it to affect a real `tauri dev`/
built-binary run — it won't. (One session did this by mistake and had to revert it once this
was discovered — see `ask-user-loop/decisions.md`.)

## Test infrastructure

- **A free-tier, OpenAI-compatible test LLM provider (OpenCode Zen, model
  `deepseek-v4-flash-free`, endpoint `https://opencode.ai/zen/v1`) is configured in the real
  app config location above** on this development machine, for real end-to-end verification.
  Treat the credential as a shared, low-sensitivity test fixture (matching pre-existing
  practice already in a few git-tracked e2e specs) — don't paste it into new committed docs,
  but it's fine to reference that "a free-tier test provider is configured."
- **This free-tier model reliably participates in the pause/surface/collect/resume mechanics
  correctly but sometimes struggles to *close* a turn** (populate `content` rather than
  re-asking or looping) — observed as a small-model prompt-following characteristic across
  multiple sessions (`ask-user-loop`), not a defect in whatever mechanism is being tested.
  Design verification prompts accordingly; don't conclude a mechanism is broken just because
  this specific free model won't cooperate on the first few tries.
- **WDIO + `tauri-driver` e2e testing works in this environment** and drives the actual built
  app binary (`src-tauri/target/debug/squirecli.exe`) via a real WebDriver session — run via
  `npm run test:e2e:dev` (starts `tauri-driver` + wdio together). `tauri-driver` needs
  `msedgedriver.exe` on `PATH` (matching the installed Edge/WebView2 version) — see
  `ask-user-loop/decisions.md` for where a prior session found a cached copy when none was on
  `PATH` by default.
- **Verification-methodology convention, settled across many nodes**: a backend-only change
  with no new frontend surface gets unit tests in both `SquireStore` backends plus, if useful,
  a headless `cargo run --example ..._e2e` integration harness against a real `LanceDbSquireStore`
  — not a WDIO/GUI spec (confirmed via a repo-wide frontend grep before assuming this, every
  time). A change that adds or touches real frontend surface gets a real WDIO+tauri-driver e2e
  spec. Don't build GUI specs for pure backend plumbing "just in case" — it's a strictly weaker
  signal than a direct backend integration check for that kind of change.
- **A recurring e2e flakiness pattern**: a non-polling `expect(...).toBe(...)` assertion
  immediately after a UI action (e.g. a toggle click) can race the React re-render. Fix with a
  polling `browser.waitUntil(...)`, matching the idiom already used elsewhere in
  `e2e/specs/*.test.ts`. `session-ux-polish` found and fixed one instance; others may exist
  unaudited elsewhere in `e2e/specs/`.

## Build prerequisite

- **`protoc` (the protobuf compiler) is required for cold builds only** — a fresh checkout,
  `cargo clean`, or a `Cargo.lock` change touching `lance-encoding` (transitive via `lancedb`,
  added for the real LanceDB storage backend per Q4). Install via `winget install --id
  Google.Protobuf -e` (or the platform equivalent), then restart your shell, or set
  `PROTOC=<path-to-protoc.exe>` directly for the build invocation. Not needed again once
  compiled — warm/incremental builds and `cargo test` do not re-invoke it.
- **`arrow-array`/`arrow-schema` must stay pinned to `"53.2"`** in `src-tauri/Cargo.toml`,
  matching `lancedb 0.14`'s own public API — not the `52.x` line its internal `lance` storage
  engine pulls in transitively. Using the wrong line produces "no method found... multiple
  different versions of crate `arrow_array`" errors even though the method genuinely exists,
  because it exists on the other version's trait.

## Known pre-existing issues (not from this epic, not caused by it)

- `src/components/tools-panel.tsx` references `AppConfig.disabledTools`, which doesn't exist
  on the `AppConfig` type in `src/types/ipc.ts`, plus an invalid `title` prop passed to a
  lucide-react icon (2 spots).
- `chat-input.test.tsx` ("calls onSend on Enter without Shift") and `chat-blocks.test.tsx`
  ("renders thinking blocks collapsed by default") both fail intermittently at HEAD,
  independent of this epic — confirmed via stash-and-rerun against a clean baseline more than
  once.

See `decisions.md` for the settled architectural/scope judgment calls (as opposed to the
stable facts/constraints/commands captured here), and `handoff.md` for current operator-facing
epic status.
