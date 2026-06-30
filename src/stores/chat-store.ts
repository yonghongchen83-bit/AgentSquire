import { create } from 'zustand'
import type { SessionSummary, Message, Block, ToolApprovalRequest, ProviderInfo } from '@/types/ipc'
import {
  listConversations,
  getConversation,
  createConversation,
  renameConversation,
  deleteConversation,
  sendMessage as sendMessageIpc,
  abortStream,
  listProviders,
  onStreamChunk,
  onStreamThinking,
  onStreamToolCall,
  onStreamToolResult,
  onStreamToolPending,
  onStreamDone,
  onStreamError,
  onStreamStatus,
  approveToolCall as approveIpc,
  rejectToolCall as rejectIpc,
} from '@/lib/ipc'

const CHAT_MODEL_PREF_KEY = 'chat:last-model-selection'
const CHAT_THINKING_PREF_KEY = 'chat:last-thinking-level'

function loadStoredSelection(): { provider: string; model: string } {
  if (typeof window === 'undefined') {
    return { provider: '', model: '' }
  }
  try {
    const raw = window.localStorage.getItem(CHAT_MODEL_PREF_KEY)
    if (!raw) return { provider: '', model: '' }
    const parsed = JSON.parse(raw) as { provider?: string; model?: string }
    return {
      provider: parsed.provider ?? '',
      model: parsed.model ?? '',
    }
  } catch {
    return { provider: '', model: '' }
  }
}

function saveStoredSelection(provider: string, model: string) {
  if (typeof window === 'undefined') return
  try {
    window.localStorage.setItem(CHAT_MODEL_PREF_KEY, JSON.stringify({ provider, model }))
  } catch {
    // Ignore storage errors (private mode/quota/etc.)
  }
}

function loadStoredThinkingLevel(): 'none' | 'low' | 'mid' | 'high' {
  if (typeof window === 'undefined') return 'mid'
  try {
    const raw = window.localStorage.getItem(CHAT_THINKING_PREF_KEY)
    if (raw === 'none' || raw === 'low' || raw === 'mid' || raw === 'high') {
      return raw
    }
  } catch {
    // ignore
  }
  return 'mid'
}

function saveStoredThinkingLevel(level: 'none' | 'low' | 'mid' | 'high') {
  if (typeof window === 'undefined') return
  try {
    window.localStorage.setItem(CHAT_THINKING_PREF_KEY, level)
  } catch {
    // ignore
  }
}

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

function composeStreamingBlocks(thinkingText: string, responseText: string, existing: Block[]): Block[] {
  const nonTextBlocks = existing.filter((b) => b.type === 'tool_call')
  const blocks: Block[] = []

  if (thinkingText.trim()) {
    blocks.push({ type: 'thinking', content: thinkingText })
  }

  blocks.push(...parseBlocks(responseText))
  blocks.push(...nonTextBlocks)
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
  streamingThinkingText: string
  streamingStatus: string
  error: string | null
  providers: ProviderInfo[]
  selectedProvider: string
  selectedModel: string
  selectedThinkingLevel: 'none' | 'low' | 'mid' | 'high'
  pendingApprovals: ToolApprovalRequest[]

  loadConversations: () => Promise<void>
  loadProviders: () => Promise<void>
  selectConversation: (id: string) => Promise<void>
  createNewConversation: () => Promise<string | null>
  renameConversation: (id: string, title: string) => Promise<void>
  deleteConversation: (id: string) => Promise<void>
  setSelectedProvider: (name: string) => void
  setSelectedModel: (model: string) => void
  setSelectedThinkingLevel: (level: 'none' | 'low' | 'mid' | 'high') => void
  sendMessage: (content: string) => Promise<void>
  cancelStreaming: () => void
  clearError: () => void
  approveToolCall: (callId: string) => Promise<void>
  rejectToolCall: (callId: string) => Promise<void>
}

