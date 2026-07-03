import { create } from 'zustand'
import type { SessionSummary, Message, Block, ToolApprovalRequest, ProviderInfo, AskUserQuestion, ContextMode } from '@/types/ipc'
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
  answerAskUserQuestion as answerAskUserQuestionIpc,
  truncateMessagesFrom as truncateMessagesFromIpc,
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
  autoApproveScope: 'none' | 'session' | 'workspace'
  /** sa-5: at most one outstanding Squire ask_user question per session
   *  (the protocol loop is strictly sequential — see ask-user-loop/decisions.md). */
  pendingAskUserQuestion: AskUserQuestion | null

  loadConversations: () => Promise<void>
  loadProviders: () => Promise<void>
  selectConversation: (id: string) => Promise<void>
  /** contextMode defaults to 'legacy' when omitted, matching NewSession's backend default
   *  and the pre-existing implicit behavior before session-creation-ux added an explicit
   *  UI choice — see session-creation-ux/decisions.md. Mode is immutable by construction
   *  once a session exists (session-mode/decisions.md); this is the only place a mode is
   *  ever chosen. */
  createNewConversation: (contextMode?: ContextMode) => Promise<string | null>
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
  answerAskUserQuestion: (questionId: string, answer: string) => Promise<void>
  approveAllPending: () => Promise<void>
  setAutoApproveScope: (scope: 'none' | 'session' | 'workspace') => void
  truncateMessagesFrom: (messageId: string) => Promise<void>
  retryLastMessage: () => Promise<void>
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
    autoApproveScope: 'none',
    pendingAskUserQuestion: null,

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
        set((s) => ({
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
          pendingAskUserQuestion: null,
          autoApproveScope: s.autoApproveScope === 'session' ? 'none' : s.autoApproveScope,
        }))
      } catch (e) {
        set({ error: String(e) })
      }
    },

    createNewConversation: async (contextMode?: ContextMode) => {
      try {
        const session = await createConversation('New Chat', contextMode)
        set({ activeConversationId: session.id, messages: [], pendingApprovals: [], pendingAskUserQuestion: null })
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
        pendingAskUserQuestion: null,
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
      const { activeConversationId, streamingBlocks, streamingText, streamingThinkingText, messages } = get()
      // Preserve whatever content was already rendered before clearing streaming state,
      // so the user doesn't lose the AI's response (e.g. a question it was asking).
      const hasContent = streamingText || streamingThinkingText || (streamingBlocks && streamingBlocks.length > 0)
      if (hasContent) {
        const content = [
          streamingThinkingText ? `[Thinking]\n${streamingThinkingText}\n\n` : '',
          streamingText,
        ].filter(Boolean).join('') || '(response interrupted)'
        set({
          messages: [
            ...messages,
            {
              id: `stream-cancelled-${Date.now()}`,
              sessionId: activeConversationId || '',
              role: 'assistant' as const,
              content,
              createdAt: new Date().toISOString(),
              blocks: streamingBlocks && streamingBlocks.length > 0 ? streamingBlocks : undefined,
            },
          ],
        })
      }
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
        pendingAskUserQuestion: null,
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

    answerAskUserQuestion: async (questionId: string, answer: string) => {
      try {
        await answerAskUserQuestionIpc(questionId, answer)
        set((s) => (
          s.pendingAskUserQuestion?.question_id === questionId
            ? { pendingAskUserQuestion: null }
            : {}
        ))
      } catch (e) {
        set({ error: String(e) })
      }
    },

    approveAllPending: async () => {
      const { pendingApprovals } = get()
      await Promise.all(pendingApprovals.map((a) => approveIpc(a.call_id).catch(() => {})))
      set({ pendingApprovals: [] })
    },

    setAutoApproveScope: (scope: 'none' | 'session' | 'workspace') => {
      set({ autoApproveScope: scope })
    },

    truncateMessagesFrom: async (messageId: string) => {
      const { activeConversationId, messages } = get()
      if (!activeConversationId) return
      const idx = messages.findIndex((m) => m.id === messageId)
      if (idx === -1) return
      try {
        await truncateMessagesFromIpc(activeConversationId, messageId)
        set({ messages: messages.slice(0, idx) })
      } catch (e) {
        set({ error: String(e) })
      }
    },

    retryLastMessage: async () => {
      const { messages, activeConversationId } = get()
      if (!activeConversationId) return
      let lastUserIdx = -1
      for (let i = messages.length - 1; i >= 0; i--) {
        if (messages[i].role === 'user') {
          lastUserIdx = i
          break
        }
      }
      if (lastUserIdx === -1) return
      const content = messages[lastUserIdx].content
      const messageId = messages[lastUserIdx].id
      try {
        await truncateMessagesFromIpc(activeConversationId, messageId)
        set({ messages: messages.slice(0, lastUserIdx) })
        await get().sendMessage(content)
      } catch (e) {
        set({ error: String(e) })
      }
    },
  }
})

if (typeof window !== 'undefined' && (import.meta as any).env?.DEV) {
  ;(window as any).__chatStore = useChatStore
}
