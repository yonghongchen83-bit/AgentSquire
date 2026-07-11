# State — RuntimeContext

## Status
🟡 Not started — Design phase.

## Goal
Define a `RuntimeContext` struct that bundles all dependencies an engine needs to run,
decoupling the engine from Tauri's `AppState`. This enables headless testing and future engine implementations.
