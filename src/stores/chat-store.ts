import { create } from 'zustand'
import type { SessionSummary, Message, Block, ToolApprovalRequest, ProviderInfo } from '@/types/ipc'
import {
  listConversations,
  getConversation,
  createConversation,
  deleteConversation,
  sendMessage as sendMessageIpc,
  listProviders,
  onStreamChunk,
  onStreamToolCall,
  onStreamToolResult,
  onStreamToolPending,
  onStreamDone,
  onStreamError,
  approveToolCall as approveIpc,
  rejectToolCall as rejectIpc,
} from '@/lib/ipc'

function parseBlocks(content: string): Block[] {
  const blocks: Block[] = []
  let remaining = content
  const codeBlockRegex = /```(\w*)\n?([\s\S]*?)```/
  let match: RegExpExecArray | null

  while ((match = codeBlockRegex.exec(remaining)) !== null) {
    if (match.index > 0) {
      const before = remaining.slice(0, match.index).trim()
      if (before) blocks.push({ type: 'text', content: before })
    }
    blocks.push({
      type: 'code',
      language: match[1] || 'plaintext',
      content: match[2].trim(),
    })
    remaining = remaining.slice(match.index + match[0].length)
  }

  const after = remaining.trim()
  if (after) blocks.push({ type: 'text', content: after })

  if (blocks.length === 0 && content) {
    blocks.push({ type: 'text', content })
  }

  return blocks
}

interface ChatState {
  conversations: SessionSummary[]
  activeConversationId: string | null
  messages: Message[]
  isStreaming: boolean
  streamingMessageId: string | null
  streamingBlocks: Block[]
  streamingText: string
  error: string | null
  providers: ProviderInfo[]
  selectedProvider: string
  selectedModel: string
  pendingApprovals: ToolApprovalRequest[]

  loadConversations: () => Promise<void>
  loadProviders: () => Promise<void>
  selectConversation: (id: string) => Promise<void>
  createNewConversation: () => Promise<string | null>
  deleteConversation: (id: string) => Promise<void>
  setSelectedProvider: (name: string) => void
  setSelectedModel: (model: string) => void
  sendMessage: (content: string) => Promise<void>
  cancelStreaming: () => void
  clearError: () => void
  approveToolCall: (callId: string) => Promise<void>
  rejectToolCall: (callId: string) => Promise<void>
}

