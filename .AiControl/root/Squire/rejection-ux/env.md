# Env

- Parent node: root/Squire
- Node path: root/Squire/rejection-ux
- Objective: Implement user-facing compliance-failure visibility (Q6) and preserve-list lifecycle handling (Q7).
- Scope now: surfacing failure reason + final failed AI response after retry exhaustion, structured failure-metadata persistence, preserve-list next-turn-only lifecycle.
- Non-goal now: protocol validation logic itself (see squire-adapter).
- Depends on: squire-adapter (needs the validation/retry path to hook into).
