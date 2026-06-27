import { Trash2, Plus, MessageSquare } from 'lucide-react'
import type { SessionSummary } from '@/types/ipc'

interface ConversationSidebarProps {
  conversations: SessionSummary[]
  activeId: string | null
  onSelect: (id: string) => void
  onCreate: () => void
  onDelete: (id: string) => void
}

function formatDate(iso: string): string {
  try {
    const d = new Date(iso)
    const now = new Date()
    const diffMs = now.getTime() - d.getTime()
    const diffMins = Math.floor(diffMs / 60000)
    if (diffMins < 1) return 'Just now'
    if (diffMins < 60) return `${diffMins}m ago`
    const diffHours = Math.floor(diffMins / 60)
    if (diffHours < 24) return `${diffHours}h ago`
    const diffDays = Math.floor(diffHours / 24)
    if (diffDays < 7) return `${diffDays}d ago`
    return d.toLocaleDateString()
  } catch {
    return ''
  }
}

export function ConversationSidebar({
  conversations,
  activeId,
  onSelect,
  onCreate,
  onDelete,
}: ConversationSidebarProps) {
  return (
    <div className="flex flex-col h-full border-r border-border bg-[#F8F9FB]">
      <div className="flex items-center justify-between px-3 h-10 border-b border-border">
        <span className="text-xs font-semibold text-[#6B7B8D] uppercase tracking-wider">Conversations</span>
        <button
          onClick={onCreate}
          className="flex items-center justify-center w-6 h-6 rounded hover:bg-[#E8EDF2] text-[#6B7B8D] hover:text-[#1A2332] transition-colors"
          title="New chat"
        >
          <Plus className="h-4 w-4" />
        </button>
      </div>
      <div className="flex-1 overflow-y-auto">
        {conversations.length === 0 && (
          <div className="flex flex-col items-center justify-center h-full gap-2 text-[#6B7B8D] p-4">
            <MessageSquare className="h-8 w-8" />
            <p className="text-sm text-center">No conversations yet</p>
            <p className="text-xs text-center">Start a new chat to begin</p>
          </div>
        )}
        {conversations.map((conv) => (
          <div
            key={conv.id}
            className={`group flex items-center gap-2 px-3 py-2 cursor-pointer transition-colors ${
              activeId === conv.id
                ? 'bg-[#4A90D9]/10 border-l-2 border-[#4A90D9]'
                : 'hover:bg-[#E8EDF2] border-l-2 border-transparent'
            }`}
            onClick={() => onSelect(conv.id)}
          >
            <MessageSquare className="h-4 w-4 flex-shrink-0 text-[#6B7B8D]" />
            <div className="flex-1 min-w-0">
              <div className="text-sm truncate">{conv.title}</div>
              <div className="text-xs text-[#6B7B8D]">{formatDate(conv.lastMessageAt)}</div>
            </div>
            <button
              onClick={(e) => {
                e.stopPropagation()
                onDelete(conv.id)
              }}
              className="opacity-0 group-hover:opacity-100 flex items-center justify-center w-6 h-6 rounded hover:bg-destructive/10 text-[#6B7B8D] hover:text-destructive transition-all"
              title="Delete conversation"
            >
              <Trash2 className="h-3.5 w-3.5" />
            </button>
          </div>
        ))}
      </div>
    </div>
  )
}