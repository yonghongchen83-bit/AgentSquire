# Debugging/Testing — Workflow Definition

`WF_DebugTesting` introduces no new token type and no parallel vocabulary. A
bug investigation is a Decision Tree (see `decision_tree_workflow_skill.md`,
`WF_DecisionTree` / `SKILL_DecisionTree`) whose evidence-gathering step
happens to route through Subagent Dispatch (`WF_UseSubagent` /
`SKILL_Subagent_Dispatch`, same file). This document only adds what neither
base pattern already covers: capturing raw observations *before* a tree
exists, one additional predicate for confirmed (not just invalidated)
assumptions, and which base procedure applies at each step.

**Dependency:** requires `WF_DecisionTree`, `SKILL_DecisionTree`,
`WF_UseSubagent`, and `SKILL_Subagent_Dispatch` to already be registered as
tokens. This workflow does not restate their behavioural patterns — it
composes them.

**Additional predicate required** (add to the base vocabulary if not already
present):

| Predicate | Subject → Object | Meaning |
|---|---|---|
| `confirmedBy` | `CONCEPT_Assumption_x → TRT_evidence` | The concrete evidence that supported the assumption. Mirrors `invalidatedBy`; without this, a tested-and-holding assumption is graph-indistinguishable from one nobody has checked yet. |

---

## WF_DebugTesting (Workflow token, `full_desc`)

**When to select this workflow:** the user reports something broken,
failing, or behaving unexpectedly, and the cause is not immediately obvious
from the report alone. If the fix is obvious (a typo, a stack trace pointing
at one line), there's no genuine fork to decide between — just fix it. Same
threshold as the base `WF_DecisionTree`: only use this when more than one
plausible cause exists.

**Behavioural pattern:**

1. **Create the root node first.** `CONCEPT_DT_Bug_<slug>`, `short_desc` =
   the symptom as reported, not a guessed cause. Same naming rule as
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

5. **On the result, follow two symmetric paths:**
   - **Holds** → `CONCEPT_Assumption_x confirmedBy TRT_evidence`, then either
     drill deeper (new `CONCEPT_DT_` child under the current one, back to
     step 3) or, if this fully accounts for the symptom, proceed to step 7.
   - **Breaks** → the base skill's backtrack procedure verbatim:
     `invalidatedBy` → `abandoned` → reselect at the parent with a new
     assumption. Per `SKILL_Subagent_Dispatch`'s failure handling, an
     inconclusive or failed subagent run is itself valid evidence for
     `invalidatedBy` — it doesn't need to be treated as a no-op retry.

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
(step 1's naming), the pre-tree fact capture (step 2), and the fixed pairing
of evidence-gathering with subagent dispatch (step 4). Everything else is
`SKILL_DecisionTree` and `SKILL_Subagent_Dispatch` applied, not reinvented.

### When not to use this pattern

If the fix is obvious from the report itself, there's no fork to decide
between — just fix it. Reserve this workflow for cases where more than one
plausible cause exists and the first move is genuinely uncertain.

### Worked example

```
CONCEPT_DT_Bug_NullOnCheckout       (root; short_desc: "checkout throws null
                                      ref on step 3 for some users")

TRT_ErrorLog_1        observedIn    CONCEPT_DT_Bug_NullOnCheckout
TRT_ReproSteps        observedIn    CONCEPT_DT_Bug_NullOnCheckout

CONCEPT_DT_Bug_NullOnCheckout   considers   CONCEPT_DT_StaleCartCache
CONCEPT_DT_Bug_NullOnCheckout   considers   CONCEPT_DT_RaceOnSessionInit
CONCEPT_DT_Bug_NullOnCheckout   considers   CONCEPT_DT_MissingFieldNewSchema

CONCEPT_DT_Bug_NullOnCheckout   selects     CONCEPT_DT_MissingFieldNewSchema
CONCEPT_DT_MissingFieldNewSchema  drivenBy  CONCEPT_Assumption_SchemaMigrationIncomplete
   -- picked first: cheapest to check (grep for field usage), highly
      narrowing (rules in/out an entire category of causes at once)

-- dispatch subagent: grep for field access + check migration status --

TRT_MigrationCheckResult   (evidence)
CONCEPT_Assumption_SchemaMigrationIncomplete   confirmedBy   TRT_MigrationCheckResult

CONCEPT_DT_MissingFieldNewSchema   resolves   CONCEPT_DT_Bug_NullOnCheckout

TRT_Fix_NullOnCheckout            fixes     CONCEPT_DT_Bug_NullOnCheckout
TRT_Verification_NullOnCheckout   verifies  TRT_Fix_NullOnCheckout
```

Note `CONCEPT_DT_StaleCartCache` and `CONCEPT_DT_RaceOnSessionInit` are left
as `considers`-only siblings — never selected, never disproven, just
recorded as options that existed. If this bug resurfaces differently later,
`explore("memory", "CONCEPT_DT_Bug_NullOnCheckout", 2)` surfaces them as
untried alternatives rather than forcing a re-derivation from scratch.
