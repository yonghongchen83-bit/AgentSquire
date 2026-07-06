# Context Squire — Addendum: Process Trees

**Status:** Extends `context_squire_spec_v2.md`. Adds no new storage technology —
built entirely on the existing token/relationship substrate (Section 3–4 of
the base spec). This addendum defines one new reserved predicate, one new
computed status rule, and one new protocol field.

**Purpose:** Two observed AI failure modes motivate this addendum, and each
tree targets a different one:

- **Reasoning drift** — the AI fixates on one hypothesis, chases an
  edge-case gamble instead of the most-narrowing test, or re-tries something
  already ruled out without noticing. Addressed by `WF_DecisionTree` /
  `SKILL_DecisionTree` (already specified) — the tree makes exploration
  legible so re-treading ground is visible as backtracking, not a fresh idea.
- **Objective drift** — the AI loses the thread of what it was doing
  entirely, typically after burning a lot of tokens fighting a sub-problem,
  and starts hallucinating or free-associating instead of resuming the task.
  Addressed by the **Todo Tree**, defined below.

Both are rendered as trees to the human because the human's only lever is
observation and instruction ("hey, your list isn't done" / "you're
re-trying something you already ruled out") — not direct manipulation. The
aim is for the AI to complete the task with minimal intervention; the
render exists so a human can *notice* when that aim isn't being met.

---

## A. Todo Tree

### A.1 Semantics

A Todo Tree is a mandatory decomposition of work, not a record of
uncertainty. Every child is required, not an alternative — this is the
structural opposite of Decision Tree's `considers` (many proposed, one
`selects`ed). A parent node's job is done only when the parent's own work is
done *and* every child's job is done.

This deliberately rejects the flat todo-list model. Real engineering work
nests: closing a bug involves reproducing it, isolating it, fixing it, and
verifying the fix — and isolating it may itself decompose further once the
AI learns something mid-task. A flat list can't represent that a subtask
was discovered halfway through, or that "done" at one level depends on three
open items at another.

### A.2 Token conventions

No new token type. A todo node is a `concept` token following naming
convention `TODO_<slug>`, distinguished from a Decision Tree node purely by
which predicate connects it to its children.

| Token | Naming | Represents |
|---|---|---|
| Todo node | `TODO_<slug>` | A unit of required work. `short_desc` = the task, stated as an action. |

### A.3 Reserved predicate: `subtask`

```
subtask   TODO_parent → TODO_child   The child is required for the parent to be considered complete.
```

`subtask` is **reserved**, per the same principle as `Summary` / `Intent` /
`Trigger` in the base design: reserve only what the Squire itself must act
on mechanically. `considers` / `selects` / `drivenBy` etc. are conventions
the Squire never inspects — it just stores them. `subtask` is different:
the Squire evaluates it every time it computes a node's displayed status.
That makes it the one relationship in this addendum the Squire actually
reads.

### A.4 Status is computed, not stored

Do **not** store a mutable `status: open|done` field that the Squire
flips. Instead:

```
stored:    self_marked_done(node)   boolean, set by the AI, defaults false
computed:  is_done(node) = self_marked_done(node)
                            AND all(is_done(child) for child in subtask_children(node))
```

This is the same pattern as `effective_priority` in the base spec (Section
3.3): store the raw fact, derive the rest lazily at read time. It has one
important consequence you specifically asked for: **a child can be added
under an already-"done" node, and the parent correctly reopens with no
migration or cascading update.** The parent's `self_marked_done` never
changes; `is_done` just recomputes to false the next time anything reads it,
because the new child evaluates false. There is no "reopening" operation to
get wrong, because there was never a terminal state to begin with — closure
was always a computed view of the leaves, not a fact written at the parent.

**Consequence for the AI:** marking a node `self_marked_done` is not a
privileged, Squire-validated operation the way an earlier draft of this
addendum considered (a write-time gate: "reject if children are open").
That gate turned out to be unnecessary once status is computed rather than
stored — there's nothing to protect against, since a premature
`self_marked_done` on a parent with open children simply has no visible
effect until the children close too. The AI can mark it whenever it
believes its own portion of the work is finished; the tree's displayed
completion state reflects reality regardless of what the AI claims about
ancestors.

### A.5 Relationship to Decision Tree

A Todo leaf frequently needs investigation before it can be marked done —
that's where Decision Tree composes in, exactly as `WF_DebugTesting` already
composes `WF_DecisionTree` with `WF_UseSubagent`:

```
predicate: investigatedVia   TODO_child → CONCEPT_DT_root
```

Optional, written when the AI actually opens a Decision Tree to resolve a
todo leaf rather than just doing it directly. This keeps the two structures
cleanly separated by concern — Todo Tree tracks *the shape of the work*,
Decision Tree tracks *the shape of the reasoning inside one node of that
work* — while remaining traversable as one graph.

### A.6 Rendering

Both trees render through the same client-side component. The component
needs only a generic node shape:

