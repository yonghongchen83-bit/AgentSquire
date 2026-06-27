import { useEffect } from 'react'
import { useLayoutStore } from '@/stores/ui-store'
import { useSettingsStore } from '@/stores/settings-store'

export function KeyboardShortcuts() {
  const toggleLeftPanel = useLayoutStore((s) => s.toggleLeftPanel)
  const toggleBottomPanel = useLayoutStore((s) => s.toggleBottomPanel)
  const setView = useLayoutStore((s) => s.setLeftPanelView)
  const setSettingsOpen = useSettingsStore((s) => s.setOpen)

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const ctrl = e.ctrlKey || e.metaKey
      const shift = e.shiftKey

      if (ctrl && shift && e.key.toLowerCase() === 'p') {
        e.preventDefault()
        setSettingsOpen(true)
        return
      }

      if (ctrl && e.key === '`') {
        e.preventDefault()
        toggleBottomPanel()
        return
      }

      if (ctrl && shift && e.key.toLowerCase() === 'f') {
        e.preventDefault()
        setView('search')
        return
      }

      if (ctrl && e.key.toLowerCase() === 'b') {
        e.preventDefault()
        toggleLeftPanel()
        return
      }
    }

    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  }, [toggleLeftPanel, toggleBottomPanel, setView, setSettingsOpen])

  return null
}
