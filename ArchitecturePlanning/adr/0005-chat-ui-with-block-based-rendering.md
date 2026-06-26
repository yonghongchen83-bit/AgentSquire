# ADR-0005: Chat UI with Block-Based Rendering

**Status:** Accepted

**Date:** 2026-06-26

## Context

The chat UI needs to render messages that include plain markdown, thinking/reasoning blocks, tool calls, code blocks, and error states. Streaming LLM responses arrive incrementally.

Two approaches exist:
1. **Incremental streaming render** — render partial markdown as each token arrives, handle edge cases (unclosed tags, partial code fences)
2. **Block-based render** — backend emits structured blocks; frontend renders each block only when complete

## Decision

Use **block-based rendering**. The backend emits discrete blocks (`text`, `thinking`, `tool_call`, `code`, `error`). The frontend renders each block upon completion.

The UI scaffold is sourced from **shadcn-chatbot-kit** (copy-paste model, zero dependencies), customized for our block types.

### Message stream format

```
text: "Hello, I'll help you with that."
thinking: "Analyzing the codebase structure..."
text: "The relevant file is `src/main.rs`."
tool_call: { "name": "read_file", "args": { "path": "src/main.rs" } }
code: { "language": "rust", "content": "fn main() {}" }
```

Each block is a self-contained JSON-ish segment. The frontend appends it to the message list only when the block terminator arrives.

### Streaming behavior

- Text blocks render progressively via react-markdown (stable markdown, no unclosed tags)
- Thinking blocks are collapsible with an animated indicator while streaming
- Tool call blocks show expandable args/result pairs
- Code blocks render with Monaco read-only view and action buttons (copy, apply)

## Consequences

### Positive

- No edge cases from partial markdown (unclosed fences, broken HTML)
- Frontend rendering is synchronous and simple — block is always self-contained
- Easy to add new block types without touching render pipeline
- Zero additional runtime dependencies (shadcn-chatbot-kit is copy-paste)
- Works naturally with our Rust LLM provider abstraction — the provider formats blocks

### Negative

- Blocks are a custom protocol — not compatible with plain OpenAI/Anthropic chat completions out of the box
- Users see content appear in chunks rather than character-by-character (acceptable tradeoff)
- Backend needs to buffer and split the stream into blocks
