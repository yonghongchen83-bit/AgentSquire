# ADR-0009: Log / Debug Mode Configuration

**Status:** Accepted

**Date:** 2026-06-27

## Context

The application needs structured, configurable logging for debugging and monitoring. Logs must support multiple verbosity levels, file rotation, per-module filtering, and different sinks for development vs production environments.

## Decision

We will use the `tracing` crate ecosystem:

- **`tracing-subscriber`** — registry + layer-based dispatch for structured events
- **`tracing-appender`** — non-blocking file writer with daily rotation
- **Config source**: `src-tauri/logging.yaml` (serde-deserialized, optional — sensible defaults)
- **Two sinks**: console (stdout, colorized in dev) + rolling file
- **Level control**: `--verbose` / `--quiet` CLI flags override config; per-module levels in YAML
- **File path**: `{app_data_dir}/logs/` via `dirs` crate (platform-specific)
- **Rotation**: Daily, 30-day retention, max 100MB per file
- **Format**: Human-readable in dev, JSON in production (controlled by config)

## Consequences

### Positive

- Structured JSON logs in production — easy to pipe to log aggregators (Datadog, Grafana Loki)
- Human-readable colorized output in development
- Per-module filter enables targeted debugging without verbosity spam
- Non-blocking appender prevents logging from blocking the main thread
- File rotation prevents disk exhaustion

### Negative

- YAML config file is an additional setup step for self-hosters
- `tracing-appender` adds a dependency and has occasional cross-platform quirks on Windows
