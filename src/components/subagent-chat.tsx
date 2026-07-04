import { useSubagentStore } from '@/stores/subagent-store'
import { ChatMessage } from '@/components/chat-message'
import { Bot, Loader2, Square } from 'lucide-react'

export function SubagentChat() {
  const tabs = useSubagentStore((s) => s.tabs)
  const activeTabId = useSubagentStore((s) => s.activeTabId)
  const setActiveTab = useSubagentStore((s) => s.setActiveTab)
  const abortTab = useSubagentStore((s) => s.abortTab)

  const activeTab = tabs.find((t) => t.sessionId === activeTabId)

  if (tabs.length === 0) {
    return (
      <div className="flex h-full items-center justify-center text-sm text-[#6B7B8D]">
        <div className="flex flex-col items-center gap-2">
          <Bot className="h-8 w-8" />
          <p>No subagents running</p>
          <p className="text-xs text-center max-w-xs">
            Subagents appear here when the AI spawns them to work on tasks independently.
          </p>
        </div>
      </div>
    )
  }

  if (!activeTab) {
    return (
      <div className="flex h-full items-center justify-center text-sm text-[#6B7B8D]">
        Select a subagent tab to view its conversation
      </div>
    )
  }

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="flex items-center gap-2 px-3 py-2 border-b border-border bg-muted/30">
        {activeTab.status === 'running' ? (
          <Loader2 className="h-4 w-4 text-[#4A90D9] animate-spin" />
        ) : (
          <Bot className="h-4 w-4 text-[#6B7B8D]" />
        )}
        <div className="flex-1 min-w-0">
          <div className="text-xs font-medium truncate">{activeTab.task}</div>
          <div className="text-[10px] text-[#6B7B8D]">
            {activeTab.status === 'running' ? 'Running...' : activeTab.status === 'error' ? 'Error' : 'Completed'}
          </div>
        </div>
        {activeTab.status === 'running' && (
          <button
            onClick={() => abortTab(activeTab.sessionId)}
            className="shrink-0 flex items-center gap-1 rounded px-2 py-1 text-xs font-medium text-red-600 hover:bg-red-50 transition-colors"
            title="Stop subagent"
          >
            <Square className="h-3 w-3 fill-current" />
            Stop
          </button>
        )}
      </div>

      {/* Messages */}
      <div className="flex-1 overflow-y-auto">
        {activeTab.messages.length === 0 && activeTab.streamingText ? (
          <div className="px-4 py-3">
            <div className="flex items-start gap-3">
              <div className="flex-shrink-0 w-8 h-8 rounded-full flex items-center justify-center bg-[#1A2332] text-white">
                <Bot className="h-4 w-4" />
              </div>
              <div className="flex-1 min-w-0">
                <div className="text-xs font-semibold text-[#6B7B8D] mb-1">Subagent</div>
                <p className="whitespace-pre-wrap text-sm">{activeTab.streamingText}</p>
                <span className="inline-block w-2 h-4 bg-[#4A90D9] animate-pulse ml-1" />
              </div>
            </div>
          </div>
        ) : (
          <div className="divide-y divide-border">
            {activeTab.messages.map((msg) => (
              <ChatMessage
                key={msg.id}
                message={msg}
                augmentBlocks={msg.blocks}
              />
            ))}
          </div>
        )}
        {activeTab.messages.length > 0 && activeTab.streamingText && (
          <div className="px-4 py-3 border-t border-border">
            <div className="flex items-start gap-3">
              <div className="flex-shrink-0 w-8 h-8 rounded-full flex items-center justify-center bg-[#1A2332] text-white">
                <Bot className="h-4 w-4" />
              </div>
              <div className="flex-1 min-w-0">
                <div className="text-xs font-semibold text-[#6B7B8D] mb-1">Subagent</div>
                <p className="whitespace-pre-wrap text-sm">{activeTab.streamingText}</p>
                <span className="inline-block w-2 h-4 bg-[#4A90D9] animate-pulse ml-1" />
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  )
}
