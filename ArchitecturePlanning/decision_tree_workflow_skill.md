# Decision Tree — Workflow and Skill Definitions

Session-scoped. No cross-session persistence — the tree lives and dies with the
session store, consistent with Section 16 (deferred) of the spec. If cross-session
continuity is implemented later, this pattern needs no changes: the root node
simply becomes reachable across sessions instead of starting fresh.

---

## WF_DecisionTree (Workflow token, `full_desc`)

**When to select this workflow:** the user presents a problem with more than one
plausible approach — a bug with an unknown location, a design with multiple
viable structures, any task where the first move is genuinely uncertain rather
than a known procedure.

**Behavioural pattern:**

1. **Do not answer immediately.** First externalise the decision space: what are
   the distinct approaches available right now, before committing to any of them.
2. **Create a root decision node** (`CONCEPT_DT_<slug>`) whose `short_desc` states
   the problem being decided, not the eventual answer.
3. **Enumerate every option you're aware of as a `considers` relationship**, even
   the ones you're about to reject. A path not taken is still worth recording —
   it tells a future reader (or yourself, backtracking) what was already ruled
   out and why, so it isn't re-explored.
4. **Commit to exactly one option per open decision** via a `selects`
   relationship, and pair it with a `drivenBy` relationship to an explicit
   assumption token. Never select without recording the assumption — an
   unjustified selection can't be diagnosed later when it turns out wrong.
   Weigh considered options against the branch selection criteria (see skill)
   before choosing — the assumption token's description should make clear
   which criteria tipped the decision, not just what the decision was.
5. **Act on the selected branch.** If the branch itself contains a further
   decision (e.g. "read source" then splits into "check module A first" vs
   "check module B first"), create a new `CONCEPT_DT_` child node under it and
   repeat from step 3. The tree grows exactly as deep as real uncertainty goes —
   don't manufacture branches for steps that only have one reasonable move.
6. **When an assumption breaks**, do not delete or silently overwrite anything.
   Follow the backtrack procedure (see skill) to close the current branch and
   reopen the parent decision with a new selection.
7. **When a branch resolves the original problem**, mark it explicitly so the
   tree has a clear terminus, and preserve the root node and the winning path
   (not the abandoned ones) going into the next turn if the task continues.

**Interaction with `preserve`:** preserve the current open decision node(s) — the
ones actively being worked, not resolved or abandoned ones. Abandoned branches
should be reachable via `explore()` if needed, but don't consume bootstrap
budget by default.

---

## SKILL_DecisionTree (Skill token, markdown instruction set)

### Token conventions

No new token type is introduced. Everything is a standard `concept` or
`referential` token; the pattern lives entirely in naming and relationships.

| Token | Naming | Represents |
|---|---|---|
| Decision node | `CONCEPT_DT_<slug>` | A point where more than one approach exists. `short_desc` = the question, not the answer. |
| Assumption | `CONCEPT_Assumption_<slug>` | The belief that justifies a specific selection. Always exactly one per `selects` edge. |
| Evidence / outcome | `TRT_<slug>` (via `§^`) | Actual content — what you found, read, or observed. This is what assumptions get validated or invalidated against. |

### Relationship vocabulary

| Predicate | Subject → Object | Meaning |
|---|---|---|
| `considers` | `CONCEPT_DT_parent → CONCEPT_DT_child` | Option was identified. Does not imply it was chosen. |
| `selects` | `CONCEPT_DT_parent → CONCEPT_DT_child` | This is the currently active path. A parent may have multiple `selects` edges over time (see backtracking) — the live one is whichever child hasn't been `abandoned`. |
| `drivenBy` | `CONCEPT_DT_child → CONCEPT_Assumption_x` | The assumption that justified this selection. Mandatory on every `selects`. |
| `invalidatedBy` | `CONCEPT_Assumption_x → TRT_evidence` | The concrete evidence that broke the assumption. |
| `confirmedBy` | `CONCEPT_Assumption_x → TRT_evidence` | The concrete evidence that supported the assumption. Mirrors `invalidatedBy`; without this, a tested-and-holding assumption is graph-indistinguishable from one nobody has checked yet. |
| `abandoned` | `CONCEPT_DT_child → CONCEPT_Assumption_x` | Marks a previously-selected branch as no longer active, pointing at the assumption that failed. |
| `resolves` | `CONCEPT_DT_child → CONCEPT_DT_root` | Marks a branch as the terminus that actually solved the original problem. |

Relationships are freeform per the base spec — this is a convention, not an
enforced schema. Consistency across a session is what keeps traversal useful.

### Branch selection criteria

