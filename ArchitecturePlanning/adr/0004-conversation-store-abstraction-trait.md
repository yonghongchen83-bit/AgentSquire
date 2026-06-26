# ADR-0004: Conversation Store via Abstract Trait

**Status:** Accepted

**Date:** 2026-06-26

## Context

Conversation history storage is an area where requirements will evolve — we may start with local SQLite, later move to cloud sync, file-based export, or encrypted stores. The storage implementation must be entirely swappable without changing any code that reads or writes conversations.

## Decision

Define a minimal `ConversationStore` trait in Rust. The rest of the app depends only on this trait. SQLite via `rusqlite` is the initial implementation.

```rust
/// Minimum surface for conversation persistence.
#[async_trait]
trait ConversationStore {
    /// Create a new conversation session.
    async fn create_session(&self, session: NewSession) -> Result<Session>;

    /// Append a message (user or assistant) to a session.
    async fn append_message(&self, msg: NewMessage) -> Result<Message>;

    /// Load full message history for a session.
    async fn get_session(&self, id: SessionId) -> Result<SessionWithMessages>;

    /// List all sessions (title, timestamp, preview).
    async fn list_sessions(&self) -> Result<Vec<SessionSummary>>;

    /// Delete a session.
    async fn delete_session(&self, id: SessionId) -> Result<()>;
}
```

The trait is intentionally bare-bones:
- No pagination, no filtering, no search — add only when needed
- No assumptions about storage format (SQL tables, JSON files, cloud API)
- `SessionId` is an opaque newtype (`String` or `Uuid`), not a database integer

## Consequences

### Positive

- Storage backend can be swapped by writing a new `impl ConversationStore` — no other code changes
- Testing: in-memory `Vec`-backed implementation for unit tests, no database setup
- SQLite implementation details (migrations, connection pool, schema) are sealed behind the trait
- Can evolve the trait with additive methods without breaking existing impls

### Negative

- Trait is minimal upfront — may need breaking changes if we discover missing operations
- SQLite-specific features (FTS search, migrations) are not exposed — must be handled inside the impl
- Need to handle cross-session operations (search across conversations) at a higher layer
