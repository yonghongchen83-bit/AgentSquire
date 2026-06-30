import type { Block } from '@/types/ipc'
import {
  getConversation,
  onStreamChunk,
  onStreamDone,
  onStreamError,
  onStreamStatus,
  onStreamThinking,
  onStreamToolCall,
  onStreamToolPending,
  onStreamToolResult,
} from '@/lib/ipc'
import { composeStreamingBlocks } from '@/stores/chat-store/block-parser'

type SetState = (arg: any) => void

type GetState = () => {
  activeConversationId: string | null
  loadConversations: () => Promise<void> | void
}

export async function setupStreamListeners({
  set,
  get,
}: {
  set: SetState
  get: GetState
}): Promise<Array<() => void>> {
  const cleanupFns: Array<() => void> = []

  cleanupFns.push(
    await onStreamChunk((text) => {
      set((s: { streamingText: string; streamingThinkingText: string; streamingBlocks: Block[] }) => ({
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
      set((s: { streamingThinkingText: string; streamingText: string; streamingBlocks: Block[] }) => ({
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
      set((s: { streamingBlocks: Block[] }) => ({
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
      set((s: { streamingBlocks: Block[] }) => {
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
      set((s: { streamingBlocks: Block[]; pendingApprovals: Array<{ call_id: string }> }) => {
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

  return cleanupFns
}
