# Decisions

## LanceDB dependency: real crate, not deferred (Q4 compliance)

Added `lancedb = "0.14"` plus `arrow-array`/`arrow-schema` pinned to `"53.2"` to `src-tauri/Cargo.toml`. Q4 explicitly rejects a "SQLite-only stopgap," so this node adds the real dependency rather than simulating LanceDB semantics on top of the existing `rusqlite` store. Spiked the addition first (build-only, no code) before committing to the design, since a dependency this heavy (Arrow + DataFusion + Lance transitively) was the biggest unknown in this node — see the `protoc` finding below, which would have blocked the whole node if unresolvable.

**Arrow version pin rationale**: `lancedb 0.14.1`'s own public API (`Table`, `Query`, `Connection`) is built against `arrow-array`/`arrow-schema` `53.2`, but `lance` (lancedb's internal storage engine, pulled in transitively) depends on `arrow` `52.2.0`. Both versions coexist in the dependency graph (Cargo allows this), but code calling `lancedb`'s public methods must use the `53.2` line — using `52.x` types produces "no method found... there are multiple different versions of crate `arrow_array`" errors even though the method exists, because it exists on the *other* version's trait.

## `protoc` build prerequisite — accepted, documented, not worked around

`lance-encoding` (transitive via `lance` via `lancedb`) runs a `prost-build` build script that requires the system `protoc` binary; it is not vendored or optional in this version. This machine did not have `protoc` installed. Options considered:
1. **Install `protoc` via `winget install Google.Protobuf`** (chosen) — one-time, ~5 minute dev-machine setup step, standard practice for `prost`/gRPC-adjacent Rust dependencies, well-precedented in the wider Rust ecosystem.
2. Pin an older `lancedb`/`lance` version without a `protoc` requirement — rejected: no realistic older version avoids `lance-encoding`'s proto usage; would mean losing current functionality/fixes for no real gain.
3. Vendor a `protoc` binary in-repo or fetch one via build.rs — rejected as overengineering for a dev-machine-only, one-time cost; `protoc-bin-vendored` crates exist but add their own maintenance surface for a problem `winget`/`apt`/`brew` already solve cleanly per-platform.

Verified end to end: cold build with `PROTOC` pointed at the winget-installed binary compiles cleanly (~7 min, dependency compilation only); once compiled, `protoc` is not required again for warm/incremental builds or `cargo test`. Documented in `env.md` as a new durable fact, since this is a new build prerequisite for the whole repo, not just this node — anyone doing a clean checkout needs it.

## Embedding function: deterministic hash-based placeholder, not a real model

`explore_memory`'s vector-search path needed *something* to embed text into the fixed-size float vector LanceDB's `nearest_to`/cosine-similarity search operates on. No embedding-model provider (local or remote) exists anywhere in this codebase today — building one is a materially different, larger feature (model selection, API integration, caching) that is out of this node's scope (`env.md` scope is storage, not embedding infrastructure).

Decision: implement a deterministic, dependency-free hash-based bag-of-words embedding (`embed_text` in `squire_lancedb.rs`, 64 dimensions, FNV-1a hashing of whitespace-split lowercase tokens into buckets, L2-normalized). This is not semantically meaningful the way a trained embedding model's output is, but it:
- exercises the *real* LanceDB vector-search code path (schema with a `FixedSizeList<Float32>` column, cosine similarity ranking, `max_results` truncation) rather than stubbing it out,
- is swappable later by changing only `embed_text`'s body — the `SquireStore` trait, its callers (`SquireExploreTool`, `SquireContextAdapter::build_turn_input`), and the table schema (as long as dimensionality is preserved or the column is migrated) are unaffected,
- is tested (`explore_memory_ranks_closer_match_higher`) to confirm it produces sane relative rankings for exact-vs-unrelated content, with a small substring-match score boost layered on top so exact name/keyword hits aren't lost to hash-collision noise in such a small embedding space.

This is flagged as a known limitation, not silently absorbed as if it were a finished embedding story — a future node (or this one revisited) should replace `embed_text` once a real embedding-model integration exists elsewhere in the codebase.

## Storage layout: one LanceDB directory, four tables

