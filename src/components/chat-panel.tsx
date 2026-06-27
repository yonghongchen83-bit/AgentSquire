import { useEffect } from 'react'
import { useChatStore } from '@/stores/chat-store'
import { ChatMessage } from '@/components/chat-message'
import { ChatInput } from '@/components/chat-input'
import { ConversationSidebar } from '@/components/conversation-sidebar'
import { MessagesSquare } from 'lucide-react'

export function ChatPanel() {
  const conversations = useChatStore((s) => s.conversations)
  const activeConversationId = useChatStore((s) => s.activeConversationId)
  const messages = useChatStore((s) => s.messages)
  const isStreaming = useChatStore((s) => s.isStreaming)
  const streamingBlocks = useChatStore((s) => s.streamingBlocks)
  const error = useChatStore((s) => s.error)
  const loadConversations = useChatStore((s) => s.loadConversations)
  const selectConversation = useChatStore((s) => s.selectConversation)
  const createNewConversation = useChatStore((s) => s.createNewConversation)
  const deleteConversation = useChatStore((s) => s.deleteConversation)
  const sendMessage = useChatStore((s) => s.sendMessage)
  const cancelStreaming = useChatStore((s) => s.cancelStreaming)
  const clearError = useChatStore((s) => s.clearError)

  useEffect(() => {
    loadConversations()
  }, [loadConversations])

  return (
    <div className="flex h-full bg-background">
      <div className="w-56 flex-shrink-0">
        <ConversationSidebar
          conversations={conversations}
          activeId={activeConversationId}
          onSelect={selectConversation}
          onCreate={createNewConversation}
          onDelete={deleteConversation}
        />
      </div>
      <div className="flex-1 flex flex-col min-w-0">
        {error && (
          <div className="flex items-center gap-2 px-4 py-2 bg-destructive/10 text-destructive text-sm border-b border-border">
            <span className="flex-1">{error}</span>
            <button
              onClick={clearError}
              className="text-xs hover:underline"
            >
              Dismiss
            </button>
          </div>
        )}
        <div className="flex-1 overflow-y-auto">
          {messages.length === 0 && !isStreaming ? (
            <div className="flex flex-col items-center justify-center h-full gap-3 text-[#6B7B8D] p-6">
              <MessagesSquare className="h-12 w-12" />
              <h2 className="text-lg font-semibold">Chat</h2>
              <p className="text-sm text-center max-w-md">
                Ask questions about your codebase, request changes, or get help with tasks.
              </p>
            </div>
          ) : (
            <div className="divide-y divide-border">
              {messages.map((msg) => (
                <ChatMessage
                  key={msg.id}
                  message={msg}
                />
              ))}
              {isStreaming && (
                <ChatMessage
                  message={{
                    id: 'streaming',
                    sessionId: activeConversationId || '',
                    role: 'assistant',
                    content: '',
                    createdAt: new Date().toISOString(),
                  }}
                  streamingBlocks={streamingBlocks}
                  isStreaming
                />
              )}
            </div>
          )}
        </div>
        <ChatInput
          onSend={sendMessage}
          onCancel={cancelStreaming}
          isStreaming={isStreaming}
        />
      </div>
    </div>
  )
}