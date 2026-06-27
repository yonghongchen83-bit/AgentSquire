# Env — Phase 6

See `.AiControl/root/env.md` for full toolchain setup.

Phase-specific:
- Sentry DSN goes in config, not hardcoded
- Auto-update requires a server endpoint (GitHub Releases default)
- Icon and app metadata needed for distribution
- Added radix packages: `@radix-ui/react-tabs`, `@radix-ui/react-select`, `@radix-ui/react-switch`, `@radix-ui/react-label`
- Config struct flattened to match frontend shape (camelCase serde rename)
