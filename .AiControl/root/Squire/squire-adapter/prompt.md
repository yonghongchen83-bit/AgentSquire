# Prompt

Use this node to implement the Squire-mode adapter and its tool-exposure boundary.

Deliverables:
- `SquireContextAdapter` implementing `build_turn_input` / `handle_tool_loop_step` / `finalize_turn`.
- Strict Squire-only tool surface: Squire-mode sessions expose only Squire built-ins and gateway behavior; no direct external tool registrations reach the model (Q5).
- Protocol validation gates that classify compliant vs non-compliant model responses, feeding the retry/compliance-failure path defined in Q6 (surfaced concretely in ../rejection-ux).

Reference: `../planning/decisions.md` Q1, Q2, Q5, Q6 for the resolved contract and policy.
