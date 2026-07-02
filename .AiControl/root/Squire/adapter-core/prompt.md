# Prompt

Use this node to finalize and implement the ContextManagerAdapter seam.

Deliverables:
- Concrete adapter trait definition (`build_turn_input`, `handle_tool_loop_step`, `finalize_turn`) with real Rust signatures.
- Exact insertion point identified and wired into send_message orchestration.
- `LegacyContextAdapter` implementing existing history-replay behavior with no semantic change.
- Parity tests proving legacy behavior is unchanged before/after the refactor.

Reference: `../planning/analysis.md` for the proposed trait shape and rationale, `../planning/state.md` for resolved Q1-Q7 decisions.
