# State

## Child Nodes

`planning` (completed 2026-07-02) produced a locked architecture for the ContextManagerAdapter (see `planning/implementation-readiness.md`). Implementation is organized into sibling nodes, each scoped to one technical context, in dependency order:

1. `adapter-core` — adapter trait + insertion seam + LegacyContextAdapter extraction (✅ completed 2026-07-02)
2. `session-mode` — context_mode persistence + immutability guard + routing (✅ completed 2026-07-02)
3. `squire-adapter` — SquireContextAdapter + strict tool surface + validation gates (next up — unblocked)
4. `squire-storage` — LanceDB storage stack (blocked on 3)
5. `rejection-ux` — compliance-failure UX + diagnostics + preserve-list lifecycle (blocked on 3)
6. `protocol-doc-sync` — spec doc sync (non-blocking, can run any time before implementation freeze)