`LanceDbSquireStore::open(dir)` creates/opens a single LanceDB connection at `dir` containing four tables:
- `squire_tokens` — the structured partition: `token_id`, `token_type`, `short_desc`, `full_desc`, `creation_turn`, `embedding` (64-dim float32 vector). This is where `token_exists`/`upsert_token`/`token_detail`/`explore_memory` operate.
- `squire_relationships` — the triplet store (raw partition): `subject`, `predicate`, `object`, append-only.
- `squire_turns` — per-session turn counter: `session_id`, `turn`, replace-on-write (delete then insert) since LanceDB has no in-place update-by-key primitive used here.
- `squire_preserve_lists` — per-session preserve-list membership: `session_id`, `token_id` rows, replaced wholesale on every `set_preserve_list` call (matches spec's "preserve list applies only to the immediate next turn" semantics from Q7 — old rows for a session are deleted before the new list is written, there is no accumulation).

Chose one connection/directory over one-directory-per-table for operational simplicity (`setup_cmd.rs` only needs to know one path) — LanceDB's connection model is directory-based (each table is its own `.lance` dataset subdirectory under the connection URI), so this doesn't cost anything relative to separate connections.

Delete-then-insert is used for all "replace" semantics (`upsert_token`'s conflict path, `set_preserve_list`, `increment_turn`) because LanceDB 0.14's `Table` API in this crate version doesn't expose a single-statement upsert-by-key; `Table::update` exists but is column-expression based, not a natural fit for replacing a whole struct's worth of columns including the embedding vector. A `write_lock: Mutex<()>` on `LanceDbSquireStore` serializes these read-modify-write sequences per-store-instance so concurrent turns don't race a delete against another turn's insert of the same key.

## `SquireStore` trait contract: unchanged

No changes to the `SquireStore` trait signature itself (defined in `agent/squire.rs`, owned by squire-adapter) — `LanceDbSquireStore` implements it as-is. This confirms squire-adapter's design intent ("no other code needs to change when [squire-storage] lands — `AppState.squire_store` just gets constructed with the real impl") held exactly: the only call-site change was in `setup_cmd.rs`'s construction line, plus the `SquireInvokeTool` field addition below (which was an explicit, separately-flagged pointer in squire-adapter/decisions.md, not part of the trait).

## `SquireInvokeTool` redirect: additive fallback, not a hard cutover

squire-adapter/decisions.md's exact pointer: "the `invoke` tool's lookup currently uses the plain `ToolRegistry` as a stand-in for a tool-token store; that's the one spot squire-storage should redirect... lookup should move from `tool_registry.get(token_id)` to `store.token_detail(token_id)` resolving to an MCP endpoint."

On investigation, a full cutover is not implementable yet: `TokenDetail` (the store's return type) only carries `short_desc`/`full_desc` — there is no field describing an invocable endpoint (MCP server + remote tool name), and more fundamentally, **nothing in the current system ever writes tool tokens into `SquireStore`** — `explore(resource_type="tool"/"tool_skill")` reads live from `ToolRegistry::definitions()`, not from the store (see `SquireExploreTool::execute`, unchanged by this node). Redirecting `invoke`'s *primary* lookup to the store as originally worded would break every currently-working invoke path, since the store would have zero tool tokens to find.

Decision: added `store: Arc<dyn SquireStore>` to `SquireInvokeTool` and made the store a **fallback**, not the primary lookup — `tool_registry.get(token_id)` is tried first (still the only place real invocable tools live); if absent, `store.token_detail(token_id)` is tried and, if found, returns a distinguishing "recorded in Squire storage but has no invocable endpoint bound yet" error rather than the generic "non-invocable token" error. This satisfies the literal instruction ("redirect... to store.token_detail") in the one narrow way that's safe today, is forward-compatible with a real tool-token-ingestion feature landing later (that feature would only need to add ingestion + an endpoint field to `TokenDetail`/a new struct — this call site wouldn't need to change again), and is covered by a new test (`invoke_tool_falls_back_to_store_token_detail_when_not_in_registry`) alongside the existing registry-hit and unknown-token tests.

Real tool-token ingestion (turning MCP/local tool discovery into persisted, invocable `SquireStore` rows) is explicitly out of scope for this node — flagged as a non-goal in `env.md` and not silently absorbed. If a future node wants full production parity here, it needs: (a) a discovery-to-ingestion write path (likely in `streaming_cmd.rs` alongside MCP tool discovery), and (b) an endpoint-carrying extension to the token schema, neither of which existed before this node and neither of which this node's stated scope (storage layer) covers.
