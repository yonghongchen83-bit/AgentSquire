# ADR 0011: Test Code Refactoring Strategy — Separate Test Files

## Status

Accepted

## Context

Originally, all unit tests were written as inline `#[cfg(test)] mod tests { ... }` blocks inside the same source files as production code (e.g., `src/commands/config_update.rs` contained both `pub fn update_config()` and `mod tests { ... }`). As the codebase grew, this caused:

- **Poor readability** — production logic mixed with test code made files longer and harder to navigate.
- **No public API boundary enforcement** — tests could access private items freely, making refactoring without test breakage harder.
- **Difficult discovery** — finding all tests required searching inside every source file rather than looking in a dedicated `tests/` directory.

## Decision

We adopted a **hybrid approach** (Option A / Option B) to separate test code from production code:

### Option A — Top-level `tests/` directory (preferred)

Move tests into standalone files under `src-tauri/tests/`. Each test file imports the library via `use squirecli_lib::...` and tests only the **public API surface**.

**Used for** the majority of modules — 21 out of 27 files.

Example: `src/commands/config_update.rs` tests → `tests/config_update_test.rs`

**Criteria for Option A:**
- The module exposes sufficient public API for meaningful testing.
- The tests do not require access to `private` or `crate-private` items.

### Option B — `#[path]`-based sibling test files

When tests **must** access private/crate-private items (e.g., internal helper functions, `pub(crate)` structs), keep the test file as a sibling in the same directory linked via `#[cfg(test)] #[path = "..._test.rs"] mod test_module;` in the source file's `mod.rs` or equivalent.

**Used for** 6 modules where deep internal access is required:
- `src/agent/squire_test.rs` — tests `RecordingStore`, internal token operations
- `src/agent/context_adapter_test.rs` — tests internal context assembly
- `src/commands/providers_cmd_test.rs` — tests internal provider config logic
- `src/commands/streaming_cmd_test.rs` — tests internal streaming state
- `src/llm/openai_test.rs` — tests internal request building
- `src/state/config_test.rs` — tests internal config parsing

## Consequences

### Positive
- Production code files are shorter and focused.
- Clear boundary between public API tests (Option A) and internal logic tests (Option B).
- Easier to find all tests — Option A tests live in a single `tests/` directory.
- Encourages designing modules with testable public APIs.

### Negative
- Option B still requires `#[path]` attributes in source files (minimal coupling).
- Option A tests cannot access private items — some test coverage may need to be relaxed or moved to Option B.
- Some tests needed `pub` changes to previously private functions.

## Future Guidance

1. **Prefer Option A** for all new modules — design the public API to be testable.
2. **Use Option B only when** you genuinely need access to `pub(crate)` or private items and refactoring to expose a public API would be awkward.
3. **Never write inline `#[cfg(test)] mod tests { ... }` blocks** in production source files.
4. When adding a new module, create its test file immediately in `tests/` (Option A) to maintain the pattern.
