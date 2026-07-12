# Squire — Architecture Spec (MVP)

## 1. Purpose & Design Philosophy

Squire is a stateless AI agent gateway designed to combat context inflation and attention drift in traditional agents. Rather than accumulating conversation history and passing it forward by default, Squire dynamically assembles context per turn — retrieving only what's relevant, on demand.

**Core design principle:** collapse every category of information — knowledge, tools, workflows, capabilities — into a single uniform structure (tokens), and expose exactly one primitive for interacting with that structure (discovery). This mirrors two prior architectural collapses in computing history:

- **Von Neumann architecture**: instructions are data, one uniform fetch/execute loop.
- **Unix**: devices are files, one uniform read/write interface.
- **Squire**: knowledge, workflow, and tools are all tokens, one uniform discover/retrieve interface.

The payoff is the same in all three cases: the system becomes extensible without modifying its core. New behavior — personas, orchestration patterns, cleanup procedures, subagent-style dispatch — is added as *content* (more tokens), never as new Squire mechanics.

**What Squire is:** an API gateway providing discovery over a token store, plus context-assembly optimization.

**What Squire is not:** a reasoning engine, a policy enforcer, a security boundary, or an orchestrator. All of those are emergent from workflow content authored and ingested into the store — Squire has no built-in modes, personas, or agent types.

---

## 2. Token Model

Everything the system knows about or can act on is represented as a token. Four types:

### a. Source tokens (raw text)
Raw ingested text — a file, a user request, an AI response, a URL fetch, tool output. When ingested, raw text is chunked into addressable segments (exact chunking/bookmark syntax deferred — not part of this spec).

### b. Referential tokens
Point into a range within a single source token; never duplicate the underlying text. A referential token's start and end must resolve within the *same* source token (a span cannot cross two different sources — it wouldn't have coherent meaning).

### c. Concept tokens
Carry a short and long description. Do not refer to any physical text — they represent an idea, preference, fact, or relationship-organizing node purely for graph structure.

### d. Relationship tokens (triplets)
Define a relationship between two other tokens (subject, predicate, object). A relationship token can connect any token types — e.g. a source token (workflow.md) can simultaneously *be* a workflow via a relationship token asserting "this source IS-A workflow"; an AI response fragment can be linked to a user-intent concept via "SUPPORTS" or "CONTRADICTS."

**Note:** token type is not the same as *role*. A raw file is a source token, but a relationship token can additionally assert that it functions as a workflow, a policy document, or anything else. Roles are graph-assigned, not hardcoded to type.

### Knowledge categories (informational, not structural)
- **External knowledge** — files, source code, URLs, anything not yet ingested.
- **Internal knowledge** — anything ingested, chunked, and tokenized into the store (including tools — see §6).

---

## 3. Retrieval Layer

Two complementary retrieval mechanisms sit behind Squire's single discovery interface:

- **Vector DB** — semantic similarity search over token short/long descriptions, used for matching, distance scoring, and filtering.
- **Triplet store** — relationship tokens enable graph traversal ("hopping") from a known token to related tokens.

### Primitives

```
explore(text, type, num_results) -> seed tokens, via vector search
rdf(token, hops)                  -> related tokens, via triplet walk
```

- `explore` returns candidate tokens (short-description form) ranked by relevance (vector distance), gated by relevance first — recency/frequency (see §5) is a secondary tiebreak only and never promotes an irrelevant token into results.
- `rdf` performs a mechanical, non-judgmental walk of triplet edges outward from a given token, up to `hops` steps. It does not reason about which edges matter — that judgment belongs to the AI.
- Squire performs no relational reasoning itself. Triplets exist for the AI's reasoning process; Squire is a thin gateway that returns graph-adjacent tokens on request, nothing more.

### Batch composition syntax

To avoid unnecessary round trips without requiring a general scripting language, batches use a narrow, well-known subset of shell syntax:

- **`|` (pipe)** — output tokens of the left call become input tokens to the right call. Enables chains like `explore(...) | rdf(hops=2)` or further `| rdf(hops=1)`.
- **`&` / `;` / newline** — independent calls bundled into one round trip, no data dependency between them.

