import { useSubagentStore } from '@/stores/subagent-store'
import { Bot, Loader2, X } from 'lucide-react'

export function SubagentTabBar() {
  const tabs = useSubagentStore((s) => s.tabs)
  const activeTabId = useSubagentStore((s) => s.activeTabId)
  const setActiveTab = useSubagentStore((s) => s.setActiveTab)
  const closeTab = useSubagentStore((s) => s.closeTab)

  if (tabs.length === 0) return null

  return (
    <div className="flex h-8 items-stretch bg-[#E8EDF2] border-b border-border overflow-x-auto shrink-0">
      {tabs.map((tab) => (
        <div
          key={tab.sessionId}
          className={`group flex items-center gap-1.5 px-2 text-xs border-r border-border cursor-pointer whitespace-nowrap shrink-0 transition-colors ${
            tab.sessionId === activeTabId
              ? 'bg-background text-foreground'
              : 'bg-[#E8EDF2] text-[#6B7B8D] hover:text-foreground'
          }`}
        >
          <div
            className="flex items-center gap-1.5 min-w-0 flex-1"
            onClick={() => setActiveTab(tab.sessionId)}
          >
            {tab.status === 'running' ? (
              <Loader2 className="h-3 w-3 text-[#4A90D9] animate-spin shrink-0" />
            ) : (
              <Bot className={`h-3 w-3 shrink-0 ${tab.status === 'error' ? 'text-red-500' : 'text-[#6B7B8D]'}`} />
            )}
            <span className="truncate max-w-[100px]">{tab.task}</span>
            <span className="text-[10px] text-[#6B7B8D] opacity-60 shrink-0">
              {tab.status === 'running' ? '...' : '\u2713'}
            </span>
          </div>
          <button
            onClick={(e) => {
              e.stopPropagation()
              closeTab(tab.sessionId)
            }}
            className="shrink-0 rounded-sm p-0.5 opacity-0 group-hover:opacity-100 hover:bg-[#D1D9E6] transition-opacity"
            title="Close tab (subagent continues in background)"
          >
            <X className="h-3 w-3" />
          </button>
        </div>
      ))}
    </div>
  )
}
