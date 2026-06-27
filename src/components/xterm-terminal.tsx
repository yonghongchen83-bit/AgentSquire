import { useEffect, useRef } from 'react'
import { Terminal } from '@xterm/xterm'
import { FitAddon } from '@xterm/addon-fit'
import { useSettingsStore } from '@/stores/settings-store'
import '@xterm/xterm/css/xterm.css'

export function XtermTerminal() {
  const containerRef = useRef<HTMLDivElement>(null)
  const termRef = useRef<Terminal | null>(null)
  const config = useSettingsStore((s) => s.config)

  useEffect(() => {
    if (!containerRef.current || termRef.current) return

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

    term.open(containerRef.current)

    setTimeout(() => fitAddon.fit(), 0)

    const resizeObserver = new ResizeObserver(() => {
      fitAddon.fit()
    })
    resizeObserver.observe(containerRef.current)

    term.onData(() => {
    })

    termRef.current = term

    return () => {
      resizeObserver.disconnect()
      term.dispose()
      termRef.current = null
    }
  }, [])

  useEffect(() => {
    if (termRef.current && config?.terminalFontSize) {
      termRef.current.options.fontSize = config.terminalFontSize
    }
  }, [config?.terminalFontSize])

  return (
    <div ref={containerRef} className="h-full w-full" />
  )
}
