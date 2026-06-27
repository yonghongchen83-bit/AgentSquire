import { useStatusBarStore, useLayoutStore } from '@/stores/ui-store'
import { Terminal, Bell } from 'lucide-react'

export function StatusBar() {
  const llmConnected = useStatusBarStore((s) => s.llmConnected)
  const llmProvider = useStatusBarStore((s) => s.llmProvider)
  const notificationCount = useStatusBarStore((s) => s.notificationCount)
  const cursorLine = useStatusBarStore((s) => s.cursorLine)
  const cursorCol = useStatusBarStore((s) => s.cursorCol)
  const uiFontZoom = useLayoutStore((s) => s.uiFontZoom)
  const setUiFontZoom = useLayoutStore((s) => s.setUiFontZoom)
  const bottomPanelVisible = useLayoutStore((s) => s.bottomPanelVisible)
  const toggleBottomPanel = useLayoutStore((s) => s.toggleBottomPanel)
  const projectPath = useLayoutStore((s) => s.projectPath)

  return (
    <div className="flex h-6 items-center justify-between bg-[#E8EDF2] px-3 text-xs text-[#6B7B8D] border-t border-border select-none">
      <div className="flex items-center gap-3">
        <span className={`flex items-center gap-1 ${llmConnected ? 'text-green-600' : ''}`}>
          <span className={`inline-block w-2 h-2 rounded-full ${llmConnected ? 'bg-green-500' : 'bg-gray-400'}`} />
          {llmConnected ? llmProvider || 'Connected' : 'Disconnected'}
        </span>
        {projectPath && (
          <span className="truncate max-w-[200px]" title={projectPath}>
            {projectPath}
          </span>
        )}
      </div>
      <div className="flex items-center gap-3">
        <button onClick={toggleBottomPanel} className="flex items-center gap-1 hover:text-[#1A2332] transition-colors">
          <Terminal className="h-3 w-3" />
          <span>{bottomPanelVisible ? 'Hide Terminal' : 'Show Terminal'}</span>
        </button>
        <button className="relative flex items-center hover:text-[#1A2332] transition-colors">
          <Bell className="h-3 w-3" />
          {notificationCount > 0 && (
            <span className="absolute -top-1 -right-1 w-3 h-3 bg-destructive text-destructive-foreground text-[9px] rounded-full flex items-center justify-center">
              {notificationCount}
            </span>
          )}
        </button>
        <span>Ln {cursorLine}, Col {cursorCol}</span>
        <div className="flex items-center gap-1">
          <button
            onClick={() => setUiFontZoom(uiFontZoom - 5)}
            className="hover:text-[#1A2332] transition-colors px-0.5"
          >
            −
          </button>
          <span className="w-8 text-center">{uiFontZoom}%</span>
          <button
            onClick={() => setUiFontZoom(uiFontZoom + 5)}
            className="hover:text-[#1A2332] transition-colors px-0.5"
          >
            +
          </button>
        </div>
      </div>
    </div>
  )
}
