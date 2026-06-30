import { useState, useEffect, useRef, useCallback } from 'react'
import { Terminal } from '@xterm/xterm'
import { FitAddon } from '@xterm/addon-fit'
import { useSettingsStore } from '@/stores/settings-store'
import { spawnTerminal, writeStdin, resizePty, killTerminal, onTerminalOutput, onTerminalExit } from '@/lib/ipc'
import '@xterm/xterm/css/xterm.css'

interface TermInstance {
  id: string
  label: string
  term: Terminal
  fitAddon: FitAddon
  container: HTMLDivElement
}

export function XtermTerminal() {
  const [terminals, setTerminals] = useState<TermInstance[]>([])
  const [activeId, setActiveId] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [creating, setCreating] = useState(false)
  const termRefs = useRef<Map<string, HTMLDivElement>>(new Map())
  const terminalsRef = useRef<Map<string, TermInstance>>(new Map())
  const config = useSettingsStore((s) => s.config)
  const counterRef = useRef(1)
  const cleanupRef = useRef<(() => void)[]>([])
  const mountedRef = useRef(false)

  const createTerminal = useCallback(async (): Promise<TermInstance | null> => {
    setError(null)
    setCreating(true)

    const container = document.createElement('div')
    container.className = 'h-full w-full'

    const term = new Terminal({
      cursorBlink: true,
      cursorStyle: 'block',
      fontSize: config?.terminalFontSize ?? 13,
      fontFamily: "'Cascadia Code', 'Fira Code', 'JetBrains Mono', monospace",
      theme: {
        background: '#1A2332',
        foreground: '#98C379',
        cursor: '#98C379',
        selectionBackground: '#4A90D9',
      },
    })

    const fitAddon = new FitAddon()
    term.loadAddon(fitAddon)

    let termId: string
    try {
      termId = await spawnTerminal()
    } catch (e) {
      setCreating(false)
      setError(`Failed to create terminal: ${e}`)
      return null
    }

    term.onData((data) => {
      writeStdin(termId, data).catch(() => {})
    })

    term.onResize(({ cols, rows }) => {
      resizePty(termId, cols, rows).catch(() => {})
    })

    const label = `Terminal ${counterRef.current++}`
    const inst: TermInstance = { id: termId, label, term, fitAddon, container }

    terminalsRef.current.set(termId, inst)

    setTerminals((prev) => [...prev, inst])
    setActiveId(termId)
    setCreating(false)

    setTimeout(() => {
      term.open(container)
      setTimeout(() => fitAddon.fit(), 50)
    }, 0)

    return inst
  }, [config?.terminalFontSize])

  useEffect(() => {
    if (mountedRef.current) return
    mountedRef.current = true

    const setupListeners = async () => {
      try {
        const output = await onTerminalOutput(({ terminal_id, data }) => {
          const inst = terminalsRef.current.get(terminal_id)
          if (inst) inst.term.write(data)
        })
        cleanupRef.current.push(output)
      } catch {}

      try {
        const exit = await onTerminalExit(({ terminal_id, code }) => {
          const inst = terminalsRef.current.get(terminal_id)
          if (inst) {
            inst.term.write(`\r\n\x1b[31mProcess exited with code ${code}\x1b[0m\r\n`)
          }
        })
        cleanupRef.current.push(exit)
      } catch {}
    }
    setupListeners()

    createTerminal()

    return () => {
      cleanupRef.current.forEach((fn) => {
        try { fn() } catch {}
      })
      cleanupRef.current = []
    }
  }, [createTerminal])

  useEffect(() => {
    terminals.forEach((inst) => {
      const container = termRefs.current.get(inst.id)
      if (container && container.children.length === 0) {
        container.appendChild(inst.container)
        setTimeout(() => inst.fitAddon.fit(), 50)

        const ro = new ResizeObserver(() => inst.fitAddon.fit())
        ro.observe(container)
        cleanupRef.current.push(() => ro.disconnect())
      }
    })
  }, [terminals])

  useEffect(() => {
    if (terminals.length > 0 && !activeId) {
      setActiveId(terminals[0].id)
    }
  }, [terminals, activeId])

  const handleClose = async (id: string) => {
    const inst = terminalsRef.current.get(id)
    if (inst) {
      inst.term.dispose()
      try { await killTerminal(id) } catch {}
    }
    terminalsRef.current.delete(id)
    setTerminals((prev) => {
      const next = prev.filter((t) => t.id !== id)
      if (activeId === id && next.length > 0) {
        setActiveId(next[next.length - 1].id)
      } else if (next.length === 0) {
        setActiveId(null)
      }
      return next
    })
  }

  return (
    <div className="h-full flex flex-col bg-[#1A2332]">
      <div className="flex items-center h-7 bg-[#1A2332] border-b border-white/10 shrink-0 overflow-x-auto">
        {terminals.map((inst) => (
          <div
            key={inst.id}
            className={`group flex items-center gap-1 px-3 h-full text-xs cursor-pointer shrink-0 transition-colors ${
              activeId === inst.id
                ? 'bg-[#2A2A3A] text-white'
                : 'text-gray-400 hover:text-white hover:bg-[#222233]'
            }`}
            onClick={() => setActiveId(inst.id)}
          >
            <span>{inst.label}</span>
            <button
              onClick={(e) => { e.stopPropagation(); handleClose(inst.id) }}
              className="opacity-0 group-hover:opacity-100 flex items-center justify-center w-4 h-4 rounded hover:bg-white/10 text-gray-400 hover:text-white transition-all"
            >
              <svg viewBox="0 0 12 12" className="w-3 h-3" fill="currentColor"><path d="M3 3l6 6M9 3l-6 6" stroke="currentColor" strokeWidth="1.5" fill="none" /></svg>
            </button>
          </div>
        ))}
        <button
          onClick={() => createTerminal()}
          disabled={creating}
          className="flex items-center justify-center w-7 h-7 text-gray-400 hover:text-white hover:bg-[#222233] shrink-0 transition-colors disabled:opacity-40"
          title="New terminal"
        >
          <svg viewBox="0 0 12 12" className="w-3.5 h-3.5" fill="currentColor"><path d="M6 2v8M2 6h8" stroke="currentColor" strokeWidth="1.5" fill="none" /></svg>
        </button>
      </div>
      <div className="flex-1 relative">
        {creating && (
          <div className="absolute inset-0 flex items-center justify-center text-gray-400 text-sm">
            Starting terminal...
          </div>
        )}
        {error && !creating && terminals.length === 0 && (
          <div className="absolute inset-0 flex items-center justify-center text-red-400 text-sm px-4 text-center">
            {error}
          </div>
        )}
        {terminals.map((inst) => (
          <div
            key={inst.id}
            ref={(el) => { if (el) termRefs.current.set(inst.id, el) }}
            className="absolute inset-0"
            style={{ display: activeId === inst.id ? 'block' : 'none' }}
          />
        ))}
      </div>
    </div>
  )
}
