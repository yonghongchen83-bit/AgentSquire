# AiControl Node System (Static Files Mode, No MCP)

## 1) Purpose
This document defines a generic node-based working model for coding agents when MCP is unavailable.

A node is a scoped unit of work. Each node stores its own context, progress, and todos in plain text files so any agent can resume work deterministically.

---

## 2) Core Principles
1. One active node at a time.
2. Child nodes inherit parent context.
3. Child files can override inherited assumptions.
4. Durable facts go to environment notes.
5. Ongoing reasoning and decisions go to state notes.
6. Action items are tracked in a todo file with explicit status.
7. Agents must read before writing, and write back after meaningful progress.

---

## 3) Generic Folder Structure

Use this exact structure (or a close equivalent):

AICONTROL/
  CURRENT_NODE
  root/
    env.md
    state.md
    todo.md
    snapshots/
  nodes/
    <node-id>/
      meta.md
      env.md
      state.md
      todo.md
      snapshots/
      nodes/
        <child-node-id>/
          meta.md
          env.md
          state.md
          todo.md
          snapshots/

Notes:
- CURRENT_NODE contains the active node path as plain text.
- root is the top-level default node.
- nodes/<node-id>/nodes is for child nodes.
- snapshots can contain timestamped files or folders.

---

## 4) Required Files Per Node

meta.md
- Node identity and governance.
- Suggested fields:
  - Node ID
  - Parent Node
  - Goal
  - Scope In
  - Scope Out
  - Owner (optional)
  - Created At
  - Status (planned | active | blocked | completed | archived)

env.md
- Stable, durable context.
- Include constraints, interfaces, important commands, architecture facts, non-negotiable requirements.
- Avoid transient logs.

state.md
- Current understanding and progress journal.
- Include assumptions, findings, decisions, rationale, blockers, risks, and next actions.
- Append updates in chronological order.

todo.md
- Action list with status.
- Recommended statuses: OPEN, IN_PROGRESS, BLOCKED, DONE, CANCELLED.
- Keep concise and execution-focused.

---

## 5) Active Node Resolution

On every run, agent must:
1. Read AICONTROL/CURRENT_NODE.
2. Resolve that path to a node directory.
3. If missing or invalid, fallback to root and record a warning in root/state.md.

CURRENT_NODE examples:
- root
- nodes/feature-auth
- nodes/feature-auth/nodes/fix-token-refresh

---

## 6) Context Loading Order

When building context for the active node:
1. Load root/env.md then root/state.md.
2. Walk each ancestor node from top to parent:
   - env.md
   - state.md
3. Load active node files:
   - meta.md
   - env.md
   - state.md
   - todo.md

Merge rule:
- Parent-first baseline.
- Child values/decisions override conflicting parent assumptions.
- Conflicts must be logged in active node state.md under a Conflict section.

---

## 7) Write-Back Rules

After meaningful progress, agent must update active node files:
1. state.md
- Add timestamped entry with:
  - What changed
  - Why
  - Evidence (tests, outputs, checks)
  - Decision and consequence

2. todo.md
- Update task status transitions.
- Add newly discovered tasks when needed.

3. env.md
- Update only when a fact is durable and likely relevant to future work.

4. meta.md
- Update status when node phase changes.

---

## 8) Node Lifecycle

1. Create
- Create node folder and required files.
- Set initial Goal, Scope In/Out, and OPEN todos.

2. Activate
- Set AICONTROL/CURRENT_NODE to node path.
- Start work only after context load.

3. Execute
- Keep updates small and frequent in state.md and todo.md.

4. Branch
- If subproblem diverges, create child node under current node nodes folder.

5. Complete
- Ensure todos are DONE or CANCELLED.
- Write closure summary in state.md.
- Set meta status to completed.

6. Archive (optional)
- Move completed node under an archive path, keeping full history.

---

## 9) Handoff Contract (Model-Agnostic)

Any agent taking over must:
1. Read CURRENT_NODE.
2. Load context by defined order.
3. Confirm active assumptions in first output.
4. Continue from top OPEN or IN_PROGRESS todo.
5. Write back state and todo updates before exiting.

This guarantees continuity across different models and tools.

---

## 10) Minimal Templates

### AICONTROL/CURRENT_NODE
root

### meta.md
# Meta
- Node ID: <node-id>
- Parent Node: <parent-path-or-none>
- Goal: <goal>
- Scope In: <what is included>
- Scope Out: <what is excluded>
- Created At: <YYYY-MM-DD HH:MM>
- Status: active

### env.md
# Environment
## Stable Facts
- <fact>

## Constraints
- <constraint>

## Interfaces and Contracts
- <interface>

## Useful Commands
- <command>

### state.md
# State
## Timeline
- <YYYY-MM-DD HH:MM> Started node. Initial assumptions: ...

## Decisions
- <decision>: <rationale>

## Risks
- <risk>

## Next Actions
- <next action>

### todo.md
# Todo
- [OPEN] Define acceptance criteria
- [IN_PROGRESS] Implement core change
- [BLOCKED] Validate external dependency
- [DONE] Reproduce issue

---

## 11) Suggested Status Vocabulary

Node status:
- planned
- active
- blocked
- completed
- archived

Todo status:
- OPEN
- IN_PROGRESS
- BLOCKED
- DONE
- CANCELLED

---

## 12) Rules for Static-Agent Instruction Files

If used inside a static agent instruction file, include these directives:
1. Always resolve active node from CURRENT_NODE first.
2. Never edit files outside active node scope unless explicitly instructed.
3. Prefer child node creation over widening scope.
4. Log every major decision in state.md with timestamp and rationale.
5. Keep todo statuses accurate at all times.
6. On conflicts, document assumption and proceed safely.
7. Before finish, write state and todo updates.

---

## 13) Optional Enhancements

1. snapshots/<timestamp>.md with milestone summaries.
2. decision-log.md if decisions become dense.
3. handoff.md for short, operator-friendly baton passes.
4. validation.md for test evidence and acceptance checks.

---

## 14) Acceptance Checklist

A static node system is healthy if:
1. CURRENT_NODE always points to a real node.
2. Every active node has meta/env/state/todo files.
3. Context can be reconstructed from files only.
4. Another model can continue without hidden memory.
5. Completed nodes contain closure notes and final todo states.