When more than one considered option is viable, weigh them against these four
factors, in this order of precedence when they conflict:

1. **Confidence of success** — how likely is this approach to actually resolve
   the problem, given what's currently known? Prefer the option you have the
   most concrete reason to believe will work over one that's merely plausible.
2. **Minimal effort/time** — among options with comparable confidence, prefer
   the one that costs less to try. Cheap-to-test approaches should generally
   be attempted before expensive ones, since a wrong cheap guess costs little.
3. **Least destructive** — prefer approaches that are easy to undo or that
   don't foreclose other options if wrong. A reversible experiment beats an
   irreversible commitment when confidence is otherwise similar. This matters
   most as a tiebreaker between options of similar cost and confidence.
4. **Most narrowing effect** — when confidence is genuinely low and several
   options are roughly equal on the above, prefer whichever option eliminates
   the largest share of the remaining possibility space regardless of outcome
   — i.e. it's informative even when it "fails." This is the diagnostic
   move: not "will this fix it" but "will this tell me the most either way."

These are precedence rules, not a strict lexicographic formula — use judgment
when factors trade off against each other (e.g. a slightly slower option that
is far more narrowing may still win). What matters is that the reasoning gets
recorded: the `drivenBy` assumption token's `short_desc`/`full_desc` should
name which criterion or criteria drove the choice, e.g. *"chosen for low
cost and reversibility; confidence was comparable to the alternative"*. This
is what makes a later backtrack diagnostic rather than just a dead end — the
next selection at the same parent can explicitly reason about which criteria
the previous attempt under-weighted.

### Confirming without resolving

Not every `drivenBy` assumption that holds up is the root cause — it may just
justify drilling one level deeper (step 5: create a child `CONCEPT_DT_`
node). When evidence supports an assumption but the branch isn't yet the
terminus:

1. Mark the evidence with `§^` if it isn't already a token.
2. Write `CONCEPT_Assumption_x confirmedBy TRT_evidence`.
3. Proceed to step 5 of the main pattern (create the child decision node)
   rather than looping back to `considers` at the same level.

This keeps "checked and holds" distinct from "untested" in the graph, and
gives `resolves` (below) something concrete to point back to when a later
reader asks why the terminus branch was believed correct.

### Backtrack procedure

When a selected branch's assumption breaks:

1. Mark the evidence that broke it with `§^` if it isn't already a token —
   this is what `invalidatedBy` needs to point to. Don't skip this: "I was
   wrong" without the evidence is not diagnosable later.
2. Write `CONCEPT_Assumption_x invalidatedBy TRT_evidence`.
3. Write `CONCEPT_DT_child abandoned CONCEPT_Assumption_x`.
4. Return to `CONCEPT_DT_parent`. Re-examine the original `considers` list —
   is there an already-identified option to try next, or does this new
   evidence surface an option that wasn't visible before? Either way:
5. Write a new `CONCEPT_DT_parent selects CONCEPT_DT_newchild` and a new
   `drivenBy` assumption for it.

The parent node now has two `selects` edges. This is intentional, not a
cleanup problem: `explore("memory", ..., num_hops=2)` from the parent
surfaces both, and the `abandoned` edge disambiguates which is live. Hit-count
decay (Section 3.3 of the spec) does the rest — the abandoned branch stops
accumulating references and drifts down in ranking naturally.

### Worked example (from the debugging case)

```
CONCEPT_DT_LocateBug            considers   CONCEPT_DT_ReadSource
CONCEPT_DT_LocateBug            considers   CONCEPT_DT_SetBreakpoint
CONCEPT_DT_LocateBug            considers   CONCEPT_DT_AssertCondition
CONCEPT_DT_LocateBug            selects     CONCEPT_DT_ReadSource
CONCEPT_DT_ReadSource           drivenBy    CONCEPT_Assumption_ReadSourceFast

-- assumption breaks --

CONCEPT_Assumption_ReadSourceFast  invalidatedBy  TRT_CodeMoreComplexThanExpected
CONCEPT_DT_ReadSource              abandoned      CONCEPT_Assumption_ReadSourceFast
CONCEPT_DT_LocateBug               selects        CONCEPT_DT_SetBreakpoint
CONCEPT_DT_SetBreakpoint           drivenBy       CONCEPT_Assumption_NeedRuntimeEvidence

-- bug found --

CONCEPT_DT_SetBreakpoint           resolves       CONCEPT_DT_LocateBug
```

Reading this graph back at any point tells you: what was tried, what was
ruled out and why, what's currently active, and what actually worked — without
needing a separate "status" field or timeline anywhere.

### When not to use this pattern

