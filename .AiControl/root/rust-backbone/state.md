# State — Phase 1: Rust Backbone

**Status:** Not started

## Design References
- `ArchitecturePlanning/component-analysis.md` — sections 10-20 (backend components)
- `ArchitecturePlanning/dependency-report.md` — Rust crates section
- `ArchitecturePlanning/implementation-plan.md` — Phase 1 steps
- `ArchitecturePlanning/adr/0003-llm-provider-abstraction-trait.md`
- `ArchitecturePlanning/adr/0004-conversation-store-abstraction-trait.md`
- `ArchitecturePlanning/adr/0006-ripgrep-only-code-search.md`

## Depends on
Phase 0 (scaffold). Runs parallel to Phase 0.5.

## Deliverables
- All Rust infrastructure: config, SQLite, LLM trait, provider registry, IPC commands, file ops, grep, git, file watcher
