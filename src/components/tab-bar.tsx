import { X, Pin } from 'lucide-react'
import { useEditorStore } from '@/stores/editor-store'
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from '@/components/ui/context-menu'

export function TabBar() {
  const tabs = useEditorStore((s) => s.tabs)
  const activeTabId = useEditorStore((s) => s.activeTabId)
  const setActiveTab = useEditorStore((s) => s.setActiveTab)
  const closeTab = useEditorStore((s) => s.closeTab)
  const closeOtherTabs = useEditorStore((s) => s.closeOtherTabs)
  const closeAllTabs = useEditorStore((s) => s.closeAllTabs)
  const togglePinTab = useEditorStore((s) => s.togglePinTab)

  if (tabs.length === 0) return null

  return (
    <div className="flex h-9 items-stretch bg-[#E8EDF2] border-b border-border overflow-x-auto">
      {tabs.map((tab) => (
        <ContextMenu key={tab.id}>
          <ContextMenuTrigger>
            <div
              onClick={() => setActiveTab(tab.id)}
              className={`group flex items-center gap-1.5 px-3 text-sm border-r border-border cursor-pointer whitespace-nowrap shrink-0 transition-colors ${
                tab.id === activeTabId
                  ? 'bg-background text-foreground'
                  : 'bg-[#E8EDF2] text-[#6B7B8D] hover:text-foreground'
              }`}
            >
              {tab.isDirty && <span className="w-2 h-2 rounded-full bg-[#4A90D9]" />}
              {tab.isPinned && <Pin className="h-3 w-3 text-[#4A90D9]" />}
              {tab.filename}
              {!tab.isPinned && (
                <button
                  onClick={(e) => { e.stopPropagation(); closeTab(tab.id) }}
                  className="ml-1 flex items-center justify-center w-4 h-4 rounded opacity-0 group-hover:opacity-100 hover:bg-[#D0DCE8] transition-opacity"
                >
                  <X className="h-3 w-3" />
                </button>
              )}
            </div>
          </ContextMenuTrigger>
          <ContextMenuContent className="w-44">
            <ContextMenuItem onClick={() => closeTab(tab.id)} disabled={tab.isPinned}>
              Close
            </ContextMenuItem>
            <ContextMenuItem onClick={() => closeOtherTabs(tab.id)}>
              Close Others
            </ContextMenuItem>
            <ContextMenuItem onClick={() => closeAllTabs()}>
              Close All
            </ContextMenuItem>
            <ContextMenuSeparator />
            <ContextMenuItem onClick={() => togglePinTab(tab.id)}>
              {tab.isPinned ? 'Unpin Tab' : 'Pin Tab'}
            </ContextMenuItem>
            <ContextMenuSeparator />
            <ContextMenuItem onClick={() => navigator.clipboard.writeText(tab.path)}>
              Copy Path
            </ContextMenuItem>
          </ContextMenuContent>
        </ContextMenu>
      ))}
    </div>
  )
}
