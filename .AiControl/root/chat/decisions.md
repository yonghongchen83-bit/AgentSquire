# Decisions — Phase 3

Decisions made during chat implementation.

| # | Decision | Rationale |
|---|----------|-----------|
| 1 | **No shadcn-chatbot-kit dependency** | Built custom components matching existing shadcn/ui patterns; avoids adding another UI library |
| 2 | **Inline block parsing from raw text** | Parse code blocks from LLM output directly instead of a separate markdown parser; simpler, matches ADR-0005 block-based approach |
| 3 | **Chat as left-panel view** | Added 'chat' to SidebarView, renders ChatPanel in left-side-panel; consistent with explorer/search/git pattern |
| 4 | **Auto-create conversation on first message** | No separate "new chat" form; create session on first send if none active |
| 5 | **Pre/code rendering for code blocks** | Deferred Monaco read-only integration to post-MVP; pre/code with dark bg sufficient for Phase 3 |
| 6 | **Zustand for chat state** | Follows existing state management pattern (editor-store, ui-store); event-driven streaming fits Zustand well |
