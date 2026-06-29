import { useState, useEffect, useCallback } from 'react'
import { Terminal, FileText, AlertCircle, Plus, X, ChevronDown } from 'lucide-react'
import { useLayoutStore, type BottomPanelTab } from '@/stores/ui-store'
import { XtermTerminal } from '@/components/xterm-terminal'
import { getOutput, getErrors, onOutputAppend, onErrorNew } from '@/lib/ipc'
import type { OutputEntry, ErrorEntry } from '@/types/ipc'

const tabs: { id: BottomPanelTab; icon: typeof Terminal; label: string }[] = [
  { id: 'terminal', icon: Terminal, label: 'Terminal' },
  { id: 'output', icon: FileText, label: 'Output' },
  { id: 'errors', icon: AlertCircle, label: 'Errors' },
]

const OUTPUT_SOURCES = ['stdout', 'debug', 'notifications', 'chat'] as const

function OutputPanel() {
  const [source, setSource] = useState<string>('stdout')
  const [entries, setEntries] = useState<OutputEntry[]>([])
  const [showDropdown, setShowDropdown] = useState(false)

  useEffect(() => {
    getOutput(source).then(setEntries).catch(() => {})
  }, [source])

  useEffect(() => {
    const cleanup = onOutputAppend((entry: OutputEntry) => {
      if (entry.source === source || source === 'all') {
        setEntries((prev) => [...prev, entry])
      }
    })
    return () => { cleanup.then((fn) => fn()) }
  }, [source])

  const filtered = source === 'all' ? entries : entries.filter((e) => e.source === source)

  return (
    <div className="flex-1 flex flex-col overflow-hidden">
      <div className="flex items-center gap-2 px-3 py-1.5 border-b border-border bg-[#E8EDF2]">
        <span className="text-xs text-[#6B7B8D]">Source:</span>
        <div className="relative">
          <button
            onClick={() => setShowDropdown(!showDropdown)}
            className="flex items-center gap-1 px-2 py-0.5 text-xs rounded bg-background border border-border hover:bg-[#D0DCE8] transition-colors"
          >
            {source}
            <ChevronDown className="h-3 w-3" />
          </button>
          {showDropdown && (
            <div className="absolute top-full left-0 mt-1 min-w-28 bg-background border border-border rounded-md shadow-lg py-1 z-50">
              {OUTPUT_SOURCES.map((s) => (
                <button
                  key={s}
                  onClick={() => { setSource(s); setShowDropdown(false) }}
                  className={`w-full text-left px-3 py-1 text-xs hover:bg-[#D0DCE8] transition-colors ${
                    source === s ? 'text-foreground font-medium' : 'text-[#6B7B8D]'
                  }`}
                >
                  {s}
                </button>
              ))}
            </div>
          )}
        </div>
      </div>
      <div className="flex-1 overflow-auto p-3 text-sm font-mono text-[#1A2332]">
        {filtered.length === 0 ? (
          <p className="italic text-[#6B7B8D]">No output yet</p>
        ) : (
          filtered.map((entry, i) => (
            <div key={i} className="whitespace-pre-wrap break-all leading-5">
              <span className="text-[#6B7B8D] text-xs mr-2">[{new Date(entry.timestamp).toLocaleTimeString()}]</span>
              {entry.line}
            </div>
          ))
        )}
      </div>
    </div>
  )
}

const SEVERITY_COLORS: Record<string, string> = {
  error: 'text-red-600 bg-red-50 border-red-200',
  warning: 'text-amber-600 bg-amber-50 border-amber-200',
  info: 'text-blue-600 bg-blue-50 border-blue-200',
}
const SEVERITY_BG: Record<string, string> = {
  error: 'bg-red-500',
  warning: 'bg-amber-500',
  info: 'bg-blue-500',
}

function ErrorsPanel() {
  const [errors, setErrors] = useState<ErrorEntry[]>([])

  useEffect(() => {
    getErrors().then(setErrors).catch(() => {})
  }, [])

  useEffect(() => {
    const cleanup = onErrorNew((entry: ErrorEntry) => {
      setErrors((prev) => [entry, ...prev])
    })
    return () => { cleanup.then((fn) => fn()) }
  }, [])

  return (
    <div className="flex-1 overflow-auto">
      {errors.length === 0 ? (
        <div className="p-3 text-sm text-[#6B7B8D]">
          <p className="italic">No errors</p>
        </div>
      ) : (
        <div className="divide-y divide-border">
          {errors.map((err) => (
            <div
              key={err.id}
              className={`px-3 py-2 text-sm ${SEVERITY_COLORS[err.severity] || 'text-[#1A2332]'}`}
            >
              <div className="flex items-center gap-2">
                <span className={`w-2 h-2 rounded-full ${SEVERITY_BG[err.severity] || 'bg-gray-400'} shrink-0`} />
                <span className="font-medium flex-1 truncate">{err.message}</span>
                <span className="text-xs text-[#6B7B8D] shrink-0">
                  {new Date(err.timestamp).toLocaleTimeString()}
                </span>
              </div>
              {(err.source || err.stackTrace) && (
                <div className="mt-1 text-xs text-[#6B7B8D] ml-4 space-y-0.5">
                  {err.source && <div className="truncate">Source: {err.source}</div>}
                  {err.stackTrace && <pre className="whitespace-pre-wrap font-mono text-[10px] mt-1">{err.stackTrace}</pre>}
                </div>
              )}
            </div>
          ))}
        </div>
      )}
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
        {bottomPanelActiveTab === 'output' && <OutputPanel />}
        {bottomPanelActiveTab === 'errors' && <ErrorsPanel />}
      </div>
    </div>
  )
}