Explicitly **out of scope**: conditional execution (`&&`, `||`), redirection, subshells, command substitution. These would reintroduce general scripting complexity that the narrow pipe/batch model is designed to avoid. If a request needs branching logic, that logic belongs in the AI's reasoning between separate batch calls, not in the batch syntax itself.

### Bounding manual retrieval

The AI is instructed (via workflow) to manually construct necessary context before responding. To prevent unbounded retrieval loops, the number of batches issuable per turn is capped (exact cap: implementation parameter, not fixed by this spec — small, e.g. 2–3). Beyond the cap, the AI is expected to respond with available information or invoke clarification (see §8).

---

## 4. Context Assembly (Short List / Long List)

Every request assembles exactly two lists of tokens for the reasoning AI:

- **Short list** — token ID + short description only. Requires a `token_to_detail()` call to expand before use.
- **Long list** — full content already inlined. No further round trip needed.

### Sources feeding each list

1. **Squire prefetch** (this turn, optimization only) — a background semantic search run automatically on the incoming request:
   - One run against the workflow/skill/tool store (top 3 results)
   - One run against memory-type stores — conversation/ingested files (top 10 results), with a minimum-distance relevance floor
   - Always enters the **short list** only.

2. **Formatter carry-forward** (from previous turn) — the formatter (see §7) curates two preserved candidate lists at the end of each turn: one destined for next turn's short list, one for next turn's long list.

3. *(Future)* workflow-directed placement — workflows may eventually instruct the AI on what belongs in which list. Not required for MVP; the mechanism above is sufficient without it.

### Assembly algorithm

```
finalTokenIds = {}
budget = LONG_LIST_BUDGET

for token in longList_candidates:
    if token.id in finalTokenIds: continue
    if cost(token) <= budget:
        place as LONG; budget -= cost(token)
    else:
        place as SHORT   # demoted, never dropped

for token in shortList_candidates:
    if token.id in finalTokenIds: continue
    place as SHORT
```

Rules:
- **No token appears in both lists.** Long placement always wins on ID conflict; short duplicates are silently absorbed, not re-added.
- **Long list has a context-token budget.** When exhausted, remaining long-candidates are **demoted to short**, never dropped outright — nothing the formatter flagged as important is silently lost, it only degrades to "here's a pointer, fetch it if needed."
- **Short list has no equivalent budget guard** — it's cheap by construction (ID + short desc only).

### Correctness independent of optimization

The short/long list mechanism, prefetch, and formatter carry-forward are **pure optimizations**, not load-bearing for correctness. If Squire's prefetch and the formatter's carry-forward were both empty every turn, the system degrades to "AI manually explores everything from scratch" — slower and more round-trip-heavy, but never functionally broken. This is a deliberate property: tuning or disabling any optimization layer can only affect performance, never correctness.

---

## 5. Token Activeness (Recency + Frequency)

Each token carries a single field, `hitcount`, that jointly encodes recency and access frequency:

```
on access at turn T:
    if hitcount < T:  hitcount = T
    else:              hitcount += 1

activeScore = hitcount - currentTurn   // evaluated at query time
```

Behavior:
- A token untouched since creation at turn N has `hitcount = N`; at the current turn `activeScore` grows increasingly negative the longer it goes unaccessed ("cooling down").
- A token accessed at the current turn gets `hitcount` bumped to the current turn (`activeScore = 0`).
- Multiple accesses to the same token *within the same turn* increment `hitcount` further above the current turn — a burst-frequency credit. This credit persists and only erodes as future turns pass without further access (e.g. a token hit 5 times in turn 10, `hitcount = 15`, stays "hot" through turn 15 even with no further access, then decays normally).

`activeScore` is used strictly as a **secondary sort / tiebreak** among tokens already deemed relevant by vector distance or hop proximity. It never overrides relevance — a highly "active" but topically irrelevant token cannot outrank a relevant one.

---

## 6. Tools as Tokens

- Squire hides all MCP and builtin tools from the AI's direct view — there is no separate "tool-calling" interface exposed to the reasoning AI.
- All tools (MCP servers, builtin functions, skills, shell/file/DB access, sub-invocation capability) are ingested into the token store at Squire startup, represented the same as any other token (typically source + relationship tokens describing what the tool is and how to invoke it).
- Tools are discovered exactly like any other knowledge — via `explore()`/`rdf()` — and expanded via `token_to_detail()` to obtain invocation details.
- **Asking the user for clarification is itself a builtin tool**, discoverable the same way, not a special Squire-level branch.

