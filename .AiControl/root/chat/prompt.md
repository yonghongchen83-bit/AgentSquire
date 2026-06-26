# Prompt — Phase 3: Chat

Build the chat interface. Streaming responses, block rendering, conversation persistence.

## Steps (from implementation-plan.md)
3.1 Chat IPC wiring — send_message routes through LlmProvider, streams blocks
3.2 shadcn-chatbot-kit scaffold — copy components, adapt to our IPC
3.3 Block-based stream render — text, thinking, tool_call, code blocks
3.4 Thinking block — collapsible, animated
3.5 Tool call block — expandable card
3.6 Code block — Monaco read-only + action buttons
3.7 Conversation sidebar — list/load/create/delete sessions
3.8 Message persistence — auto-save via ConversationStore IPC
