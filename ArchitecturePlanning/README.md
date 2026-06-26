# Architecture Planning

This folder contains architecture documentation and decision records for the project.

## Structure

```
ArchitecturePlanning/
├── README.md               # This file
├── component-analysis.md   # Critical components & build-vs-borrow analysis
├── dependency-report.md    # Full dependency inventory (Rust, npm, binaries, OS)
├── implementation-plan.md  # Build order: 7 phases, step by step
└── adr/                    # Architecture Decision Records
    ├── 0001-use-tauri-as-desktop-framework.md
    ├── 0002-use-react-as-frontend-framework.md
    ├── 0003-llm-provider-abstraction-trait.md
    ├── 0004-conversation-store-abstraction-trait.md
    ├── 0005-chat-ui-with-block-based-rendering.md
    ├── 0006-ripgrep-only-code-search.md
    └── 0007-use-momoi-explorer-for-file-tree.md
```

## ADR Process

Each ADR follows this format:
- **Title & Number**: Unique identifier
- **Status**: Proposed / Accepted / Deprecated / Superseded
- **Context**: Why this decision is needed
- **Decision**: What we decided
- **Consequences****: What this means for the project
- **Alternatives Considered**: Other options evaluated

## Active ADRs

| # | Title | Status |
|---|-------|--------|
| 1 | Use Tauri as Desktop Framework | Accepted |
| 2 | Use React as Frontend Framework | Accepted |
| 3 | LLM Provider Abstraction via Rust Trait | Accepted |
| 4 | Conversation Store Abstraction via Trait | Accepted |
| 5 | Chat UI with Block-Based Rendering | Accepted |
| 6 | ripgrep-Only Code Search — No Built-in Indexing | Accepted |
| 7 | Use momoi-explorer for File Tree | Accepted |

## State Principle

**Rust owns all authoritative state.** Frontend is a cache/view. All mutations go through Tauri IPC. This is a cross-cutting decision applied to all components.