This is the concrete realization of the "one primitive" principle (§1): there is no separate capability layer alongside the knowledge layer. Discovery is the only interface, and what's discoverable spans memory, workflow, and every capability the AI has access to.

---

## 7. Turn Lifecycle

1. **User input arrives.**
2. **Mechanical chunking** — input is chunked (exact chunking rules out of scope for this spec).
3. **Squire prefetch** — two semantic searches (tools/workflows top-3; memory top-10, distance-floored) run automatically, results enter short-list candidates.
4. **Context assembly** — prefetch candidates merge with the previous turn's formatter-preserved short/long candidates per the algorithm in §4, producing the turn's final short and long lists.
5. **Main reasoning AI** receives the short/long lists and the user input. It is instructed to manually construct any additional necessary context via `explore`/`rdf` batches (bounded, §3) before responding — including discovering any workflow tokens that govern how it should behave for this request (style, process, security checks, etc. — see §8). It generates a response.
6. **Response returned to user.**
7. **Formatter pass (background)** — a second AI, chosen for speed/structured-output reliability rather than reasoning depth, receives the (user request, AI response) pair for the turn. It:
   - Mints new referential tokens highlighting important pieces of the response/request
   - Mints relationship tokens (triplets) describing how new and existing tokens relate
   - May mint concept tokens purely for organizational/graph-building purposes
   - Curates the next turn's preserved short-list and long-list candidates
   - The formatter operates statelessly per turn — it sees only the current turn's request/response pair, not the existing graph neighborhood. Deduplication/merging of near-duplicate concept tokens is **not** performed here; it's deferred to compaction (§9).
8. **DB write** — new tokens and relationships are persisted; `hitcount` updates apply to whatever was accessed this turn; turn counter increments.

---

## 8. Governance via Workflow Tokens

Workflows are not a distinct token *type* — they are ordinary tokens (typically source tokens with a relationship token asserting "IS-A workflow"), discovered and retrieved exactly like any other knowledge. There is no special Squire-level handling for them.

Workflows can govern:
- **Style** — e.g. friendly conversational tone vs. structured/formal response.
- **Process** — e.g. "generate objectives, create a plan, verify your answer" before responding.
- **Security/validation** — e.g. run a script to check for injection attempts before ingesting external content.
- **Persona/domain adaptation** — different workflows for different user types (e.g. a housewife vs. a scientist vs. a student), ingested from external sources and matched to the current user/context.
- **Meta-orchestration** — a "workflow of workflows" that arbitrates between other matched workflows (e.g. "when the user is frustrated, don't just apologize — switch to an alternative workflow" or "when the user confirms/approves, author a new workflow for similar future jobs").
- **Sandboxed procedures** — a workflow can be a full runbook: set up a scoped working environment (temp directory, tool hardlinks, project docs, shell env), do work, run validation, resync results back only if validation passes. Compaction (§9) is one instance of this pattern, not a special case Squire needs to know about.

**Squire's role is purely to make workflow tokens discoverable.** It does not interpret, enforce, or validate workflow content. Enforcement of anything a workflow specifies (security checks, quality gates, resync conditions) is the AI's responsibility, following the workflow's own instructions.

### Multi-agent / persona / orchestration pattern

There is no "agent mode" or subagent primitive implemented in Squire. The apparent multi-agent behavior described in this design (a master workflow matching user intent to a worker workflow, "launching" a scoped worker, cancelling and swapping on dissatisfaction) is **entirely a convention living in workflow-document content**, not architecture:

- "Launching a subagent" = the same AI, under a turn/session boundary the workflow document defines by convention, operating under a narrower workflow scope.
- "Reporting back" = whatever channel the workflow author chooses (a file write, a token write, a return value) — not a Squire-mediated mechanism.
- "Swapping workflows on failure" = the master workflow's own instructions (e.g. "try the next of the top-3 matched workflows") — no Squire-level re-matching logic.

Example master workflow (illustrative, author-defined):
> "Match user intention against top 3 workflows. Launch the first. Pass user intent to it. Have it return upon work completion or user frustration. On return, write a summary of the return reason to file X."

