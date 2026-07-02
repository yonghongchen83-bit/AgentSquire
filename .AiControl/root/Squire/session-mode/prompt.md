# Prompt

Use this node to implement per-session context mode persistence and lifecycle rules.

Deliverables:
- DB migration adding `sessions.context_mode` (default legacy).
- `Session` / `NewSession` / IPC type extensions to carry `context_mode`.
- Guard enforcing immutability once at least one user message exists (Q3): mode may be set at session creation only; a mode change request after that must be rejected, with the user directed to create a new session instead.
- `send_message` routes to the correct adapter based on the session's `context_mode`.

Reference: `../planning/decisions.md` Q3 for the resolved rule and rationale.
