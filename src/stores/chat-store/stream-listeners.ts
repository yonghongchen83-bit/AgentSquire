import type { Block, Message } from '@/types/ipc'
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
  onStreamAskUserPending,
  setMessageBlocks,
  approveToolCall as approveIpc,
} from '@/lib/ipc'
import { composeStreamingBlocks } from '@/stores/chat-store/block-parser'

type SetState = (arg: any) => void

type GetState = () => {
  activeConversationId: string | null
  streamingBlocks: Block[]
  autoApproveScope: 'none' | 'session' | 'workspace'
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

  function isTreeToolCall(name: string, args: Record<string, unknown>): boolean {
    if (name !== 'todo_tree') return true
    const op = args.operation
    return op === 'list' || op === 'get'
  }

  cleanupFns.push(
    await onStreamToolCall((tc) => {
      if (!isTreeToolCall(tc.name, tc.arguments)) return
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
      if (get().autoApproveScope !== 'none') {
        void approveIpc(approval.call_id)
        return
      }
      set((s: { streamingBlocks: Block[]; pendingApprovals: Array<{ call_id: string; commandAnalysis?: import('@/types/ipc').CommandAnalysis }> }) => {
        const blocks = [...s.streamingBlocks]
        const idx = blocks.findIndex(
          (b) => b.type === 'tool_call' && b.callId === approval.call_id,
        )
        if (idx !== -1) {
          const block = blocks[idx]
          if (block.type === 'tool_call') {
            blocks[idx] = {
              ...block,
              isPending: true,
              commandAnalysis: approval.commandAnalysis,
            }
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
    await onStreamAskUserPending((question) => {
      set({ pendingAskUserQuestion: question })
    }),
  )

  cleanupFns.push(
    await onStreamStatus((status) => {
      set({ streamingStatus: status })
    }),
  )

  cleanupFns.push(
    await onStreamDone(() => {
      const toolCallBlocks = get().streamingBlocks.filter((b) => b.type === 'tool_call')

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

      const activeId = get().activeConversationId
      if (activeId) {
        getConversation(activeId)
          .then((session) => {
            if (toolCallBlocks.length > 0 && session.messages.length >= 2) {
              // Tool calls were made in this turn. The backend persists two
              // assistant messages: one with the pre-tool-call text (from
              // FinishReason::ToolCalls handler) and one with the final
              // response text (from finalize_turn). Merge them into a single
              // assistant message, nesting the tool_call blocks between the
              // two text parts so tool calls are not rendered as a separate
              // assistant response.
              const secondLast = session.messages[session.messages.length - 2]
              const lastMsg = session.messages[session.messages.length - 1]
              if (secondLast.role === 'assistant' && lastMsg.role === 'assistant') {
                // Merge the pre-tool-call text into blocks (before the
                // tool_call blocks) so the render order is:
                //   [pre-tool-call text] → [tool_call blocks] → [final response text]
                const preToolText: Block = secondLast.content
                  ? { type: 'text' as const, content: secondLast.content }
                  : null
                const mergedBlocks: Block[] = preToolText
                  ? [preToolText, ...toolCallBlocks]
                  : toolCallBlocks

                // Persist blocks onto the final merged message
                void setMessageBlocks(lastMsg.id, mergedBlocks)

                const messages = session.messages.slice(0, -2).concat([
                  {
                    ...lastMsg,
                    blocks: mergedBlocks,
                  } as Message,
                ])
                set({ messages })
                return
              }
            }
            set({ messages: session.messages })
          })
          .catch(() => {})
      }
      get().loadConversations()
    }),
  )

  cleanupFns.push(
    await onStreamError((err) => {
      const activeId = get().activeConversationId
      const currentBlocks = get().streamingBlocks
      const currentText = get().streamingText
      const currentThinking = get().streamingThinkingText
      const currentStatus = get().streamingStatus
      // Preserve streaming content instead of clearing it, so the user
      // doesn't lose already-rendered text/blocks when an error occurs.
      set({
        isStreaming: false,
        streamingMessageId: null,
        pendingApprovals: [],
        pendingAskUserQuestion: null,
        error: err,
      })
      // If we had any streaming content, keep it visible as a synthetic
      // assistant message so the user can see what was rendered before the error.
      if ((currentBlocks && currentBlocks.length > 0) || currentText || currentThinking) {
        const content = [
          currentThinking ? `[Thinking]\n${currentThinking}\n\n` : '',
          currentText,
        ].filter(Boolean).join('')
        set((state) => ({
          messages: [
            ...state.messages,
            {
              id: `stream-error-${Date.now()}`,
              sessionId: activeId || '',
              role: 'assistant' as const,
              content: content || '(response interrupted by error)',
              createdAt: new Date().toISOString(),
              blocks: currentBlocks && currentBlocks.length > 0 ? currentBlocks : undefined,
            },
          ],
        }))
      }
      // Also reload from DB in case any messages were persisted
      if (activeId) {
        getConversation(activeId)
          .then((session) => set({ messages: session.messages }))
          .catch(() => {})
      }
    }),
  )

  return cleanupFns
}
