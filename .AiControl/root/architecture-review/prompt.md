# Prompt — Architecture Review

Review the full project architecture. Examine the existing codebase, document component relationships, data flow, integration points, and architectural decisions. Produce a comprehensive architecture review report covering all key aspects of the system.

## Scope

1. **System Context** — high-level project overview, technology stack, build pipeline
2. **Component Architecture** — frontend component tree, Rust module hierarchy, IPC boundary
3. **Data Flow** — state ownership (Rust vs frontend), IPC message flow, storage layer
4. **Integration Points** — external APIs (LLM providers), file system, git, shell
5. **Security & Operations** — auth, config, error handling, observability
6. **Architectural Decisions** — review existing ADRs, document new findings
7. **Risk Assessment** — technical debt, scalability concerns, design gaps
