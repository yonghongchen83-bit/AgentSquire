import { create } from 'zustand'
import type { Message } from '@/types/ipc'
import { getConversation, abortSubagent } from '@/lib/ipc'
import {
  onSubagentCreated,
  onSubagentChunk,
  onSubagentDone,
  onSubagentError,
  type SubagentCreatedPayload,
} from '@/lib/ipc'

export interface SubagentTab {
  sessionId: string
  parentCallId: string
  task: string
  status: 'running' | 'completed' | 'error'
  messages: Message[]
  result?: string
  streamingText: string
}

interface SubagentStore {
  tabs: SubagentTab[]
  activeTabId: string | null
  setActiveTab: (id: string | null) => void
  closeTab: (id: string) => void
  abortTab: (id: string) => void
  addOrUpdateTab: (payload: SubagentCreatedPayload) => void
  updateTabStream: (sessionId: string, text: string) => void
  completeTab: (sessionId: string, result: string, isError: boolean) => void
  failTab: (sessionId: string, error: string) => void
  cleanup: () => () => void
}

export const useSubagentStore = create<SubagentStore>((set, get) => {
  const cleanupFns: (() => void)[] = []

  async function setupListeners() {
    cleanupFns.push(
      await onSubagentCreated((payload) => {
        get().addOrUpdateTab(payload)
      }),
    )

    cleanupFns.push(
      await onSubagentChunk((payload) => {
        get().updateTabStream(payload.session_id, payload.text)
      }),
    )

    cleanupFns.push(
      await onSubagentDone((payload) => {
        get().completeTab(payload.session_id, payload.result, payload.is_error)
      }),
    )

    cleanupFns.push(
      await onSubagentError((payload) => {
        get().failTab(payload.session_id, payload.error)
      }),
    )
  }

  // Setup listeners immediately
  setupListeners()

  return {
    tabs: [],
    activeTabId: null,

    setActiveTab: (id) => set({ activeTabId: id }),

    closeTab: (id) => {
      set((s) => {
        const newTabs = s.tabs.filter((t) => t.sessionId !== id)
        const newActiveId = s.activeTabId === id ? (newTabs.length > 0 ? newTabs[newTabs.length - 1].sessionId : null) : s.activeTabId
        return { tabs: newTabs, activeTabId: newActiveId }
      })
    },

    abortTab: (id) => {
      // Call the IPC to abort the subagent
      abortSubagent(id).catch(() => {
        // Ignore errors — subagent may have already completed
      })
      // Remove the tab from view immediately
      set((s) => {
        const newTabs = s.tabs.filter((t) => t.sessionId !== id)
        const newActiveId = s.activeTabId === id ? (newTabs.length > 0 ? newTabs[newTabs.length - 1].sessionId : null) : s.activeTabId
        return { tabs: newTabs, activeTabId: newActiveId }
      })
    },

    addOrUpdateTab: (payload) => {
      set((s) => {
        const existing = s.tabs.find((t) => t.sessionId === payload.session_id)
        if (existing) {
          return {
            tabs: s.tabs.map((t) =>
              t.sessionId === payload.session_id
                ? { ...t, status: 'running' as const }
                : t,
            ),
          }
        }
        return {
          tabs: [
            ...s.tabs,
            {
              sessionId: payload.session_id,
              parentCallId: payload.parent_call_id,
              task: payload.task,
              status: 'running' as const,
              messages: [],
              streamingText: '',
            },
          ],
        }
      })
    },

    updateTabStream: (sessionId, text) => {
      set((s) => ({
        tabs: s.tabs.map((t) =>
          t.sessionId === sessionId
            ? { ...t, streamingText: t.streamingText + text }
            : t,
        ),
      }))
    },

    completeTab: async (sessionId, result, isError) => {
      // Load messages from the subagent session to show full conversation
      try {
        const session = await getConversation(sessionId)
        set((s) => ({
          tabs: s.tabs.map((t) =>
            t.sessionId === sessionId
              ? {
                  ...t,
                  status: isError ? 'error' as const : 'completed' as const,
                  messages: session.messages,
                  result,
                  streamingText: '',
                }
              : t,
          ),
        }))
      } catch {
        set((s) => ({
          tabs: s.tabs.map((t) =>
            t.sessionId === sessionId
              ? {
                  ...t,
                  status: isError ? 'error' as const : 'completed' as const,
                  result,
                  streamingText: '',
                }
              : t,
          ),
        }))
      }
    },

    failTab: (sessionId, error) => {
      set((s) => ({
        tabs: s.tabs.map((t) =>
          t.sessionId === sessionId
            ? { ...t, status: 'error' as const, result: error, streamingText: '' }
            : t,
        ),
      }))
    },

    cleanup: () => {
      return () => {
        cleanupFns.forEach((fn) => fn())
      }
    },
  }
})
