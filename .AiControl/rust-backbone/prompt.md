# Prompt — Phase 1: Rust Backbone

Build all Rust infrastructure. No frontend work — test via IPC.

## Steps (from implementation-plan.md)
1.1 Config module — `state/config.rs`, serde TOML
1.2 Logging — tracing subscriber
1.3 SQLite init — rusqlite, migrations
1.4 ConversationStore trait
1.5 SQLite ConversationStore impl
1.6 LlmProvider trait
1.7 OpenAI impl
1.8 Anthropic impl
1.9 Provider registry
1.10 Tauri IPC commands
1.11 File ops module
1.12 Grep command
1.13 Git ops module
1.14 Terminal/process module
1.15 File watcher adapter
