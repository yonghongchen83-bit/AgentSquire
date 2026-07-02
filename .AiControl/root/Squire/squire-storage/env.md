# Env

- Parent node: root/Squire
- Node path: root/Squire/squire-storage
- Objective: Implement the Squire storage stack on LanceDB from day one (Q4) — structured and raw partitions with production-parity triplet store and vector search.
- Scope now: LanceDB schema for structured/raw partitions, triplet store + vector search path, storage interface abstraction for test doubles.
- Non-goal now: adapter control flow (see squire-adapter), UX/diagnostics (see rejection-ux).
- Depends on: squire-adapter (adapter needs a storage contract to read/write context against).
