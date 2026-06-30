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
  approveToolCall as approveIpc,
  rejectToolCall as rejectIpc,
} from '@/lib/ipc'
import {
  loadStoredSelection,
  saveStoredSelection,
  loadStoredThinkingLevel,
  saveStoredThinkingLevel,
} from '@/stores/chat-store/preferences'
import { resolveModelSelection } from '@/stores/chat-store/core'
import { setupStreamListeners as setupStreamListenersImpl } from '@/stores/chat-store/stream-listeners'

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
    void sessionId
    cleanupFns.forEach((fn) => fn())
    cleanupFns = await setupStreamListenersImpl({ set, get })
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
          const stored = loadStoredSelection()
          const { selectedProvider, selectedModel } = resolveModelSelection(
            providers,
            s.selectedProvider,
            s.selectedModel,
            stored.provider,
            stored.model,
          )

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
