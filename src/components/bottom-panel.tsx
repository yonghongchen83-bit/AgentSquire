import { Terminal, FileText, AlertCircle, Plus, X } from 'lucide-react'
import { useLayoutStore, type BottomPanelTab } from '@/stores/ui-store'
import { XtermTerminal } from '@/components/xterm-terminal'

const tabs: { id: BottomPanelTab; icon: typeof Terminal; label: string }[] = [
  { id: 'terminal', icon: Terminal, label: 'Terminal' },
  { id: 'output', icon: FileText, label: 'Output' },
  { id: 'errors', icon: AlertCircle, label: 'Errors' },
]

function OutputPlaceholder() {
  return (
    <div className="flex-1 p-3 text-sm text-[#6B7B8D] overflow-auto">
      <p className="italic">No output yet</p>
    </div>
  )
}

function ErrorsPlaceholder() {
  return (
    <div className="flex-1 p-3 text-sm text-[#6B7B8D] overflow-auto">
      <p className="italic">No errors</p>
    </div>
  )
}

export function BottomPanel() {
  const bottomPanelVisible = useLayoutStore((s) => s.bottomPanelVisible)
  const bottomPanelActiveTab = useLayoutStore((s) => s.bottomPanelActiveTab)
  const setBottomPanelTab = useLayoutStore((s) => s.setBottomPanelTab)
  const toggleBottomPanel = useLayoutStore((s) => s.toggleBottomPanel)

  if (!bottomPanelVisible) return null

  return (
    <div className="h-full flex flex-col bg-[#E8EDF2] border-t border-border">
      <div className="flex items-center justify-between h-8 bg-[#E8EDF2] border-b border-border">
        <div className="flex items-center">
          {tabs.map(({ id, icon: Icon, label }) => (
            <button
              key={id}
              onClick={() => setBottomPanelTab(id)}
              className={`flex items-center gap-1.5 px-3 h-full text-xs border-r border-border transition-colors ${
                id === bottomPanelActiveTab
                  ? 'bg-background text-foreground'
                  : 'text-[#6B7B8D] hover:text-foreground'
              }`}
            >
              <Icon className="h-3.5 w-3.5" />
              {label}
            </button>
          ))}
          {bottomPanelActiveTab === 'terminal' && (
            <button className="flex items-center justify-center w-6 h-6 ml-1 rounded hover:bg-[#D0DCE8] text-[#6B7B8D] hover:text-foreground transition-colors">
              <Plus className="h-3.5 w-3.5" />
            </button>
          )}
        </div>
        <button
          onClick={toggleBottomPanel}
          className="flex items-center justify-center w-6 h-6 mr-1 rounded hover:bg-[#D0DCE8] text-[#6B7B8D] hover:text-foreground transition-colors"
        >
          <X className="h-3.5 w-3.5" />
        </button>
      </div>
      <div className="flex-1 flex flex-col overflow-hidden">
        {bottomPanelActiveTab === 'terminal' && <XtermTerminal />}
        {bottomPanelActiveTab === 'output' && <OutputPlaceholder />}
        {bottomPanelActiveTab === 'errors' && <ErrorsPlaceholder />}
      </div>
    </div>
  )
}
