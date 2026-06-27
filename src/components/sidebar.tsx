import { Files, Search, GitBranch, Settings } from 'lucide-react'
import { useLayoutStore, type SidebarView } from '@/stores/ui-store'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip'

const items: { id: SidebarView; icon: typeof Files; label: string }[] = [
  { id: 'explorer', icon: Files, label: 'Explorer' },
  { id: 'search', icon: Search, label: 'Search' },
  { id: 'git', icon: GitBranch, label: 'Git' },
]

export function Sidebar() {
  const activeView = useLayoutStore((s) => s.leftPanelActiveView)
  const setView = useLayoutStore((s) => s.setLeftPanelView)
  const leftPanelVisible = useLayoutStore((s) => s.leftPanelVisible)
  const toggleLeftPanel = useLayoutStore((s) => s.toggleLeftPanel)

  return (
    <TooltipProvider>
      <div className="flex w-12 flex-col items-center gap-1 bg-[#E8EDF2] py-2 border-r border-border">
        {items.map(({ id, icon: Icon, label }) => {
          const isActive = activeView === id && leftPanelVisible
          return (
            <Tooltip key={id}>
              <TooltipTrigger asChild>
                <button
                  onClick={() => isActive ? toggleLeftPanel() : setView(id)}
                  className={`flex items-center justify-center w-9 h-9 rounded-md transition-colors ${
                    isActive
                      ? 'bg-[#4A90D9] text-white'
                      : 'text-[#6B7B8D] hover:bg-[#D0DCE8] hover:text-[#1A2332]'
                  }`}
                >
                  <Icon className="h-5 w-5" />
                </button>
              </TooltipTrigger>
              <TooltipContent side="right" sideOffset={8}>
                {label}
              </TooltipContent>
            </Tooltip>
          )
        })}
        <div className="flex-1" />
        <Tooltip>
          <TooltipTrigger asChild>
            <button className="flex items-center justify-center w-9 h-9 rounded-md text-[#6B7B8D] hover:bg-[#D0DCE8] hover:text-[#1A2332] transition-colors">
              <Settings className="h-5 w-5" />
            </button>
          </TooltipTrigger>
          <TooltipContent side="right" sideOffset={8}>
            Settings
          </TooltipContent>
        </Tooltip>
      </div>
    </TooltipProvider>
  )
}
