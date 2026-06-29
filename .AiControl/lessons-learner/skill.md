# Lessons Learner Skill

## Concept

After any significant development session (debugging, feature implementation, test fixing), use this skill to systematically capture lessons learned. The agent reviews the session, identifies root causes, creates structured documentation, and updates the project's knowledge graph so future agents can quickly reference past mistakes and solutions.

## When to Trigger

Run this skill at the end of a session when ANY of these are true:
- You fixed a bug that took >2 attempts to resolve
- You discovered a root cause that wasn't obvious at first
- You spent significant time on something that could have been avoided
- You learned something about the toolchain, framework, or project conventions that isn't documented elsewhere
- The user explicitly says "document this" or "what did we learn"

## Workflow

### Phase 1: Session Review

1. **Review the conversation history** — scan for:
   - Bugs that were fixed (what was the symptom vs root cause?)
   - Dead ends or wrong approaches taken
   - Configuration issues (wrong paths, wrong tool versions, wrong flags)
   - Framework quirks discovered (API changes, unexpected behavior)
   - Process improvements (things to do differently next time)

2. **For each candidate lesson, answer**:
   - What was the symptom? (what did the user see?)
   - What did we initially think was wrong?
   - What was the actual root cause?
   - What was the fix?
   - How can we prevent this in the future?
   - What area does this belong to? (E2E Testing, Rust/IPC, Frontend, Build, etc.)

3. **Filter to unique, non-trivial lessons only** — skip issues that were one-off typos or trivial misconfigurations.

### Phase 2: Create/Update Lesson Pages

4. **Check the lessons index** at `.AiControl/lessons-learned/lessons.md`. Read it to see existing entries and the next available number.

5. **For each new lesson, create a detail page** at `.AiControl/lessons-learned/NNN-slug.md`:

   ```markdown
   # Lesson NNN: Title

   ## Problem
   (What went wrong — describe the user-facing symptom)

   ## Symptoms
   (Error messages, unexpected behavior, test failures)
   
   ```
   
   (Surrounding code blocks or terminal output)
   
   ```
   
   ## Root Cause
   (What was actually causing it — the deeper reason)

   ## Fix
   (What change resolved it — include code snippets or commands)

   ## Prevention
   (How to avoid this in the future — conventions, tests, checks to add)

   ## Related
   (Links to other lessons, node documents, or external resources)
   ```

   Required sections: **Problem**, **Symptoms**, **Root Cause**, **Fix**, **Prevention**.

### Phase 3: Update the Index

6. **Update `.AiControl/lessons-learned/lessons.md`**:
   - Add the new entry to the table (lesson #, title, area, root cause summary)
   - Keep the table sorted by lesson number (descending — newest first)
   - Update the "Related Nodes" section if this lesson ties to a node

### Phase 4: Update Node Documents

7. **Identify which node(s) this lesson belongs to** — check `.AiControl/.current` or `.AiControl/root/` for the active node.

8. **Update the node's `state.md`**:
   - Add a "Lessons Learned" section at the bottom
   - Link to the new lesson page
   - If multiple lessons, add a mini-table

9. **If the lesson has practical implications** for how to use a tool or run a process, also update:
   - The node's `env.md` (if startup procedures or prerequisites changed)
   - The node's `skill.md` (if the workflow or key learnings section should reference it)

### Phase 5: Update Root Reference

10. **Update `.AiControl/root/state.md`**:
    - Find the "Lessons Learned" section
    - Add the new lesson to the mini-table (or create the section if it doesn't exist)

## Output Structure Reference

```
.AiControl/
├── lessons-learned/
│   ├── lessons.md                          ← Main index (table of ALL lessons)
│   ├── 001-vite-server-survival.md         ← Each lesson is a separate file
│   ├── 002-tauri-command-naming.md
│   └── NNN-slug.md
├── <node-name>/
│   ├── state.md                            ← Linked from Lessons section
│   ├── env.md                              ← Updated if tooling/procedures changed
│   └── skill.md                            ← Updated if workflow changed
└── root/
    └── state.md                            ← Main reference table
```

## Lesson Detail Page Template

Use this exact structure for every lesson:

```markdown
# Lesson NNN: <Short Title>

## Problem
<1-2 sentences describing the user-facing symptom>

## Symptoms
- <Error message or behavior 1>
- <Error message or behavior 2>

## Root Cause
<Paragraph explaining the actual cause. Include: what we thought vs what it was.>

## Fix
<What changed. Include before/after code or commands in fenced blocks.>

## Prevention
<Actionable steps to avoid this in the future. Be specific — include commands to run, tests to add, or conventions to follow.>

## Related
- <Link to other lessons: [NNN](./NNN-slug.md)>
- <Link to node documents: [Node](../node-name/env.md)>
- <External links if applicable>
```

## Table Entry Convention

In `.AiControl/lessons-learned/lessons.md`, each table entry should be:

```
| [NNN](./NNN-slug.md) | Short title (<60 chars) | Area (one of: E2E Testing, IPC/Rust, Frontend, Build, Tooling, Architecture, Process) | Root cause summary (<80 chars) |
```

## Checklist

Before finishing, verify:

- [ ] Each lesson has: Problem, Symptoms, Root Cause, Fix, Prevention
- [ ] Index table has the new entry
- [ ] Index table sorted by number descending
- [ ] Active node's `state.md` links to the lesson
- [ ] `root/state.md` has the lesson in its mini-table
- [ ] `env.md` or `skill.md` updated if procedures changed
- [ ] Lesson file uses `.md` extension and is in `lessons-learned/` directory
- [ ] File naming follows `NNN-slug.md` convention (zero-padded to 3 digits)