Don't create a decision node for a step with only one reasonable move. The
tree should reflect genuine forks, not every action taken. If in doubt: if you
wouldn't be able to articulate a second option someone might reasonably have
chosen instead, it's not a decision node.

---

# Subagent Dispatch — Workflow and Skill Definitions

No protocol or schema change required. A subagent is a `TOOL_` token like any
other — `invoke()` proxies the call and returns a structured result, exactly
as Section 6.3 and Section 13.2 already describe. The only new ground covered
here is behavioural: when Main AI should reach for a subagent, how it should
package the handoff, and what happens to what comes back.

Sequential-only for now: one `invoke()` at a time, synchronous, per Section
9.2 as written. Concurrent dispatch (multiple subagents at once) would need a
small protocol extension — a batch `invoke()` — but that's explicitly deferred
until there's a real usage pattern to justify it.

## WF_UseSubagent (Workflow token, `full_desc`)

**When to select this workflow:** a subtask has emerged that is (a) boundable
with a clear success/return contract, and (b) either noisy to execute inline,
better suited to a different model, or independent enough from the live
working context that it doesn't need what's currently loaded.

**Behavioural pattern:**

1. **Recognise a delegation candidate**, not just "any tool call." A single
   `readfile`/`webfetch` doesn't need a subagent. A multi-step exploration
   whose intermediate reasoning would just be noise in the main context does.
2. **Package a self-contained task.** The subagent inherits nothing from
   `prefetched_tokens`/`preserved_tokens` automatically. Explicitly state
   everything it needs to know in the task payload, and state what a
   successful result looks like (format, scope, what "done" means). If you
   can't write that contract concretely, the task isn't ready to delegate —
   go back to reasoning about it directly instead.
3. **Invoke and wait.** Sequential for now — one subagent call, get its
   result, decide the next move before dispatching another.
4. **Treat the result as external-world content by default.** Per Section
   12.2, it's ephemeral unless you explicitly `§^`-mark it. Most subagent
   results are a means to an end within the current turn and don't need to
   become memory — only mark and connect the ones with future retrieval
   value (a conclusion you'll want to recall, not the search process that
   found it).
5. **If this was gathering evidence for a Decision Tree branch**, feed the
   result in as the `TRT_` evidence token and continue that workflow's
   backtrack/resolve procedure normally — subagent dispatch is just the
   mechanism that produced the evidence, not a separate tracked structure.

## SKILL_Subagent_Dispatch (Skill token, markdown instruction set)

### Token conventions

| Token | Naming | Represents |
|---|---|---|
| Subagent tool | `TOOL_Subagent_<capability>` | A tool token whose `full_desc` is a standard MCP schema, discovered via `explore("tool", ...)` like any other tool. Nothing subagent-specific in the schema format itself. |

No new token type. The "subagent" concept exists only in naming convention
and in the behavioural guidance above — the Squire treats it identically to
any other tool token.

### Packaging checklist

Before calling `invoke()` on a subagent tool, confirm:

- **Self-contained task description** — doesn't assume the subagent can see
  anything from the current session's memory graph.
- **Explicit return contract** — what shape should the result come back in,
  and what counts as success vs failure vs partial result.
- **No memory-graph responsibility handed off** — the subagent does not
  create tokens or relationships itself. Only Main AI writes to the graph,
  consistent with Section 11 ("Squire never builds relationships") extended
  here to: subagents never build relationships either. They return data;
  Main AI decides what, if anything, becomes memory.

### Result handling

Default: ephemeral, discarded at turn end, same as any tool result.

To retain: `§^`-mark the relevant portion of your own response summarizing
the subagent's finding (not the subagent's raw output verbatim), and connect
it via relationships to whatever concept token the task relates to. Optional
provenance relationship if useful:

```
TRT_evidence   gatheredBy   TOOL_Subagent_CodeSearch
```

This is optional, not mandatory — only add it if provenance (which subagent
produced this) is likely to matter for interpreting the evidence later.

### Failure handling

A subagent that fails, times out, or returns an inconclusive result is itself
evidence. If dispatched to gather evidence for a Decision Tree branch, an
inconclusive result can validly `invalidatedBy` the driving assumption
("assumed the subagent could isolate the failure; it couldn't narrow it down"
is a legitimate reason to backtrack), rather than being treated as a no-op.

### When not to use this pattern

Don't delegate a task you can't specify a success contract for — those need
the ongoing judgment of direct reasoning, not a one-shot dispatch. And don't
delegate small single-tool-call tasks; the packaging overhead isn't worth it
for anything a direct `invoke()` or `explore()` already handles in one step.

---

# Debugging/Testing — Workflow Definition

