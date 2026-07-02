# Env

- Parent node: root/Squire
- Node path: root/Squire/session-mode
- Objective: Add per-session context_mode persistence with an immutable-after-first-turn lifecycle guard (Q3), and route send_message by session context_mode.
- Scope now: DB migration (sessions.context_mode, default legacy), Session/NewSession/IPC type extension, immutability enforcement, routing logic.
- Non-goal now: SquireContextAdapter internals, storage internals, UI mode selector (see sibling nodes).
- Depends on: adapter-core (needs the adapter trait/seam to route through).
- Blocks: squire-adapter (needs a session mode to select it).
