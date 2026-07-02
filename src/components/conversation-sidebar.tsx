import { Trash2, Plus, MessageSquare, Pencil, Check, X } from 'lucide-react'
import { useState } from 'react'
import type { SessionSummary, ContextMode } from '@/types/ipc'
import { Switch } from '@/components/ui/switch'
import {
  loadStoredSquireModeDefault,
  saveStoredSquireModeDefault,
} from '@/stores/chat-store/preferences'

interface ConversationSidebarProps {
  conversations: SessionSummary[]
  activeId: string | null
  onSelect: (id: string) => void
  /** contextMode reflects the "Squire mode" toggle at creation time — undefined/omitted
   *  means Legacy (the default), matching createNewConversation's own default. Mode is
   *  chosen once, here, and is immutable afterward (session-mode/decisions.md) — there is
   *  deliberately no equivalent "change mode" callback for an existing conversation. */
  onCreate: (contextMode?: ContextMode) => void
  onDelete: (id: string) => void
  onRename: (id: string, title: string) => void | Promise<void>
  standalone?: boolean
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
  onRename,
  standalone = false,
}: ConversationSidebarProps) {
  const [editingId, setEditingId] = useState<string | null>(null)
  const [titleDraft, setTitleDraft] = useState('')
  // Squire mode is an explicit opt-in at creation time; Legacy is the default so existing
  // muscle memory / expectations aren't disrupted (session-creation-ux/decisions.md). The
  // last-chosen value is persisted (session-ux-polish/decisions.md) so it survives a
  // remount instead of always resetting to Legacy — a brand-new/never-toggled install still
  // defaults to Legacy since loadStoredSquireModeDefault() returns false when unset.
  const [nextSessionSquireMode, setNextSessionSquireMode] = useState(() => loadStoredSquireModeDefault())

  const handleSquireModeToggle = (checked: boolean) => {
    setNextSessionSquireMode(checked)
    saveStoredSquireModeDefault(checked)
  }

  const startEditing = (id: string, title: string) => {
    setEditingId(id)
    setTitleDraft(title)
  }

  const cancelEditing = () => {
    setEditingId(null)
    setTitleDraft('')
  }

  const saveEditing = async (id: string, originalTitle: string) => {
    const trimmed = titleDraft.trim()
    if (!trimmed) {
      cancelEditing()
      return
    }
    if (trimmed !== originalTitle) {
      await onRename(id, trimmed)
    }
    cancelEditing()
  }

  return (
    <div className={`flex flex-col h-full bg-[#F8F9FB] ${standalone ? '' : 'border-r border-border'}`}>
      <div className="flex items-center justify-between px-3 h-10 border-b border-border">
        <span className="text-xs font-semibold text-[#6B7B8D] uppercase tracking-wider">Sessions</span>
        <div className="flex items-center gap-2">
          <label className="flex items-center gap-1.5 cursor-pointer select-none" title="New sessions use Squire's curated protocol context instead of full-history replay. This choice is fixed once a session is created.">
            <span className={`text-[10px] font-medium uppercase tracking-wide ${nextSessionSquireMode ? 'text-[#4A90D9]' : 'text-[#6B7B8D]'}`}>
              Squire
            </span>
            <Switch
              checked={nextSessionSquireMode}
              onCheckedChange={handleSquireModeToggle}
              className="h-4 w-7 [&>span]:h-3 [&>span]:w-3 [&>span]:data-[state=checked]:translate-x-3"
              aria-label="Create new sessions in Squire mode"
            />
          </label>
          <button
            onClick={() => onCreate(nextSessionSquireMode ? 'squire' : 'legacy')}
            className="flex items-center justify-center w-6 h-6 rounded hover:bg-[#E8EDF2] text-[#6B7B8D] hover:text-[#1A2332] transition-colors"
            title="New session"
          >
            <Plus className="h-4 w-4" />
          </button>
        </div>
      </div>
      <div className="flex-1 overflow-y-auto">
        {conversations.length === 0 && (
          <div className="flex flex-col items-center justify-center h-full gap-2 text-[#6B7B8D] p-4">
            <MessageSquare className="h-8 w-8" />
            <p className="text-sm text-center">No sessions yet</p>
            <p className="text-xs text-center">Start a new session to begin</p>
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
            onClick={() => {
              if (editingId !== conv.id) onSelect(conv.id)
            }}
          >
            <MessageSquare className="h-4 w-4 flex-shrink-0 text-[#6B7B8D]" />
            <div className="flex-1 min-w-0">
              {editingId === conv.id ? (
                <input
                  value={titleDraft}
                  onChange={(e) => setTitleDraft(e.target.value)}
                  className="w-full text-sm bg-white border border-[#C9D4E1] rounded px-1.5 py-0.5"
                  autoFocus
                  onClick={(e) => e.stopPropagation()}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter') {
                      e.preventDefault()
                      void saveEditing(conv.id, conv.title)
                    }
                    if (e.key === 'Escape') {
                      e.preventDefault()
                      cancelEditing()
                    }
                  }}
                />
              ) : (
                <div className="flex items-center gap-1.5 min-w-0">
                  <span className="text-sm truncate">{conv.title}</span>
                  {conv.contextMode === 'squire' && (
                    <span
                      className="flex-shrink-0 text-[9px] font-semibold uppercase tracking-wide text-[#4A90D9] bg-[#4A90D9]/10 rounded px-1 py-[1px]"
                      title="This session uses Squire's curated protocol context"
                    >
                      Squire
                    </span>
                  )}
                </div>
              )}
              <div className="text-xs text-[#6B7B8D]">{formatDate(conv.lastMessageAt)}</div>
            </div>
            {editingId === conv.id ? (
              <>
                <button
                  onClick={(e) => {
                    e.stopPropagation()
                    void saveEditing(conv.id, conv.title)
                  }}
                  className="flex items-center justify-center w-6 h-6 rounded hover:bg-emerald-100 text-emerald-700 transition-all"
                  title="Save title"
                >
                  <Check className="h-3.5 w-3.5" />
                </button>
                <button
                  onClick={(e) => {
                    e.stopPropagation()
                    cancelEditing()
                  }}
                  className="flex items-center justify-center w-6 h-6 rounded hover:bg-[#E8EDF2] text-[#6B7B8D] transition-all"
                  title="Cancel rename"
                >
                  <X className="h-3.5 w-3.5" />
                </button>
              </>
            ) : (
              <button
                onClick={(e) => {
                  e.stopPropagation()
                  startEditing(conv.id, conv.title)
                }}
                className="opacity-0 group-hover:opacity-100 flex items-center justify-center w-6 h-6 rounded hover:bg-[#E8EDF2] text-[#6B7B8D] hover:text-[#1A2332] transition-all"
                title="Rename conversation"
              >
                <Pencil className="h-3.5 w-3.5" />
              </button>
            )}
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