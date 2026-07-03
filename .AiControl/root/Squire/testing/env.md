# Env

- Parent node: root/Squire
- Node path: root/Squire/testing
- Objective: a consolidation/hardening testing-and-QA pass over the entire Squire
  context-mode pipeline (adapter, storage, retrieval, streaming) built up incrementally across
  all prior implementation nodes, rather than a new feature or gap closure. Inventory existing
  coverage, identify concrete real gaps (parity drift between the two `SquireStore` backends,
  missing negative-path coverage, happy-path-only e2e specs, untested frontend surfaces), and
  close them with tests following the epic's already-established conventions.
- Scope: unit tests (both `InMemorySquireStore` and `LanceDbSquireStore` where applicable),
  headless integration-harness tests (`src-tauri/examples/*_e2e.rs` style) for backend-only
  paths, and WDIO+tauri-driver e2e specs (`e2e/specs/*.test.ts` style) for genuine frontend
  surface — strictly following the tiering convention already documented in `../env.md`'s
  "Verification-methodology convention" entry. Also includes a targeted audit of
  `e2e/specs/*.test.ts` for the non-polling-assertion flakiness pattern `session-ux-polish`
  found once and explicitly left unaudited elsewhere.
- Non-goal: implementing new product functionality; re-opening the nested-`§!`-citation
  residual `hit-count-fidelity`/`memory-alias-fix` deliberately left as a permanent
  simplification; rewriting already-adequate tests for style; fixing the two known
  pre-existing frontend issues (`tools-panel.tsx` type errors, two intermittently-failing
  frontend tests) that predate this epic.
- Depends on: every completed prior node in the epic (see `../state.md`'s Child Nodes list and
  `../handoff.md` for the full history) — this phase reads their cumulative test suites as its
  starting inventory rather than building coverage from zero. Most directly relevant:
  `squire-storage` (the two-backend parity convention and six-table LanceDB layout),
  `retrieval-fidelity`/`hit-count-fidelity` (scoring-event test precedents),
  `ask-user-loop`/`session-creation-ux`/`session-ux-polish` (the e2e-spec precedents and the
  one already-found flakiness pattern), `tool-token-ingestion`/`user-input-chunking`/
  `raw-partition-storage` (the headless-integration-harness precedent for backend-only paths).
- Status: not started, created 2026-07-03.

## Durable facts inherited from the parent (see `../env.md` for full detail — summarized here for quick reference)

- Two `SquireStore` implementations (`InMemorySquireStore`, `LanceDbSquireStore`) must be kept
  at test parity — every trait method needs real unit tests against **both** backends.
- LanceDB backend has six tables as of `token-detail-endpoint`: `squire_tokens` (incl. vector
  embedding + nullable JSON `endpoint` column), `squire_relationships`, `squire_turns`,
  `squire_preserve_lists`, `squire_compliance_failures`, `squire_raw_partition`.
- Verification-methodology convention: backend-only changes get unit tests in both backends
  plus, if useful, a headless `cargo run --example ..._e2e` harness — not a WDIO/GUI spec
  (confirm via repo-wide frontend grep first, every time). Changes touching real frontend
  surface get a real WDIO+tauri-driver e2e spec.
- Known recurring e2e flakiness pattern: a non-polling `expect(...).toBe(...)` immediately
  after a UI action can race a React re-render; fix with `browser.waitUntil(...)`.
  `session-ux-polish` found and fixed one instance in `session-creation-ux.test.ts`; explicitly
  left the rest of `e2e/specs/` unaudited.
- Known pre-existing, not-this-epic's-fault issues: `tools-panel.tsx` references a
  nonexistent `AppConfig.disabledTools` field plus an invalid `title` prop on a lucide icon (2
  spots); `chat-input.test.tsx` and `chat-blocks.test.tsx` each have one intermittently-failing
  test, confirmed pre-existing via stash-and-rerun.
- Build prerequisite: `protoc` required for cold builds only (`winget install --id
  Google.Protobuf -e`, or set `PROTOC` directly). `arrow-array`/`arrow-schema` must stay
  pinned to `"53.2"` in `src-tauri/Cargo.toml`.
- A free-tier OpenAI-compatible test LLM provider (OpenCode Zen, `deepseek-v4-flash-free`) is
  configured in the real app config location (`%APPDATA%\com.squirecli.app\config.toml` on
  Windows) for real end-to-end verification — do not paste the raw key into any committed doc.

## Useful commands

- `cd src-tauri && cargo build && cargo build --bins && cargo build --examples && cargo test --lib`
- From repo root: `npx tsc --noEmit -p tsconfig.app.json`
- From repo root: `npm test -- --run`
- `cd src-tauri && cargo run --example <name>_e2e` for any existing headless integration
  harness (`ask_user_e2e`, `tool_token_ingestion_e2e`, `user_input_chunking_e2e`,
  `raw_partition_storage_e2e`).
- `npm run test:e2e:dev` for WDIO + tauri-driver e2e specs (requires `msedgedriver.exe` on
  `PATH`, matching the installed Edge/WebView2 version).

See `../env.md` for the full, unabridged version of every fact summarized above.
