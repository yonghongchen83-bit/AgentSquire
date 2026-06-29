---
name: lessons-learner
description: "Use at the END of a development session to systematically capture lessons learned. Trigger when: fixing bugs that took multiple attempts, discovering non-obvious root causes, learning toolchain/ framework quirks, or when the user says 'document this'. Creates structured lesson pages, updates the index in .AiControl/lessons-learned/, and links from node documents. Do NOT use for trivial one-off typos or normal feature implementation without unexpected issues."
---

# Lessons Learner Skill

## Workflow

### Phase 1: Session Review
1. Review the conversation history for: bugs fixed, dead ends, config issues, framework quirks, process improvements
2. For each candidate, answer: symptom → initial assumption → root cause → fix → prevention → area
3. Filter to non-trivial lessons only

### Phase 2: Create Lesson Pages
4. Read `.AiControl/lessons-learned/lessons.md` to find next available number
5. For each new lesson, create `lessons-learned/NNN-slug.md` with sections: Problem, Symptoms, Root Cause, Fix, Prevention, Related

### Phase 3: Update Index
6. Add entry to `.AiControl/lessons-learned/lessons.md` table (sorted by number descending)

### Phase 4: Update Node Documents
7. Read `.AiControl/.current` to find the active node
8. Update the node's `state.md` with a "Lessons Learned" section linking to the new lesson
9. If procedures changed, also update the node's `env.md` and `skill.md`

### Phase 5: Update Root Reference
10. Add/update entry in `.AiControl/root/state.md` Lessons Learned table

## Lesson Page Template
```markdown
# Lesson NNN: Title

## Problem
(Symptom description)

## Symptoms
- Error or behavior

## Root Cause
(What was actually causing it)

## Fix
(Before/after code or commands)

## Prevention
(Actionable future steps)

## Related
- [Link to other lessons](./NNN-slug.md)
```

## Checklist
- [ ] Each lesson has Problem, Symptoms, Root Cause, Fix, Prevention
- [ ] Index table updated and sorted by number descending
- [ ] Active node's `state.md` links to the lesson
- [ ] `root/state.md` has the lesson in its mini-table
- [ ] `env.md` or `skill.md` updated if procedures changed
- [ ] File naming follows `NNN-slug.md` convention
