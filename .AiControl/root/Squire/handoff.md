# Handoff — 2026-07-03, memory-alias-fix landed (epic backlog cleared to a single, permanently-deferred item)

Short, operator-oriented status for picking this back up from another machine after `git pull`.

## Where things stand

Squire epic (`root/Squire`) — building a swappable `ContextManagerAdapter` so sessions can use Legacy (full history replay) or Squire (curated protocol context):

| Node | Status |
|------|--------|
| `planning` | Completed — architecture locked, see `planning/implementation-readiness.md` |
| `adapter-core` | Completed — `ContextManagerAdapter` trait + `LegacyContextAdapter` in `src-tauri/src/agent/context_adapter.rs` |
| `session-mode` | Completed — `context_mode` (legacy\|squire) persisted end-to-end, immutable by construction |
| `squire-adapter` | Completed — real `SquireContextAdapter` in `src-tauri/src/agent/squire.rs`, wired into `send_message_impl`. Strict Q5 tool boundary (explore/token_to_detail/invoke only), Q6 validation gates + retry/failure via `TurnOutcome` enum. Both follow-up gaps flagged at the time (sa-4, sa-5) are now resolved — see `stream-sigil-fix` and `ask-user-loop` below. |
| `squire-storage` | Completed — real `LanceDbSquireStore` (`src-tauri/src/storage/squire_lancedb.rs`) implements the `SquireStore` trait against LanceDB (Q4). Its one flagged follow-up gap (`squire-storage/todo.json` ss-9, real tool-token ingestion) is now resolved — see `tool-token-ingestion` below. |
| `rejection-ux` | Completed — real Q6 compliance-failure UX and real Q7 preserve-list lifecycle. |
| `protocol-doc-sync` | Completed — both protocol docs reconciled against runtime truth; flagged 5 genuine implementation gaps beyond the previously-known sa-4/sa-5/ss-9 (graph traversal, hit-count scoring, user-input auto-chunking, raw-partition audit storage — plus reconfirming ask_user as known). **All five are now resolved.** |
| `retrieval-fidelity` | Completed — implements the two gaps `protocol-doc-sync` judged most load-bearing (spec §7.3's explicit "core differentiator from a RAG wrapper" claims): graph traversal (`num_hops`) and `accumulated_hits`/`effective_priority` scoring, in both `SquireStore` backends. |
| `stream-sigil-fix` | Completed 2026-07-02 — closed `squire-adapter/todo.json` sa-4 (live-stream sigil leak). |
| `ask-user-loop` | Completed 2026-07-03 — closes `squire-adapter/todo.json` sa-5 (response-field AskUser loop), with real end-to-end manual verification. |
| `tool-token-ingestion` | Completed 2026-07-03 — closes `squire-storage/todo.json` ss-9 (real tool-token ingestion). |
| `session-creation-ux` | Completed 2026-07-03 — closes the previously-unclaimed "no frontend UI to create a Squire-mode session" gap. Surfaced two small optional follow-ups in its own Risks section, both now resolved (see `session-ux-polish` below). |
| `user-input-chunking` | Completed 2026-07-03 — closes `protocol-doc-sync`'s item-11 gap: user-input auto-chunking into `USR_TN_NNN` tokens. |
| `raw-partition-storage` | Completed 2026-07-03 — closes `protocol-doc-sync`'s item-12 gap: raw-partition audit-log storage. |
| `session-ux-polish` | Completed 2026-07-03 — closes both small UX follow-ups `session-creation-ux` surfaced: toggle persistence across remounts, and an active-conversation chat-header mode indicator. |
| `hit-count-fidelity` | Completed 2026-07-03 — closes `retrieval-fidelity/todo.json` rf-13 (fuller hit-count-event fidelity). |
| `token-detail-endpoint` | Completed 2026-07-03 — closes the endpoint-carrying `TokenDetail`/`invoke()` extension `tool-token-ingestion` deliberately left out of scope. |
| `memory-alias-fix` | Completed 2026-07-03 — closes the `"memory"`-alias/`system_referential` gap `user-input-chunking` flagged. See below for detail. |
| `testing` | **Not started, created 2026-07-03** — new dedicated testing/QA phase for the Squire context-mode pipeline as a whole (adapter, storage, retrieval, streaming). Not a gap closure; a consolidation/hardening pass over the cumulative test coverage the 17 completed nodes above built up incrementally. See below for detail. |
| `tool-token-registry` | **Planned, created 2026-07-03** — new node to fix a verified tool-discovery break: `SquireExploreTool`'s live-registry contiguous-substring filter fails multi-word natural-language queries that `explore_memory`'s existing word-level matching would catch. Unifies built-in and MCP tool registration onto one `SquireStore` schema, register-before-explore. Design later simplified to pivot on a real local embedding model swap (`fastembed BGESmallENV15`, 384-dim) as the keystone change. See below for detail. |
| `squire-observability` | **Planned, created 2026-07-03** — new node building debug/observability facilities for the Squire semantic loop: a structured `squire-trace.log` (JSONL) for retrieval scoring (cosine/substr_boost breakdown, near-misses, embedding-path tag), token lifecycle, the explore->detail->invoke funnel, per-turn store snapshots, and timing, plus a query-probe dev command. A DEPENDENCY of `testing` (provides the instrumentation tests will assert against); closely related to `tool-token-registry` (tags real-vs-fallback embedding path). See below for detail. |

**Every node in the originally-planned Child Nodes list, plus eleven follow-up nodes (`retrieval-fidelity`, `stream-sigil-fix`, `ask-user-loop`, `tool-token-ingestion`, `session-creation-ux`, `user-input-chunking`, `raw-partition-storage`, `session-ux-polish`, `hit-count-fidelity`, `token-detail-endpoint`, `memory-alias-fix`), is complete.** Every gap `protocol-doc-sync` ever flagged is resolved, both optional UX follow-ups `session-creation-ux` surfaced are resolved, `retrieval-fidelity`'s own flagged follow-up (rf-13) is resolved, the endpoint-carrying `TokenDetail` extension is resolved (full implementation), and the `"memory"`-alias gap is resolved too. **The residual functional backlog is down to exactly one item, and it is intentionally staying open, not pending closure:** the nested-`§!`-citation residual `hit-count-fidelity` flagged. The user was asked directly and chose to leave it alone ("ignoring nesting feels more right to me") rather than have it fixed — this is a final product decision, not a deferral. **A new, separate `testing` phase node has now been created** to consolidate and harden test coverage across the whole pipeline before considering the epic fully closed out at this level — see its own `prompt.md`/`env.md`/`todo.json` for full scope.

**`.AiControl/.current`** now points to `root/Squire/memory-alias-fix`. Whoever picks up next should repoint it to wherever they choose to work — there is no more unclaimed, intended-to-be-closed backlog left in this epic.

## Test LLM provider still configured — manual verification remains possible

A free-tier, OpenAI-compatible LLM provider (OpenCode Zen, `deepseek-v4-flash-free`) remains
configured in the app's **real** config location, `%APPDATA%\com.squirecli.app\config.toml`
on Windows (not `src-tauri/.squirecli/config.toml` — that path is only a `dirs_fallback()`
used when `set_config_dir` is never called, e.g. by unit tests; it is not what a real `tauri
dev`/built-binary run reads). This session used it: the extended
`e2e/specs/session-creation-ux.test.ts` sends a real message through a real Squire-mode
session as part of its existing (unmodified) second case, and the small free-tier model
again closed the turn cleanly.

**This is a shared free-tier test credential, not a production secret, but still treat it
carefully:** don't paste the raw key into any committed doc. If you need to reference that a
provider is configured, just say "a free-tier test provider is configured" without embedding
the key.

## What `session-ux-polish` did (prior session, 2026-07-03)

Closed both small, optional, non-blocking UX follow-ups `session-creation-ux` surfaced in
its own `decisions.md`/`state.md` Risks section:

1. **Toggle persistence.** `ConversationSidebar`'s Squire-mode creation toggle
   (`nextSessionSquireMode`) previously reset to off/Legacy on every remount. It now
   persists its last-chosen value via `localStorage` — two new functions,
   `loadStoredSquireModeDefault()`/`saveStoredSquireModeDefault(value)`, added to
   `src/stores/chat-store/preferences.ts` (new key `chat:last-squire-mode-default`),
   mirroring that file's existing provider/model/thinking-level persistence pattern exactly.
   The first-run/never-touched default is unchanged (still Legacy).
2. **Active-conversation chat header indicator.** `chat-panel.tsx`'s "Chat" tab now shows a
   small "Squire" badge (visually identical to the pre-existing sidebar badge) when the open
   conversation is Squire mode, derived via `useMemo` over already end-to-end-tested data —
   no new IPC/store surface needed.

**Testing:** 5 new frontend unit/component tests; `npm test -- --run` 87/89 passing (82
baseline + 5 new), same 2 known pre-existing failures. `cargo build`/`cargo test --lib`:
unchanged, clean/206/206 (pure frontend node). Verified via an extended
`e2e/specs/session-creation-ux.test.ts` (now 3 cases) plus component tests for the header
indicator; found and fixed a real, pre-existing e2e timing race along the way (see
`session-ux-polish/decisions.md` for the full writeup).

## What `hit-count-fidelity` did (prior session, 2026-07-03, closes `retrieval-fidelity/todo.json` rf-13)

Closed the one remaining item in `retrieval-fidelity`'s own follow-up list: fuller fidelity
to spec §3.3's four hit-count-increment events.

**The precise gap.** Spec §6.1's own gloss on event 1 ("Token appears in explore() results
that AI acts on") is a disjunction: "Squire increments accumulated_hits for every token in the
returned list that the AI subsequently acts on (**calls token_to_detail or references in
output**)." `retrieval-fidelity`'s original proxy wired only the first disjunct — a hit was
credited only when the model called `token_to_detail` on a token. The second disjunct — an AI
citing a token via `§!TokenID` directly in its own response `content`, purely from having seen
its `short_desc` in a prefetched/preserved/explored list, without ever calling
`token_to_detail` — previously earned **zero** hit credit, despite being (if anything) the
*more* common citation pattern the system prompt itself encourages ("§!TokenID - inline
reference to an existing token, expanded to its short description before display"). This same
wiring point also satisfies event 3 ("§! reference found in a chunk loaded into context") for
the AI's own response content, since that content is unambiguously "loaded into context" via
`expand_for_display` immediately afterward.

**The fix.** `SquireContextAdapter::finalize_turn` (`src-tauri/src/agent/squire.rs`) now
credits a hit, via the pre-existing `record_hit` method (`retrieval-fidelity`; no new
`SquireStore` trait surface needed), for every already-existing token in `finalize_turn`'s own
pre-existing `known` set — the set of `§!`-referenced ids already confirmed via `token_exists`
**before** the turn's own `new_tokens` are upserted. This ordering is exactly what makes the
double-count guard work for free: a token that is *both* defined in `new_tokens` and cited via
`§!` in the same turn (the ordinary "define and cite" pattern) is correctly excluded from this
new crediting loop, since it wasn't yet `token_exists` when `known` was computed — it still
gets exactly one hit, from `upsert_token`'s pre-existing "regardless" +1 rule (event 4). A
pre-existing token merely cited (not redefined) is correctly included and gets exactly one new
hit, regardless of how many times it's cited in that one response (the pre-existing
`HashSet`-based dedup in `known` already handles that). No `SquireStore` trait changes, no
signature changes anywhere — the entire change is one new loop inside an existing function,
calling an already-existing, already-tested primitive method.

**What remains deliberately unwired, and why.** A `§!` reference nested *inside* a `full_desc`
body itself (a chunk citing a different chunk — only surfaced when that body is loaded via
`token_to_detail`) is still not scanned for embedded references. Closing this fully would
require a genuine context-composition audit pass scanning every piece of content that ever
enters context (the AI's response, every `full_desc` returned by every `token_to_detail` call,
every prefetched `short_desc`, tool results) — the same disproportionate-infrastructure
concern `retrieval-fidelity/decisions.md` originally flagged for the broader gap this node
substantially narrows. This is documented explicitly in `hit-count-fidelity/decisions.md` and
`state.md` as a deliberate, bounded tradeoff, not a silent gap — and it is meaningfully
narrower than the original rf-13 gap, since it now only affects a comparatively rare authoring
pattern (a token's own content citing a different token) rather than the much more common
"AI cites a token directly in its own visible output" pattern this node fully fixes. A future
node could close it by extending `SquireTokenToDetailTool::execute` to scan its returned
`full_desc`/`short_desc` via the same `extract_inline_refs` helper reused here.

**Testing:** 4 new unit tests — 3 in `squire.rs` (citing a pre-existing token without calling
`token_to_detail` now earns a hit; a token defined-and-cited in the same turn earns exactly
one hit, not two; repeated citations of the same token in one response still credit exactly
one hit) against real `finalize_turn` integration paths through `InMemorySquireStore`, plus 1
in `squire_lancedb.rs` confirming the real `LanceDbSquireStore`'s `record_hit`/`upsert_token`
primitives compose identically to what the new call site performs. `cargo build`/`cargo build
--bins`/`cargo build --examples`: all clean, zero warnings. `cargo test --lib`: **210/210
passing** (206 baseline + 4 new). No WDIO/GUI spec or new headless example harness was
needed — confirmed via a repo-wide frontend grep (zero hits for
`accumulated_hits`/`hit-count`/`record_hit`) that this remains pure backend scoring logic with
no user-facing surface, and the change itself needed no new cross-process/storage data flow
beyond what `retrieval-fidelity`'s own tests already exercise (no new table, no new trait
method). See `hit-count-fidelity/decisions.md` for the full operationalization, the
double-count-guard reasoning, and the deliberately-deferred nested-citation tradeoff.

## What `token-detail-endpoint` did this session (closes the endpoint-carrying `TokenDetail`/`invoke()` extension)

Closed the gap `tool-token-ingestion/env.md` explicitly scoped out and `squire-storage/
decisions.md` originally described as a second, separate "full cutover" piece beyond
ingestion itself: `SquireInvokeTool`'s store-fallback path could describe a
not-currently-live tool but never actually invoke it, dead-ending on a "recorded but no
invocable endpoint bound yet" diagnostic.

**Proportionality assessment, and why it landed on full implementation.** The task framing
(and `tool-token-ingestion/env.md`'s original non-goal) both worried real dispatch might
require "session/connection lifecycle management outside the normal per-turn flow" — the kind
of disproportionate new infrastructure this epic has repeatedly and correctly declined
elsewhere (e.g. `retrieval-fidelity`'s declined context-composition audit trail,
`tool-token-ingestion`'s declined active tool-token cleanup sweep). Reading the actual MCP
dispatch code (`src-tauri/src/mcp/mod.rs`) in full before assuming that framing was correct
overturned it: `crate::mcp::call_tool` is **already** a stateless, one-off-per-call operation
— every MCP tool call, whether the tool is "live" in the current turn's registry or not,
already spins up a fresh `StdioMcpClient`, connects, calls the one tool, and disconnects
(`StdioMcpClient`'s `Drop` kills the child process). There is no persistent MCP session
anywhere in this codebase to begin with, so "reconnecting on demand" isn't a new category of
operation — it's calling the exact same function the live path (`McpProxyTool::execute`)
already calls. Combined with `McpServerConfig` already being a small, plain,
`Clone`/`Serialize` struct (nothing new to invent to describe "how to reach this server"),
this made real dispatch tractable and proportionate, not a disproportionate new feature — so
this node implemented **full real dispatch**, not a scoped-down diagnostic. Also confirmed via
`ToolRegistry::new()`'s unconditional local-builtin registration (no config/enablement gate)
that this gap can only ever be meaningful for MCP-sourced tools — a local/built-in tool can
never actually be "ingested but not currently live."

**Implementation.** `TokenDetail`/`NewTokenSpec` (`src-tauri/src/agent/squire.rs`) gained a
new `endpoint: Option<ToolEndpoint>` field; `ToolEndpoint` is a new, single-variant enum
(`Mcp { server: McpServerConfig, remote_name: String }` — no local-builtin variant, since one
can never be needed per the finding above). Both `InMemorySquireStore` and
`LanceDbSquireStore` persist/return it — the LanceDB backend gained a new nullable,
JSON-serialized `endpoint` Utf8 column on the `squire_tokens` table, following
`retrieval-fidelity`'s established "nullable column, no destructive migration for
pre-existing directories" precedent. `ingest_tool_registry` gained a new `endpoints:
&HashMap<String, ToolEndpoint>` side-channel parameter — purely additive, every pre-existing
call site (all prior tests, the `tool_token_ingestion_e2e.rs` example) passes an empty map and
sees byte-for-byte unchanged behavior — populated only in `streaming_cmd.rs`'s MCP-discovery
loop, the one place a tool's MCP origin (`McpServerConfig` + remote tool name) is still known
before `ToolDefinition` erases it. `SquireInvokeTool`'s store-fallback branch now checks
`detail.endpoint`: a `Some(ToolEndpoint::Mcp{..})` hit is genuinely dispatched via
`crate::mcp::call_tool` — the exact primitive `McpProxyTool::execute` already uses for live
tools — instead of only returning a diagnostic; `None` (local-builtin tokens, or MCP tokens
ingested before this node shipped) preserves the original diagnostic message unchanged,
self-healing automatically on the next turn's re-ingestion once that tool's server is live
again.

**Security constraint identified and verified.** `McpServerConfig` can carry secrets in its
`env`/`headers` fields (e.g. an API key for an authenticated MCP server), so `endpoint` must
never be exposed to the model. Confirmed `SquireTokenToDetailTool::execute` never
re-serializes the whole `TokenDetail` struct (only ever reads `short_desc`/`full_desc`
individually) and added a dedicated regression test proving its output never leaks endpoint
data even for a token with a real, secret-bearing endpoint recorded.

**Incidental bug found and fixed.** While implementing, adding the new 8th `endpoint` column
to `tokens_schema()` silently broke `LanceDbSquireStore::record_hit`, which builds its own
`RecordBatch` independently of `upsert_token`'s and had not been updated for the new column —
`RecordBatch::try_new` was failing every call (swallowed by an existing `let Ok(batch) = batch
else { return }` guard), making `record_hit` a silent no-op. This broke 7 pre-existing LanceDB
tests. Fixed by adding the same endpoint-column handling to `record_hit`'s batch construction,
mirroring `upsert_token`'s exactly; all 7 pass again. Caught by running the full test suite
before declaring the change done, not just the new tests — a useful reminder for any future
node that adds a column to `tokens_schema()` to check *every* `RecordBatch::try_new` call site
against that schema, not just the one being actively modified.

**Testing:** 11 new unit tests — 7 in `squire.rs` against `InMemorySquireStore` (serde
round-trip for `TokenDetail`/`ToolEndpoint`; `upsert_token` persists and returns an endpoint;
`upsert_token` without an endpoint preserves a previously-stored one, mirroring `full_desc`'s
existing merge semantics; `ingest_tool_registry` populates the endpoint only for MCP-sourced
definitions present in the `endpoints` map, `None` for local built-ins; an
empty-endpoints-map regression guard confirming every pre-existing call site's behavior is
unchanged; a real-dispatch test pointing a stored endpoint at a deliberately unreachable fake
command, confirming the new dispatch branch is taken and a real `crate::mcp::call_tool`
connection-failure message is surfaced — proving the real function is exercised, not a mock;
and the `SquireTokenToDetailTool` no-leak security test) plus 4 in `squire_lancedb.rs` against
the real `LanceDbSquireStore` (endpoint persists via upsert; endpoint persists across a real
reopen; `record_hit` preserves an existing endpoint through its own separate write path — the
exact bug this node found and fixed; `ingest_tool_registry` populates endpoint only for
MCP-sourced definitions against the real backend). `cargo build`/`cargo build --bins`/`cargo
build --examples`: all clean, zero warnings. `cargo test --lib`: **221/221 passing** (210
baseline + 11 new). No WDIO/GUI spec or new headless example harness was needed — confirmed
via a repo-wide frontend grep (zero hits for `ToolEndpoint`/`endpoint.*invoke`/
`token_detail.*endpoint`) that this remains pure backend dispatch logic with no user-facing
surface, matching `tool-token-ingestion`'s own verification-methodology precedent for a
similarly-scoped dispatch-mechanics change. See `token-detail-endpoint/decisions.md` for the
full proportionality assessment, the two key findings, the `ToolEndpoint`/`TokenDetail` shape
design, and the security-constraint reasoning.

## What `memory-alias-fix` did this session (closes the `"memory"`-alias/`system_referential` gap)

Direct user-requested fix (the gap was already fully diagnosed in conversation before this
node was created — no discovery phase needed). `explore()`'s `resource_type="memory"` is a
convenience alias, not a real token type; its expansion logic — a duplicated one-line boolean
clause in `type_matches`, in both `InMemorySquireStore::explore_memory` (`squire.rs`) and
`LanceDbSquireStore::explore_memory` (`squire_lancedb.rs`) — expanded to `concept`/
`referential` token types only. This predated `system_referential` (the type
`user-input-chunking` gave to `USR_T{turn}_{NNN}` chunk tokens) and was never updated when that
type was introduced, so a model using the "memory" shortcut silently missed the AI's own
chunked user-input tokens — even though `resource_type="system_referential"` (exact) and
`resource_type="all"` both already found them correctly. Fixed identically in both files:
`t == "system_referential"` added to the `"memory"` branch. Verified via 2 new unit tests (one
per backend) confirming the alias now surfaces a `system_referential` token created via
`ingest_user_input_chunks`. `cargo build`: clean. `cargo test --lib`: **223/223 passing** (221
baseline + 2 new).

Per direct user instruction, the other remaining backlog item — the nested-`§!`-citation
residual `hit-count-fidelity` flagged (a `full_desc` body citing another token via `§!`, only
surfaced when loaded via `token_to_detail`, not itself scanned for embedded references) — was
explicitly **not** addressed. The user was asked to choose between fixing both remaining
items or just this one, and chose: "i understand 2, but i dont understand 1" (asking for
clarification on the memory-alias gap), followed by "fix 1 and ignore 2 ignoring nesting feels
more right to me" once the gap was explained. This is treated as a final, deliberate product
decision — not a deferral — documented in `memory-alias-fix/decisions.md` and left unchanged
in `hit-count-fidelity/decisions.md`/`state.md`.

## What `testing` is for (new node this session, not started)

Functionally, the epic's implementation backlog is empty (see above). But no prior node's job
was to step back and evaluate the *cumulative* test suite as a whole — each node only added
tests for its own scoped change. `testing` (`root/Squire/testing`) is a new, dedicated
phase node created to do exactly that: inventory existing coverage across the pipeline's four
stages (adapter, storage, retrieval, streaming), identify concrete real gaps (two-backend
`SquireStore` parity drift, missing negative-path coverage, happy-path-only e2e specs,
untested frontend surfaces), close them following the epic's already-established testing
conventions, and finish the `e2e/specs/*.test.ts` flakiness audit `session-ux-polish` started
but explicitly left incomplete. It is a QA/hardening pass, not a new feature or gap-closure
node — see `testing/prompt.md` for full scope and explicit out-of-scope boundaries (in
particular: it must not reopen the nested-`§!`-citation residual `hit-count-fidelity`/
`memory-alias-fix` deliberately left as a permanent simplification).

## What `tool-token-registry` is for (new node this session, planned, not started; design
## SIMPLIFIED 2026-07-03 after discussion — see below)

A verified, pre-diagnosed root cause: `SquireExploreTool::execute` (`src-tauri/src/agent/
squire.rs:1032-1053`) serves `resource_type="tool"|"tool_skill"` from the live `ToolRegistry`
using a **contiguous-substring filter** over the *entire* lowercased query, not the word-level
bag-of-words matching every other token type gets via `explore_memory`. In a real session the
model issued multi-word descriptive queries ("web scraping fetch url html", "scrape website
data", "fetch url web page http request") against `web_fetch`'s description ("Fetch a web page
and return its HTML content. Useful for reading documentation, checking APIs, or scraping web
content.", `agent/mod.rs:513-515`) — none matched as a contiguous substring even though every
individual word (fetch/web/html/scraping) appears in the description. Routing tool discovery
through the store's matching path (instead of the live-registry shortcut) would have fixed this
exact failure. This root-cause diagnosis is unchanged and still the node's motivating problem.

**The design was simplified after discussion.** The originally-planned MCP tool-description
pipeline — a separate turn-0 LLM pre-pass batching descriptions into
`[{tool, short_desc, keywords[]}]`, plus a global persistent summary cache keyed by
`hash(raw_desc)` — is now DROPPED. The only reason to summarize/keyword-extract descriptions
was to make them findable under a lexical/word-level matcher; once a real local embedding model
(encoder) is bundled, LanceDB does true semantic vector matching directly on the FULL raw
description, making summarization unnecessary. **The design now pivots around one keystone
change: bundle a small local embedding model** (e.g. `bge-small-en-v1.5` or
`all-MiniLM-L6-v2`, both 384-dim, via `fastembed-rs`/ONNX — encoder only, no generative LLM),
replacing the toy 64-dim bag-of-words `embed_text` (`squire_lancedb.rs:54-72`, bumping
`EMBED_DIM` to 384). `embed_text` is already the single function called at both ingest and
query time, so this one swap covers both. This reverses the node's original scoping, which had
explicitly kept real-embedding-model integration out of scope for a future node — that decision
is now overturned, confirmed via a worked "make html" example (retrieving a relevant
coding/HTML token with zero shared words with the query — something only real semantic
embeddings, not any lexical/keyword scheme, can do).

`tool-token-registry` (`root/Squire/tool-token-registry`) implements the full, simplified fix:
built-in and MCP tools still converge on one `SquireStore` schema and are discovered through
the same semantic-vector path as every other token (register-before-explore, unchanged
principle) — closing the broken discovery itself, missing package/resource granularity (all
tools are a flat `token_type="tool"`), and the built-in/MCP registration asymmetry. Built-ins
register at conversation/session start (idempotent, authored `short_desc`, full description
embedded, no LLM); MCP tools register at connect by embedding the FULL raw description directly
(no LLM, no summarization, no cache), with the display `short_desc` produced by deterministic
truncation. `SquireExploreTool`'s live-registry substring shortcut is removed for tool/
tool_skill, rerouted through the store's semantic path; `invoke()`/dispatch's
`token_id -> endpoint` resolution (from `token-detail-endpoint`) is explicitly kept unchanged —
discovery moves to the store, execution stays wired to the registry. This node builds directly
on `tool-token-ingestion`'s existing `ingest_tool_registry` free function and per-turn trigger
point in `streaming_cmd.rs`, changing *when*/*how* registration happens for discovery purposes
rather than replacing the underlying `upsert_token`-based write mechanism. See `tool-token-
registry/prompt.md` for the full verified problem statement (with file:line citations) and the
simplified architecture; `tool-token-registry/decisions.md` for the four originally-open design
decisions now RESOLVED (MCP summarization mechanism -> dropped; generative LLM/turn-0 pre-pass
-> dropped; cache location -> moot; embedding-model scope -> reversed, now in scope as the
keystone, with the "make html" worked example) plus two new OPEN sub-decisions (which embedding
model: `bge-small-en-v1.5` vs. `all-MiniLM-L6-v2`; distance computation: keep manual full-scan
cosine vs. switch to LanceDB's native vector index) flagged for confirmation before
implementation starts.

## What `squire-observability` is for (new node this session, planned, not started)

Debug/observability facilities for the Squire "semantic loop" itself, motivated directly by the
`testing` node's own needs: that node's test suite is going to grow more complex and span other
parts of the design spec, and correctness of semantic retrieval / token accretion cannot be
verified by eyeballing `provider-wire.log` — that log already shows an `explore()` call's query
args and the result handed back to the LLM, but nothing about the internal scoring, near-misses,
or which embedding path produced a given score. **This node is a DEPENDENCY of `testing`** — it
provides the instrumentation a growing test suite needs to assert against, not the tests
themselves. It is also closely related to `tool-token-registry`, which just landed the real
`fastembed BGESmallENV15` (384-dim) embedding model this node's own embedding-path tagging is
built to distinguish from the toy bag-of-words fallback (critical because a word-overlap hit
looks identical under both paths, but only the real model can retrieve a token with zero shared
words with the query, per that node's "make html" worked example).

**Design.** A dedicated structured trace, `squire-trace.log`, written as JSONL (one `{turn,
tool_call_id?, event, payload, ts}` object per line, separate from `provider-wire.log`), gated
behind a dedicated debug flag (recommended over reusing `verbose_logging`, so trace can be
enabled without also turning on full wire-log verbosity). Six event categories, in priority
order: (1) **retrieval trace** — per `explore()` call, every candidate's `cosine`/`substr_boost`
score breakdown, hop distance/via-token provenance, and — critically — the NEAR-MISSES currently
discarded silently at `LanceDbSquireStore::explore_memory`'s `score<=0.0` cut
(`squire_lancedb.rs:704`), each tagged with which embedding path scored it; (2) **token
lifecycle** — created/preserved/relationships-written per turn; (3) **funnel** —
`token_to_detail`/`invoke` calls, reconstructing the explore->detail->invoke decision chain; (4)
**per-turn store snapshot** — token counts by type, `accumulated_hits` distribution; (5)
**timing** — embed-inference latency, explore latency, model init/download duration; (6) a
**query-probe dev command** — run an arbitrary query against the current store and dump ranked
scores + near-misses + embedding path without driving the whole agent, recommended as a Tauri
command (usable from a future dev panel) with an acceptable fallback of a headless example
harness first if new IPC plumbing would otherwise block higher-value instrumentation work.

**While seeding this node, found and corrected a stale fact in this file's own `env.md`** — its
"Vector search uses a deterministic hash-based embedding, not a real embedding model" bullet
predated `tool-token-registry`'s completed real-embedding-model swap and was left unfixed; it now
describes the real `fastembed BGESmallENV15` default path plus its bag-of-words fallback for
offline/init-failure, per the write-back obligation (a durable, epic-wide fact belongs in the
parent, not only in a leaf node's own files). See `squire-observability/prompt.md` for the full
task specification and verified file:line code citations; `squire-observability/decisions.md`
for the three open decisions (debug-flag mechanism, JSONL vs. text format, Tauri-command vs.
headless-harness query-probe surface) with recommendations recorded for each; `squire-
observability/todo.json` for the ten-item (`obs-1`..`obs-10`) implementation breakdown.

## Should the epic be closed out?

**The functional/implementation backlog is empty and ready for closeout**, but the epic is
being kept open one more phase for `testing` (above) before final closeout at the
`root/Squire` level. Every gap `protocol-doc-sync` ever flagged is resolved (sa-5/ask_user,
graph traversal, hit-count scoring, user-input auto-chunking, raw-partition storage). Every
gap flagged by `squire-adapter` (sa-4, sa-5) and `squire-storage` (ss-9, and the
endpoint-carrying `TokenDetail` extension) is resolved. Both optional UX follow-ups
`session-creation-ux` surfaced are resolved. `retrieval-fidelity`'s own flagged follow-up
(rf-13) is resolved. The `"memory"`-alias gap is resolved. The **only** functional item left in
the backlog — the narrower nested-`§!`-citation residual `hit-count-fidelity` flagged (a
`full_desc` body citing another token, only surfaced via `token_to_detail`) — has been
explicitly reviewed by the user and kept as-is on purpose, not left open for lack of time or
prioritization; `testing` must not re-open it.

**Recommendation:** complete the new `testing` phase, then mark the epic complete at the
`root/Squire` level. The nested-citation residual should remain documented in
`hit-count-fidelity`'s own files as a permanent, intentional simplification (not re-flagged as
open backlog anywhere else), since re-opening it would contradict a direct, considered user
decision.

## Verification status as of this commit

- `cargo build`/`cargo build --bins`: clean, zero warnings. `cargo test --lib`: **223/223
  passing** (221 baseline from `token-detail-endpoint` + 2 new from `memory-alias-fix`: one in
  `squire.rs`, one in `squire_lancedb.rs`, both confirming `resource_type="memory"` now
  surfaces `system_referential` tokens).
- Frontend: unchanged by this session (pure backend filter-predicate fix, no user-facing
  surface). Last known frontend status (from `session-ux-polish`): `npx tsc --noEmit -p
  tsconfig.app.json` zero new errors (same 7 pre-existing `tools-panel.tsx` errors); `npm
  test -- --run` 87/89 passing (same 2 pre-existing failures).
- No e2e/manual verification needed for this fix — a one-line predicate change covered
  precisely by the two new unit tests; consistent with how every other `explore_memory`
  filtering change in this epic was verified.

## Prior session's verification (token-detail-endpoint, for reference)

`cargo build`/`cargo build --bins`/`cargo build --examples`: all clean, zero warnings. `cargo
test --lib`: 221/221 passing (210 baseline + 11 new: 7 in `squire.rs`, 4 in
`squire_lancedb.rs`). No new real end-to-end verification was performed that session — judged
disproportionate for a backend dispatch-mechanics change with no new user-facing surface; the
real-dispatch behavior was covered by a unit test exercising the actual `crate::mcp::call_tool`
function against a deliberately unreachable command (a real, if failure-mode, exercise of the
real dispatch path) rather than a mock. See `token-detail-endpoint/decisions.md`'s
verification-methodology section for the full reasoning.

## Known pre-existing issues (not from this session, not yet fixed)

1. `src/components/tools-panel.tsx` references `AppConfig.disabledTools`, which doesn't
   exist on the `AppConfig` type in `src/types/ipc.ts`, plus an invalid `title` prop passed
   to a lucide-react icon (2 spots). Predates this epic.
2. `chat-input.test.tsx` — "calls onSend on Enter without Shift" fails intermittently;
   confirmed pre-existing and unrelated by multiple prior sessions.
3. `chat-blocks.test.tsx` — "renders thinking blocks collapsed by default" also fails at
   HEAD, same reasoning as #2.
4. A handful of other `expect(...).toBe(...)` assertions immediately after a UI action
   exist elsewhere in the e2e suite (`e2e/specs/`) without polling, the same pattern that
   caused this session's flakiness finding in `session-creation-ux.test.ts`. Only the one
   assertion that actually raced during this session's own verification was fixed; the rest
   were not audited. Candidate for a small, separate test-hardening pass if flakiness is
   observed there too.

## Newly observed gaps

None newly flagged by `memory-alias-fix`. No open, unclaimed, intended-to-be-fixed gaps remain
anywhere in this epic. The only documented residual — the nested-`§!`-citation gap
`hit-count-fidelity` flagged (a `full_desc` body citing another token via `§!`, only surfaced
when that body is loaded via `token_to_detail`, not itself scanned for embedded references) —
has been explicitly reviewed and kept as a permanent, intentional simplification per direct
user instruction (see "What `memory-alias-fix` did this session" above). It remains documented
in `hit-count-fidelity/decisions.md`/`state.md`'s Risks section, but should not be re-surfaced
as open backlog.

## To resume from home

1. `git pull`.
2. If `protoc` isn't already installed/on PATH (only matters for a cold build): `winget
   install --id Google.Protobuf -e`, then either restart your shell or point `PROTOC`
   directly at the winget package's `bin/protoc.exe` (see `squire-storage/decisions.md`/
   `retrieval-fidelity/env.md` for the exact path this environment used).
3. `cd src-tauri && cargo build && cargo test --lib` — should be clean/223 passing,
   confirming the pull landed correctly. From repo root, `npx tsc --noEmit -p
   tsconfig.app.json` and `npm test -- --run` should show the same 7 pre-existing TS errors
   and 87/89 frontend tests (2 known pre-existing failures).
4. The epic is ready for closeout at the `root/Squire` level — there is no remaining backlog
   intended to be fixed. If picking up new work in this area, treat it as a fresh follow-on
   epic/node rather than "finishing" this one.
5. A test LLM provider is configured (see above) for real end-to-end verification if a
   future session wants it — the following are left in the repo as reusable verification
   tooling:
   - `src-tauri/examples/ask_user_e2e.rs` and `e2e/specs/ask-user-loop.test.ts` — for any
     future ask_user-related work.
   - `src-tauri/examples/tool_token_ingestion_e2e.rs` — for any future tool-ingestion-related
     work (no LLM/network needed, deterministic; note it does not yet exercise the new
     `endpoints` parameter `token-detail-endpoint` added to `ingest_tool_registry` — it still
     passes an empty map, matching its own pre-existing scope).
   - `src-tauri/examples/user_input_chunking_e2e.rs` — for any future chunking-related work
     (no LLM/network needed, deterministic; e.g. a starting point for a real-model-driven
     check of chunk-token referencing, or for testing the `"memory"`-alias follow-up).
   - `src-tauri/examples/raw_partition_storage_e2e.rs` — for any future raw-partition-related
     work (no LLM/network needed, deterministic; exercises both the real `LanceDbSquireStore`
     and a real SQLite `ConversationStore` together in one run).
   - `e2e/specs/session-creation-ux.test.ts` — for any future session-creation/mode-selector
     UI work (now 3 cases: default-legacy, real-squire-creation, toggle-persists-across-
     remount).
   For e2e specs: `tauri-driver` needs `msedgedriver.exe` on `PATH` — see
   `ask-user-loop/decisions.md` for where a prior session found a cached copy.
6. Read `token-detail-endpoint/decisions.md` for the full proportionality assessment (why
   real dispatch turned out tractable rather than requiring new MCP session/connection
   lifecycle infrastructure), the `ToolEndpoint`/`TokenDetail` shape design, and the
   security-constraint reasoning; `hit-count-fidelity/decisions.md` for the full
   hit-count-event operationalization, double-count-guard reasoning, and the
   deliberately-deferred nested-citation tradeoff; `session-ux-polish/decisions.md` for the
   full toggle-persistence-mechanism, chat-header-indicator-placement, and
   e2e-flakiness-root-cause writeup; `raw-partition-storage/decisions.md` for the full
   unmarked-vs-verbatim textual argument and the read-back-mechanism conclusion;
   `user-input-chunking/decisions.md` for the full four-judgment-call chunking design;
   `session-creation-ux/decisions.md` for the original UI-placement/default-behavior/
   visual-indicator design reasoning; `ask-user-loop/decisions.md` for the pause/resume
   mechanism design and verification methodology; `stream-sigil-fix/decisions.md` for the
   sa-4 fix reasoning; `retrieval-fidelity/decisions.md` for the original scoring
   formula/hit-count-event wiring/traversal design reasoning; and `tool-token-ingestion/
   decisions.md` for the tool-token ingestion trigger-point/id-scheme/content-shape/
   staleness design this session built directly on top of.
