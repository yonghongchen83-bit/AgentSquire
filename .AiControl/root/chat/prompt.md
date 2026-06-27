# Prompt — Phase 3: Chat

Build the chat interface. Streaming responses, block rendering, conversation persistence.

## Steps (from implementation-plan.md)
3.1 Chat IPC wiring — send_message routes through LlmProvider, streams blocks ✅
3.2 shadcn-chatbot-kit scaffold — copy components, adapt to our IPC ✅ (custom, no shadcn dep)
3.3 Block-based stream render — text, thinking, tool_call, code blocks ✅
3.4 Thinking block — collapsible, animated ✅
3.5 Tool call block — expandable card ✅
3.6 Code block — Monaco read-only + action buttons ⏳ (pre/code rendering deferred)
3.7 Conversation sidebar — list/load/create/delete sessions ✅
3.8 Message persistence — auto-save via ConversationStore IPC ✅

## Files Created
- `src/components/chat-panel.tsx`, `chat-message.tsx`, `chat-blocks.tsx`, `chat-input.tsx`, `conversation-sidebar.tsx`
- `src/stores/chat-store.ts`
- `src/stores/chat-store.test.ts`, `src/components/__tests__/chat-*.test.tsx`

## Modified
- `src/types/ipc.ts`, `src/lib/ipc.ts`, `src/stores/ui-store.ts`, `src/components/sidebar.tsx`, `src/components/left-side-panel.tsx`
