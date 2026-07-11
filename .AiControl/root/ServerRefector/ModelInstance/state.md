# State — ModelInstance

## Status
🟡 Not started — Design phase.

## Goal
Define a `ModelInstance` struct that encapsulates everything needed to call an LLM:
provider, model, endpoint, API key, and options (thinking level, reasoning effort, etc.).

This replaces the current pattern of passing `provider_name`, `model`, `thinking_level` as separate IPC args.