This is not special-cased by Squire in any way — it is ordinary workflow content that the AI follows using the same primitives as everything else.

**Self-authoring of new workflows** (an AI crystallizing a successful interaction pattern into a new, ingestible workflow token) only occurs if a workflow explicitly instructs it to. Squire has no default/innate authoring behavior — nothing happens unless a workflow drives it.

---

## 9. Compaction (`/compact`)

Deferred to future detailed design, but the execution model is settled:

- **Trigger**: manual invocation for MVP; may later be workflow-governed or automatic (e.g. token-count threshold).
- **Mechanics**: compaction is an instance of the general sandboxed-procedure pattern (§8), not bespoke Squire machinery:
  1. `/compact` invoked → **freeze** the live session DB (no concurrent turn processing / writes during compaction — avoids merge-conflict logic entirely).
  2. Copy frozen DB to a ramdisk working copy.
  3. `git init` + commit (baseline snapshot).
  4. AI runs a compaction script against the working copy, using a DB-primitive CLI/API (query, update, delete, merge), guided by whatever cleanup policy a workflow document specifies (e.g. remove tokens proven wrong/negated by user correction, delete unreferenced orphan tokens, merge near-duplicate concept tokens, optionally delegate to a different model for the cleanup pass).
     - Git commits incrementally; revertible mid-procedure if a step goes wrong.
  5. Validation gate — whatever the workflow specifies (schema checks, sanity counts, a second AI reviewing the diff, etc.).
  6. If validation passes: sync working copy back over the live DB (safe — nothing else touched it during the freeze).
  7. Destroy ramdisk.
  8. Unfreeze → resume normal turn processing.
- **Safety model**: no bespoke safety machinery beyond standard layers already available to any AI with shell access — git revert, OS-level protections, backups. A flawed compaction workflow document is treated as an authoring bug to be caught by testing the document before trusting it, not a condition Squire structurally prevents. This is consistent with Squire's overall non-role as a security enforcer (see §10).

---

## 10. Explicitly Deferred (Known Open Items, Not MVP-Blocking)

These are acknowledged gaps, intentionally left unresolved for MVP:

1. **Namespace scoping beyond session.** MVP is session-scoped only, flat. Future extension envisioned via prefix conventions (`Global::`, `Session::`, `Workspace::`) allowing `explore`/`rdf` to span multiple stores — not implemented now.
2. **Provenance / trust labeling on tokens.** No structural distinction currently between user-authored, AI-authored, tool-defined, or externally-ingested-and-untrusted content. Injection-defense (e.g. scanning external content before ingestion) is left entirely to workflow policy, not enforced structurally by Squire. This is a known security gap, deferred deliberately — Squire's stated role is orchestration, not security enforcement (same reasoning as §9's safety model).
3. **Referential/relationship token invalidation on source edit.** If a source token is edited, re-ingested, or deleted, referential and relationship tokens pointing into it may dangle. No versioning/invalidation strategy defined yet.
4. **Concept token deduplication outside compaction.** No dedup occurs during normal turn processing (formatter mints freely); the only cleanup path is the deferred `/compact` procedure.
5. **Multi-workflow composition/conflict arbitration.** When multiple workflows match a single context simultaneously (e.g. persona + emotional-state + domain workflows), resolution is left to workflow content itself (e.g. a meta-workflow establishing precedence) — Squire performs no arbitration.

None of these block a working MVP; they represent areas where the current answer is "solved later, by the same mechanism (more workflow content), not by new Squire architecture."

---

## 11. Summary: What Squire Actually Provides

Stripped to its essence, Squire provides exactly:

- **One store** — a uniform token structure (source, referential, concept, relationship) holding knowledge, workflows, and tools alike.
- **One primitive** — discovery (`explore` + `rdf`), composed via a narrow batch/pipe syntax.
- **One optimization layer** — prefetch + formatter carry-forward assembling short/long context lists, provably non-load-bearing for correctness.
- **One recency signal** — `hitcount`/`activeScore`, used only as a relevance tiebreak.

Everything else observed in this design — personas, multi-agent orchestration, self-authoring workflows, sandboxed compaction — is emergent behavior arising from workflow *content* discovered through that one primitive, not from additional Squire-level mechanics. Complexity in this system is meant to live in what gets ingested, not in what Squire's core has to implement.
