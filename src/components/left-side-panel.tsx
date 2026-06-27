import { useLayoutStore } from '@/stores/ui-store'
import { FileTree } from '@/components/file-tree'
import { Search, GitBranch } from 'lucide-react'

function ExplorerHeader() {
  return (
    <div className="flex items-center justify-between px-3 h-8 text-xs font-semibold text-[#6B7B8D] uppercase tracking-wider border-b border-border">
      <span>Explorer</span>
    </div>
  )
}

function SearchPlaceholder() {
  return (
    <div className="flex flex-col items-center justify-center h-full gap-2 text-[#6B7B8D] p-4">
      <Search className="h-8 w-8" />
      <p className="text-sm">Search panel coming soon</p>
    </div>
  )
}

function GitPlaceholder() {
  return (
    <div className="flex flex-col items-center justify-center h-full gap-2 text-[#6B7B8D] p-4">
      <GitBranch className="h-8 w-8" />
      <p className="text-sm">Git panel coming soon</p>
    </div>
  )
}

export function LeftSidePanel() {
  const activeView = useLayoutStore((s) => s.leftPanelActiveView)
  const leftPanelVisible = useLayoutStore((s) => s.leftPanelVisible)

  if (!leftPanelVisible) return null

  return (
    <div className="h-full bg-background border-r border-border flex flex-col">
      {activeView === 'explorer' && (
        <>
          <ExplorerHeader />
          <div className="flex-1 overflow-hidden">
            <FileTree />
          </div>
        </>
      )}
      {activeView === 'search' && <SearchPlaceholder />}
      {activeView === 'git' && <GitPlaceholder />}
    </div>
  )
}
