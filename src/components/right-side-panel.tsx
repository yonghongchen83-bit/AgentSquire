import { ChatPanel } from '@/components/chat-panel'
import { SubagentTabBar } from '@/components/subagent-tab-bar'
import { SubagentChat } from '@/components/subagent-chat'
import { useSubagentStore } from '@/stores/subagent-store'
import { useLayoutStore } from '@/stores/ui-store'
import { X } from 'lucide-react'

export function RightSidePanel() {
  const toggleRightPanel = useLayoutStore((s) => s.toggleRightPanel)
  const hasActiveSubagent = useSubagentStore((s) => s.activeTabId !== null && s.tabs.some((t) => t.sessionId === s.activeTabId))

  return (
    <div className="h-full bg-background border-l border-border flex flex-col">
      <div className="flex items-center justify-between px-3 h-8 text-xs font-semibold text-[#6B7B8D] uppercase tracking-wider border-b border-border">
        <span>Chat</span>
        <button
          onClick={toggleRightPanel}
          className="flex items-center justify-center w-5 h-5 rounded hover:bg-[#D0DCE8] text-[#6B7B8D] hover:text-[#1A2332]"
        >
          <X className="h-3.5 w-3.5" />
        </button>
      </div>
      <SubagentTabBar />
      <div className="flex-1 overflow-hidden">
        {hasActiveSubagent ? <SubagentChat /> : <ChatPanel />}
      </div>
    </div>
  )
}
