# Decisions — RuntimeContext

## D1. Engine trait lives in a new `engine-core` crate

A standalone crate with no Tauri dependency. This ensures the engine can never accidentally import Tauri types.

## D2. Event emission goes through a trait, not direct Tauri APIs

`EventEmitter` trait in `engine-core`. Real app wraps `AppHandle`, tests use `RecordingEventEmitter`. This is the key decoupling mechanism.

## D3. RuntimeConfig uses two-tier design

Typed fields for known configuration (`verbose_logging`, `squire_prefetch`). `test_config: HashMap<String, String>` for test-specific flags. `extra: HashMap<String, String>` for general extensibility.

This avoids bloating `RuntimeConfig` with every possible flag while keeping commonly-used fields discoverable.

## D4. AppHandle stays as optional legacy bridge

The `SubagentTool` currently needs `AppHandle` to access Tauri's process management. Making it optional with a clear `TODO(ServerRefactor): remove once SubagentTool is decoupled` ensures we don't block the refactoring.

## D5. One engine type per-phase, not a single mega-engine

Phase 1 (content generation) and Phase 2 (token/relationship processing) may become separate `Engine` implementations sharing the same trait. For now, `SquireEngine` handles both phases internally, matching current behavior.