export const useChatStore = create<ChatState>((set, get) => {
  let cleanupFns: (() => void)[] = []
  const storedSelection = loadStoredSelection()
  const storedThinkingLevel = loadStoredThinkingLevel()

  async function setupStreamListeners(sessionId: string) {
    cleanupFns.forEach((fn) => fn())
    cleanupFns = []

    cleanupFns.push(
      await onStreamChunk((text) => {
        set((s) => ({
          streamingText: s.streamingText + text,
          streamingBlocks: composeStreamingBlocks(
            s.streamingThinkingText,
            s.streamingText + text,
            s.streamingBlocks,
          ),
        }))
      }),
    )

    cleanupFns.push(
      await onStreamThinking((text) => {
        set((s) => ({
          streamingThinkingText: s.streamingThinkingText + text,
          streamingBlocks: composeStreamingBlocks(
            s.streamingThinkingText + text,
            s.streamingText,
            s.streamingBlocks,
          ),
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
            pendingApprovals: s.pendingApprovals.some((a) => a.call_id === approval.call_id)
              ? s.pendingApprovals
              : [...s.pendingApprovals, approval],
          }
        })
      }),
    )

    cleanupFns.push(
      await onStreamStatus((status) => {
        set({ streamingStatus: status })
      }),
    )

    cleanupFns.push(
      await onStreamDone(() => {
          set({
          isStreaming: false,
          streamingMessageId: null,
          streamingText: '',
          streamingThinkingText: '',
          streamingStatus: '',
          streamingBlocks: [],
          pendingApprovals: [],
        })
          // Reload from DB so both user and assistant messages are correct
          // and no synthetic local message is needed
          const activeId = get().activeConversationId
          if (activeId) {
            getConversation(activeId)
              .then((session) => set({ messages: session.messages }))
              .catch(() => {})
          }
          get().loadConversations()
      }),
    )

    cleanupFns.push(
      await onStreamError((err) => {
          // Reload from DB on error too so user message isn't lost from view
          const activeId = get().activeConversationId
          set({
          isStreaming: false,
          streamingMessageId: null,
          streamingText: '',
          streamingThinkingText: '',
          streamingStatus: '',
          streamingBlocks: [],
          pendingApprovals: [],
          error: err,
        })
          if (activeId) {
            getConversation(activeId)
              .then((session) => set({ messages: session.messages }))
              .catch(() => {})
          }
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
    streamingThinkingText: '',
    streamingStatus: '',
    error: null,
    providers: [],
    selectedProvider: storedSelection.provider,
    selectedModel: storedSelection.model,
    selectedThinkingLevel: storedThinkingLevel,
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
          const firstProviderName = firstProvider?.name || ''
          const firstModel = firstProvider?.default_model || firstProvider?.models[0] || ''

          const stateProvider = s.selectedProvider
          const stateModel = s.selectedModel

          const stored = loadStoredSelection()

          const hasValidPair = (providerName: string, modelName: string) => {
            if (!providerName || !modelName) return false
            const provider = providers.find((p) => p.name === providerName)
            return !!provider && provider.models.includes(modelName)
          }

          let selectedProvider = firstProviderName
          let selectedModel = firstModel

          if (hasValidPair(stateProvider, stateModel)) {
            selectedProvider = stateProvider
            selectedModel = stateModel
          } else if (hasValidPair(stored.provider, stored.model)) {
            selectedProvider = stored.provider
            selectedModel = stored.model
          }

          saveStoredSelection(selectedProvider, selectedModel)

          return {
            providers,
            selectedProvider,
            selectedModel,
          }
        })
      } catch {
        // ignore — will show empty selector
      }
    },

    setSelectedProvider: (name: string) => set((s) => {
      saveStoredSelection(name, s.selectedModel)
      return { selectedProvider: name }
    }),

    setSelectedModel: (model: string) => set((s) => {
      saveStoredSelection(s.selectedProvider, model)
      return { selectedModel: model }
    }),

    setSelectedThinkingLevel: (level: 'none' | 'low' | 'mid' | 'high') => set(() => {
      saveStoredThinkingLevel(level)
      return { selectedThinkingLevel: level }
    }),

    selectConversation: async (id: string) => {
      try {
        const session = await getConversation(id)
        set({
          activeConversationId: id,
          messages: session.messages,
          error: null,
          streamingText: '',
          streamingThinkingText: '',
          streamingStatus: '',
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

    renameConversation: async (id: string, title: string) => {
      try {
        await renameConversation(id, title)
        set((s) => ({
          conversations: s.conversations.map((c) => (
            c.id === id ? { ...c, title } : c
          )),
        }))
      } catch (e) {
        set({ error: String(e) })
      }
    },

    sendMessage: async (content: string) => {
      const { activeConversationId, selectedProvider, selectedModel, selectedThinkingLevel } = get()
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
        streamingThinkingText: '',
        streamingStatus: 'Starting generation...',
        streamingBlocks: [],
        error: null,
        pendingApprovals: [],
      }))

      await setupStreamListeners(sessionId)

      try {
        await sendMessageIpc(
          sessionId,
          content,
          selectedProvider || undefined,
          selectedModel || undefined,
          selectedThinkingLevel,
        )
      } catch (e) {
        set({
          isStreaming: false,
          streamingMessageId: null,
          error: String(e),
        })
      }
    },

    cancelStreaming: () => {
      const { activeConversationId } = get()
      if (activeConversationId) {
        void abortStream(activeConversationId)
      }
      cleanupFns.forEach((fn) => fn())
      cleanupFns = []
      set({
        isStreaming: false,
        streamingMessageId: null,
        streamingText: '',
        streamingThinkingText: '',
        streamingStatus: '',
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
