# ADR-0006: ripgrep-Only Code Search — No Built-in Indexing

**Status:** Accepted

**Date:** 2026-06-26

## Context

Code search is needed for the agent to understand code context (find definitions, usages, relevant files). Options range from simple text search (ripgrep) to AST parsing (tree-sitter) to full semantic indexing (embeddings + vector DB).

## Decision

Use **ripgrep only** for built-in code search. All advanced indexing (syntax parsing, semantic search, code graph analysis) is deferred to MCP servers — user-installed, out-of-process, orthogonal to the app core.

### Scope of built-in search

- Grep: regex search across project files via ripgrep
- Glob: file path pattern matching
- Filter: by file extension, path exclude patterns
- Context: configurable line context around matches

### What is NOT built in

- Tree-sitter AST parsing
- Embedding generation / vector search
- Code graph / dependency analysis
- Symbol indexing

These belong in **MCP servers** — the app provides the MCP host mechanism, users bring their own indexing.

## Rationale

- ripgrep is the fastest text search tool available — sufficient for the majority of code understanding needs
- Advanced indexing is highly user-specific (some want tree-sitter, some want embeddings, some want LSP)
- We intend to change how MCP works fundamentally — baking indexing into the app core would create coupling in the wrong direction
- Keeping the app core thin on indexing avoids bloating the binary with tree-sitter grammars (100+ languages), embedding models (~50MB+), and vector DBs

## Consequences

### Positive

- Minimal binary size (ripgrep ~5MB, no additional models or grammars)
- Search is instant for any project size
- Users choose their own indexing MCP servers — not our problem
- Clean separation: app = host, MCP servers = capability

### Negative

- No "go to definition" or symbol search without an MCP server
- ripgrep is line-based text search — cannot answer structural questions (e.g., "find all callers of this function")
- Users must discover and install MCP servers for advanced features
