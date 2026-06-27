import { useEffect } from 'react'
import { useLayoutStore } from '@/stores/ui-store'
import { useEditorStore } from '@/stores/editor-store'
import { useSettingsStore } from '@/stores/settings-store'

export function KeyboardShortcuts() {
  const toggleLeftPanel = useLayoutStore((s) => s.toggleLeftPanel)
  const toggleRightPanel = useLayoutStore((s) => s.toggleRightPanel)
  const toggleBottomPanel = useLayoutStore((s) => s.toggleBottomPanel)
  const setView = useLayoutStore((s) => s.setLeftPanelView)
  const setBottomPanelTab = useLayoutStore((s) => s.setBottomPanelTab)
  const uiFontZoom = useLayoutStore((s) => s.uiFontZoom)
  const setUiFontZoom = useLayoutStore((s) => s.setUiFontZoom)
  const tabs = useEditorStore((s) => s.tabs)
  const activeTabId = useEditorStore((s) => s.activeTabId)
  const closeTab = useEditorStore((s) => s.closeTab)
  const setActiveTab = useEditorStore((s) => s.setActiveTab)
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

      if (ctrl && e.key === '`' && !shift) {
        e.preventDefault()
        toggleBottomPanel()
        return
      }

      if (ctrl && shift && e.key === '`') {
        e.preventDefault()
        setBottomPanelTab('terminal')
        return
      }

      if (ctrl && shift && e.key.toLowerCase() === 'e') {
        e.preventDefault()
        setView('explorer')
        return
      }

      if (ctrl && shift && e.key.toLowerCase() === 'f') {
        e.preventDefault()
        setView('search')
        return
      }

      if (ctrl && shift && e.key.toLowerCase() === 'g') {
        e.preventDefault()
        setView('git')
        return
      }

      if (ctrl && e.key.toLowerCase() === 'b') {
        e.preventDefault()
        toggleLeftPanel()
        return
      }

      if (ctrl && e.key === '\\') {
        e.preventDefault()
        toggleRightPanel()
        return
      }

      if (ctrl && e.key.toLowerCase() === 'w') {
        e.preventDefault()
        if (activeTabId) closeTab(activeTabId)
        return
      }

      if (ctrl && e.key === 'Tab' && !shift) {
        e.preventDefault()
        if (tabs.length > 1 && activeTabId) {
          const idx = tabs.findIndex((t) => t.id === activeTabId)
          const next = (idx + 1) % tabs.length
          setActiveTab(tabs[next].id)
        }
        return
      }

      if (ctrl && shift && e.key === 'Tab') {
        e.preventDefault()
        if (tabs.length > 1 && activeTabId) {
          const idx = tabs.findIndex((t) => t.id === activeTabId)
          const prev = (idx - 1 + tabs.length) % tabs.length
          setActiveTab(tabs[prev].id)
        }
        return
      }

      if (ctrl && e.key === '=') {
        e.preventDefault()
        setUiFontZoom(uiFontZoom + 5)
        return
      }

      if (ctrl && e.key === '-') {
        e.preventDefault()
        setUiFontZoom(uiFontZoom - 5)
        return
      }

      if (ctrl && e.key === '0') {
        e.preventDefault()
        setUiFontZoom(100)
        return
      }

      if (e.key === 'Escape') {
        e.preventDefault()
        const active = document.activeElement
        if (active instanceof HTMLElement) active.blur()
        return
      }
    }

    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  }, [toggleLeftPanel, toggleRightPanel, toggleBottomPanel, setView, setBottomPanelTab, uiFontZoom, setUiFontZoom, tabs, activeTabId, closeTab, setActiveTab, setSettingsOpen])

  return null
}
