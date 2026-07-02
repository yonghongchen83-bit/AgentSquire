# Env

- Parent node: root/Squire
- Node path: root/Squire/squire-adapter
- Objective: Implement SquireContextAdapter with the Squire open/loop/close lifecycle and strict gateway-only tool exposure (Q5).
- Scope now: SquireContextAdapter skeleton, strict tool-surface separation per session mode, protocol validation gates feeding into rejection/retry behavior (Q6).
- Non-goal now: storage internals (see squire-storage), rejection UX surfacing details (see rejection-ux).
- Depends on: adapter-core (adapter trait/seam), session-mode (needs context_mode to select this adapter).
- Blocks: squire-storage, rejection-ux.
