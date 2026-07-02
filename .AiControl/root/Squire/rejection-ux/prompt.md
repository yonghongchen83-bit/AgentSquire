# Prompt

Use this node to implement compliance-failure visibility and preserve-list lifecycle.

Deliverables:
- On final compliance failure after retry exhaustion, surface both the failure reason and the final failed AI response to the user (Q6).
- Persist structured failure metadata: rule violated, validator reason, retry count, timestamp.
- Preserve list applies strictly to the immediate next turn only; cleared on restart, not long-lived continuity state (Q7).

Reference: `../planning/decisions.md` Q6 and Q7 for the resolved rules and rationale.
