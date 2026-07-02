# State

## Timeline
- 2026-07-02: Node created, split out of root/Squire/planning as implementation step 1 of the incremental delivery plan. Absorbs planning's open todo plan-2 ("Finalize adapter interface and insertion seam in send_message orchestration").

- 2026-07-02: ac-1 done — adapter interface + 3 insertion seams finalized against real code (see decisions.md).
- 2026-07-02: ac-2 done — implemented `src-tauri/src/agent/context_adapter.rs` (`ContextManagerAdapter` trait + `LegacyContextAdapter`), wired into `send_message_impl` at all 3 seams. `cargo build --lib` clean (no warnings), `cargo test --lib` 95/95 passing (5 new parity tests + no regressions in the other 90).

## Node Closed — 2026-07-02

Both todos done. Deliverables:
- `src-tauri/src/agent/context_adapter.rs` — trait + `LegacyContextAdapter`, unit tests for all 3 seams.
- `src-tauri/src/agent/mod.rs` — added `pub mod context_adapter;`.
- `src-tauri/src/commands/streaming_cmd.rs` — history assembly, per-tool-call message push, and final persistence now go through `adapter: Box<dyn ContextManagerAdapter>` (hardcoded to `LegacyContextAdapter`, no behavior change). Removed now-unused `ChatRole` import.

Adapter is not yet mode-selectable — that's `../session-mode`'s job (it will construct Legacy vs Squire based on `context_mode` instead of hardcoding Legacy).

## Next Actions
- Move to `../session-mode`: add per-session context_mode persistence + immutability guard + route adapter construction by mode.
