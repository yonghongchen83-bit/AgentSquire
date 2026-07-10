# Prompt — Moving Squire Protocol Away from JSON

## Mission

Analyze and propose alternatives to JSON in the Squire protocol's LLM output format.
DeepSeek (and LLMs generally) are unreliable at producing valid JSON on demand.
The current SquireResponse JSON container is the #1 source of compliance failures.

## Current Architecture

The LLM must output a JSON object wrapping narrative content + protocol metadata:

```
SquireResponse {
  content: String        ← narrative text with §!/§^ sigils (user-visible)
  ask_user: String       ← question to user (mutually exclusive with content)
  new_tokens: Vec<...>   ← protocol metadata
  relationships: Vec<...>← protocol metadata
  preserve: Vec<String>  ← protocol metadata
}
```

This JSON is then repaired (safe-json-repair), parsed (serde_json), and validated (protocol.rs).
On failure: 3 retries, then compliance failure recorded.

## Key Insight

The user-facing content (with sigils) and the protocol metadata (new_tokens, relationships, preserve, ask_user) serve fundamentally different purposes and can use different channels. The content is the primary LLM output; the metadata is machine-readable scaffolding.

## Deliverable

For each task completed in this node, record:
1. Analysis of the approach (trade-offs, compatibility, effort)
2. Concrete code sketches or pseudo-code showing the change
3. Decision of whether to recommend, reject, or defer


