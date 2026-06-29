# Development Workflow

## The fundamental rule: every phase must trace back to the original requirement

Software engineering follows a strict sequence. Every step proves or disproves the previous step against the original requirement. Skipping or reordering these steps produces random behavior, not engineering.

## Feature Implementation

1. **Gather requirement** - Understand what the user actually wants. State it back in writing before proceeding.
2. **Design** - Plan the approach before writing code. What components change? What's the data flow?
3. **Implement** - Write code that matches the design.
4. **Test against requirement** - Verify the implementation satisfies the original requirement, using the same measure the user will use.

Each phase gates the next. Do not start implementation without an approved design. Do not finish without testing against the stated requirement.

## Bug Fixing

A bug is a failed requirement. Fixing it follows the same four phases, but phase 1 and phase 4 have specific meanings:

1. **Confirm the requirement / reproduce the bug** - The bug report is the requirement ("this should work, it doesn't"). Before anything else:
   - Physically reproduce the failure (click the button, run the command, observe the error)
   - OR construct undeniable logical proof of the failure path (no assumptions)
   - This is not optional. If you cannot reproduce the bug, you cannot fix it.
2. **Find root cause** - Trace from the observed failure to the originating fault. This is diagnosis, not speculation.
3. **Design and implement fix** - Change only what addresses the root cause. Do not fix things unrelated to the bug.
4. **Verify fix by reproducing** - Repeat the exact same reproduction procedure from step 1. The bug must be gone. If you can't reproduce it the same way, you haven't fixed it.

## Prohibited behaviors

- Do not modify code before reproducing the bug. Analysis of code without reproduction is speculation.
- Do not fix "similar" or "nearby" issues found during code reading. Only fix the reported bug.
- Do not assume the user made a mistake until you have physically reproduced the exact behavior they described.
- Do not skip testing against the original requirement. Passing unrelated tests is not proof of a fix.
- Do not close a bug because you fixed something else. The original failure must be confirmed resolved.