`WF_DebugTesting` introduces no new token type and no parallel vocabulary. A
bug investigation is a Decision Tree whose evidence-gathering step happens to
route through Subagent Dispatch. This section only adds what neither base
pattern already covers: capturing raw observations *before* a tree exists,
and picking which workflow's procedure applies at each step.

## WF_DebugTesting (Workflow token, `full_desc`)

**When to select this workflow:** the user reports something broken,
failing, or behaving unexpectedly, and the cause is not immediately obvious
from the report alone.

**Behavioural pattern:**

1. **Create the root node first.** `CONCEPT_DT_Bug_<slug>`, `short_desc` =
   the symptom as reported, not a guessed cause. Same naming rule as the base
   `WF_DecisionTree`.

2. **Capture raw observations before any hypothesis exists.** A bug report
   usually arrives with facts that predate any decision — error text, repro
   steps, what was expected vs. what happened. `§^`-mark each as its own
   `TRT_<slug>` immediately, with:
   ```
   TRT_<slug>   observedIn   CONCEPT_DT_Bug_<slug>
   ```
   Do not wait for step 3 (`considers`) to exist before capturing these —
   they're evidence available to whichever hypotheses get proposed next.
   This is the one addition the base Decision Tree pattern doesn't cover,
   since it's written for decisions in general, not specifically for a
   process that starts with a pile of symptoms.

3. **Run `WF_DecisionTree` from here, unmodified.** Propose candidate
   explanations as sibling `CONCEPT_DT_` children via `considers`. Select
   one via `selects` + `drivenBy CONCEPT_Assumption_x`, weighed against the
   four branch selection criteria in `SKILL_DecisionTree` — confidence,
   effort, reversibility, narrowing effect, in that precedence order. The
   narrowing criterion *is* bisection: prefer the test that cuts the
   surviving hypothesis set roughly in half over the one that merely feels
   most likely.

4. **Gather the evidence the assumption predicts via `WF_UseSubagent`.**
   Package the assumption's claim directly as the subagent's success
   contract — e.g. "confirm or deny that the failure reproduces when
   condition X holds." Follow that workflow's packaging checklist: a
   self-contained task description, an explicit return contract, no memory-
   graph responsibility handed to the subagent.

5. **On the result, follow the base skill's two symmetric paths:**
   - **Holds** → `CONCEPT_Assumption_x confirmedBy TRT_evidence`, then either
     drill deeper (new `CONCEPT_DT_` child under the current one, back to
     step 3) or, if this fully accounts for the symptom, proceed to step 7.
   - **Breaks** → the base skill's backtrack procedure verbatim:
     `invalidatedBy` → `abandoned` → reselect at the parent with a new
     assumption. Per `SKILL_Subagent_Dispatch`'s failure handling, an
     inconclusive or failed subagent run is itself valid evidence for
     `invalidatedBy` — it doesn't need to be a no-op retry.

6. **Repeat 3–5** until a branch accounts for the symptom with confirmed
   evidence, not just an unbroken assumption.

7. **Mark the terminus** with `resolves`, exactly as the base skill
   describes. Then, outside the decision tree proper:
   ```
   §^TRT_Fix_<slug>            the actual change made           §^
   §^TRT_Verification_<slug>   test run confirming resolution   §^
   ```
   with relationships `TRT_Fix_<slug> fixes CONCEPT_DT_Bug_<slug>` and
   `TRT_Verification_<slug> verifies TRT_Fix_<slug>`. Fix and verification
   stay distinct tokens so a later regression gets a new verification token
   without rewriting the original.

**Interaction with `preserve`:** carry forward `CONCEPT_DT_Bug_<slug>` and
the currently-open decision node(s), same rule as the base workflow.
Abandoned branches and superseded evidence stay reachable via `explore()`
but shouldn't consume bootstrap budget by default.

**Why this is a composition and not a new pattern:** a debugging session and
an unrelated decision (say, "which library to use") should use the *same*
relationship vocabulary, or graph traversal stops generalizing across
domains. The only debugging-specific things here are the trigger condition
(step 1's naming), the pre-tree fact capture (step 2), and the fixed
pairing of evidence-gathering with subagent dispatch (step 4). Everything
else is `SKILL_DecisionTree` and `SKILL_Subagent_Dispatch` applied, not
reinvented.

### When not to use this pattern

If the fix is obvious from the report itself (a typo, a clear stack trace
pointing at one line), there's no genuine fork to decide between — just fix
it. Reserve this workflow for cases where more than one plausible cause
exists and the first move is genuinely uncertain, same threshold as the base
`WF_DecisionTree`.
