//! Engine abstraction layer.
//!
//! Defines the `Engine` trait, `RuntimeContext`, and related types that
//! decouple orchestration from Tauri's `AppState`. The engine trait is
//! the single entry point for running a chat turn — any engine
//! implementation (Squire, Legacy, future) can be driven identically
//! by constructing a `RuntimeContext` and calling `Engine::run()`.
//!
//! ## Design
//!
//! - **No Tauri dependency** — `RuntimeContext` uses trait objects for
//!   all dependencies. `EventEmitter` replaces direct `AppHandle::emit()`.
//! - **Headless testable** — tests construct `RuntimeContext` with in-memory
//!   stores, mock providers, and a recording event emitter.
//! - **RuntimeConfig extensibility** — typed fields for known config,
//!   `test_config` HashMap for test-specific flags.

mod emitter;
pub mod runtime;
mod traits;

pub use emitter::TauriEventEmitter;
pub use squire::SquireEngine;

pub use runtime::{RuntimeConfig, RuntimeContext};
pub use traits::{Engine, EngineEvent, EventEmitter};

pub mod squire;

/// Re-export key types for convenience.
pub use provider_core::{ChatRequest, ModelInstance, StreamEvent};