```json
{ "id": "TODO_IsolateSchema", "label": "...", "badge": "open", "children": [...] }
```

produced by walking from a root using a caller-supplied edge predicate and a
caller-supplied status-to-badge mapping:

| Tree | Edge predicate walked | Badge source |
|---|---|---|
| Todo Tree | `subtask` | `is_done(node)` → done / open |
| Decision Tree | `considers` (all), with `selects` marking the live edge and `abandoned` greying it out | resolved / active / abandoned / considered-only |

The renderer doesn't need to know *why* the trees differ semantically — it
needs one predicate name and one badge function per tree type. This keeps
the human-facing surface consistent (it's always "a tree with badges") while
the underlying semantics — mandatory-AND vs exploratory-OR-with-history —
stay exactly as different as they actually are.

---

## B. `active_process_state` — standing injection

### B.1 Why this is a third field, not an extension of `preserve`

`preserved_tokens` (base spec §8.1, §12.3) already lets the AI carry
context forward — but it depends on the AI *thinking to preserve it*. The
specific failure this addendum targets is the AI not thinking to look:
deep in a rabbit hole, burning tokens on a sub-problem, it forgets the
objective and starts hallucinating rather than resuming. If recovering the
objective requires the AI to remember to ask for it, the mechanism fails
exactly when it's needed most.

So this is not scored, not subject to the bootstrap token limit trim, and
not something the AI opts into via `preserve`. It is unconditionally
injected into every request while any Todo Tree or Decision Tree has open
nodes — the same "always appears regardless of score" guarantee
`preserved_tokens` gives explicit carry-forwards, but automatic rather than
AI-initiated.

### B.2 Format

Added to the request JSON (base spec §8.1) as a sibling of
`prefetched_tokens` and `preserved_tokens`:

```json
{
  "active_process_state": {
    "todo_root": "TODO_DebugCheckout",
    "open_leaves": ["TODO_ReproduceLocally", "TODO_IsolateSchema"],
    "dt_root": "CONCEPT_DT_Bug_NullOnCheckout",
    "last_decision": {
      "node": "CONCEPT_DT_MissingFieldNewSchema",
      "assumption": "CONCEPT_Assumption_SchemaMigrationIncomplete",
      "status": "unconfirmed"
    }
  }
}
```

- `todo_root` / `dt_root`: present only while at least one descendant is
  open / unresolved. Absent (field omitted or null) once fully closed.
- `open_leaves`: computed via `is_done` (§A.4) — the current frontier of
  incomplete work, not the whole tree. Keeps the reminder short even for a
  deep tree.
- `last_decision`: the most recently made `selects` + `drivenBy` pair not
  yet followed by `confirmedBy`/`invalidatedBy`. This is specifically for
  the "forgot what assumption it was testing" case — handed to the AI
  directly rather than requiring an `explore()` call it may not think to
  make while distracted.

### B.3 Squire responsibilities (addition to base §11)

**Turn open, in addition to existing steps:**
- If any `TODO_` or `CONCEPT_DT_` root exists with unresolved descendants,
  compute and inject `active_process_state`. Pure traversal — no scoring,
  no reasoning, no judgement about whether the AI is "on track." The
  Squire surfaces state; it never evaluates progress.

**Still never:**
- Squire does not decide a todo is taking too long, does not force a
  branch selection, does not refuse a `self_marked_done` write. All
  navigation judgement stays with the Main AI, per the base spec's
  intelligence boundary (§1, §11). This addendum only widens *what the
  Squire mechanically surfaces*, not what it decides.

### B.4 Human-facing role

The rendered tree (§A.6) is read-only for the human by design. The
intended intervention path is conversational — "your todo list isn't
complete," "you already tried that branch" — not direct tree editing. The
`active_process_state` injection is meant to make that intervention
unnecessary in the common case, by giving the AI the same reminder a human
would otherwise have to supply manually.

---

## C. Summary of additions to base spec

| Base spec section | Addition |
|---|---|
| §3 (Token Model) | No new token type. `TODO_` naming convention added alongside `CONCEPT_`/`TRT_`. |
| §4.2 (Triplet Store) | New reserved predicate `subtask`, read mechanically by Squire. `investigatedVia` added as a convention (not reserved) linking Todo → Decision Tree. |
| §3.3 / lazy computation pattern | `is_done(node)` computed identically in spirit to `effective_priority` — stored fact is append-only/monotonic-ish (`self_marked_done`), displayed value derived at read time. |
| §8.1 (Request Format) | New field `active_process_state`, unconditional, unscored, sibling to `prefetched_tokens`/`preserved_tokens`. |
| §11 (Squire Responsibilities) | Addition: compute and inject `active_process_state` at turn open when process trees are active. Explicitly still never judges or enforces progress. |
| Rendering (new, client-side) | One generic tree component; predicate + badge-mapping supplied per tree type (`subtask` vs `considers`/`selects`/`abandoned`). |