export const useChatStore = create<ChatState>((set, get) => {
  let cleanupFns: (() => void)[] = []

  async function setupStreamListeners(sessionId: string) {
    cleanupFns.forEach((fn) => fn())
    cleanupFns = []

    cleanupFns.push(
      await onStreamChunk((text) => {
        set((s) => ({
          streamingText: s.streamingText + text,
          streamingBlocks: parseBlocks(s.streamingText + text),
        }))
      }),
    )

    cleanupFns.push(
      await onStreamToolCall((tc) => {
        set((s) => ({
          streamingBlocks: [
            ...s.streamingBlocks,
            {
              type: 'tool_call',
              toolName: tc.name,
              args: JSON.stringify(tc.arguments, null, 2),
              callId: tc.id,
            },
          ],
        }))
      }),
    )

    cleanupFns.push(
      await onStreamToolResult((result) => {
        set((s) => {
          const blocks = [...s.streamingBlocks]
          const idx = blocks.findIndex(
            (b) => b.type === 'tool_call' && b.callId === result.call_id,
          )
          if (idx !== -1) {
            const block = blocks[idx]
            if (block.type === 'tool_call') {
              blocks[idx] = {
                ...block,
                result: result.output,
                isPending: false,
                isError: result.is_error,
              }
            }
          }
          return { streamingBlocks: blocks }
        })
      }),
    )

    cleanupFns.push(
      await onStreamToolPending((approval) => {
        set((s) => {
          const blocks = [...s.streamingBlocks]
          const idx = blocks.findIndex(
            (b) => b.type === 'tool_call' && b.callId === approval.call_id,
          )
          if (idx !== -1) {
            const block = blocks[idx]
            if (block.type === 'tool_call') {
              blocks[idx] = { ...block, isPending: true }
            }
          }
          return {
            streamingBlocks: blocks,
            pendingApprovals: [...s.pendingApprovals, approval],
          }
        })
      }),
    )

    cleanupFns.push(
      await onStreamDone(() => {
        const { streamingText } = get()
        const newMsg: Message = {
          id: crypto.randomUUID(),
          sessionId,
          role: 'assistant',
          content: streamingText,
          createdAt: new Date().toISOString(),
        }
        set({
          isStreaming: false,
          streamingMessageId: null,
          streamingText: '',
          streamingBlocks: [],
          pendingApprovals: [],
        })
        set((s) => ({
          messages: [...s.messages, newMsg],
        }))
        get().loadConversations()
      }),
    )

    cleanupFns.push(
      await onStreamError((err) => {
        set({
          isStreaming: false,
          streamingMessageId: null,
          streamingText: '',
          streamingBlocks: [],
          pendingApprovals: [],
          error: err,
        })
      }),
    )
  }

  return {
    conversations: [],
    activeConversationId: null,
    messages: [],
    isStreaming: false,
    streamingMessageId: null,
    streamingBlocks: [],
    streamingText: '',
    error: null,
    providers: [],
    selectedProvider: '',
    selectedModel: '',
    pendingApprovals: [],

    loadConversations: async () => {
      try {
        const conversations = await listConversations()
        set({ conversations })
      } catch (e) {
        set({ error: String(e) })
      }
    },

    loadProviders: async () => {
      try {
        const providers = await listProviders()
        set((s) => {
          const firstProvider = providers[0]
          const firstModel = firstProvider?.models[0] || ''
          return {
            providers,
            selectedProvider: s.selectedProvider || firstProvider?.name || '',
            selectedModel: s.selectedModel || firstModel,
          }
        })
      } catch {
        // ignore — will show empty selector
      }
    },

    setSelectedProvider: (name: string) => set({ selectedProvider: name }),

    setSelectedModel: (model: string) => set({ selectedModel: model }),

    selectConversation: async (id: string) => {
      try {
        const session = await getConversation(id)
        set({
          activeConversationId: id,
          messages: session.messages,
          error: null,
          streamingText: '',
          streamingBlocks: [],
          isStreaming: false,
          streamingMessageId: null,
          pendingApprovals: [],
        })
      } catch (e) {
        set({ error: String(e) })
      }
    },

    createNewConversation: async () => {
      try {
        const session = await createConversation('New Chat')
        set({ activeConversationId: session.id, messages: [], pendingApprovals: [] })
        get().loadConversations()
        return session.id
      } catch (e) {
        set({ error: String(e) })
        return null
      }
    },

    deleteConversation: async (id: string) => {
      try {
        await deleteConversation(id)
        const { activeConversationId } = get()
        if (activeConversationId === id) {
          set({ activeConversationId: null, messages: [] })
        }
        get().loadConversations()
      } catch (e) {
        set({ error: String(e) })
      }
    },

    sendMessage: async (content: string) => {
      const { activeConversationId, selectedProvider, selectedModel } = get()
      let sessionId = activeConversationId

      if (!sessionId) {
        sessionId = await get().createNewConversation()
        if (!sessionId) return
      }

      const userMsg: Message = {
        id: crypto.randomUUID(),
        sessionId,
        role: 'user',
        content,
        createdAt: new Date().toISOString(),
      }

      const assistantId = crypto.randomUUID()

      set((s) => ({
        messages: [...s.messages, userMsg],
        isStreaming: true,
        streamingMessageId: assistantId,
        streamingText: '',
        streamingBlocks: [],
        error: null,
        pendingApprovals: [],
      }))

      await setupStreamListeners(sessionId)

      try {
        await sendMessageIpc(sessionId, content, selectedProvider || undefined, selectedModel || undefined)
      } catch (e) {
        set({
          isStreaming: false,
          streamingMessageId: null,
          error: String(e),
        })
      }
    },

    cancelStreaming: () => {
      cleanupFns.forEach((fn) => fn())
      cleanupFns = []
      set({
        isStreaming: false,
        streamingMessageId: null,
        streamingText: '',
        streamingBlocks: [],
        pendingApprovals: [],
      })
    },

    clearError: () => set({ error: null }),

    approveToolCall: async (callId: string) => {
      try {
        await approveIpc(callId)
        set((s) => ({
          pendingApprovals: s.pendingApprovals.filter((a) => a.call_id !== callId),
        }))
      } catch (e) {
        set({ error: String(e) })
      }
    },

    rejectToolCall: async (callId: string) => {
      try {
        await rejectIpc(callId)
        set((s) => ({
          pendingApprovals: s.pendingApprovals.filter((a) => a.call_id !== callId),
        }))
      } catch (e) {
        set({ error: String(e) })
      }
    },
  }
})

if (typeof window !== 'undefined' && (import.meta as any).env?.DEV) {
  ;(window as any).__chatStore = useChatStore
}
