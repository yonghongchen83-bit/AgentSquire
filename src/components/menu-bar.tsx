import { useState, useRef, useEffect, useCallback } from 'react'
import { open } from '@tauri-apps/plugin-dialog'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useLayoutStore } from '@/stores/ui-store'
import { useSettingsStore } from '@/stores/settings-store'

interface MenuItem {
  label: string
  shortcut?: string
  action: () => void
  separatorAfter?: boolean
}

interface MenuDef {
  label: string
  items: MenuItem[]
}

export function MenuBar() {
  const [openMenu, setOpenMenu] = useState<string | null>(null)
  const menuRef = useRef<HTMLDivElement>(null)

  const toggleLeftPanel = useLayoutStore((s) => s.toggleLeftPanel)
  const toggleRightPanel = useLayoutStore((s) => s.toggleRightPanel)
  const toggleBottomPanel = useLayoutStore((s) => s.toggleBottomPanel)
  const setProjectPath = useLayoutStore((s) => s.setProjectPath)
  const uiFontZoom = useLayoutStore((s) => s.uiFontZoom)
  const setUiFontZoom = useLayoutStore((s) => s.setUiFontZoom)
  const setSettingsOpen = useSettingsStore((s) => s.setOpen)

  const handleOpenProject = useCallback(async () => {
    try {
      const selected = await open({ directory: true, multiple: false, title: 'Open Project' })
      if (selected) setProjectPath(selected)
    } catch {}
    setOpenMenu(null)
  }, [setProjectPath])

  const handleExit = useCallback(async () => {
    await getCurrentWindow().close()
  }, [])

  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        setOpenMenu(null)
      }
    }
    document.addEventListener('mousedown', handler)
    return () => document.removeEventListener('mousedown', handler)
  }, [])

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setOpenMenu(null)
    }
    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  }, [])

  const menus: MenuDef[] = [
    {
      label: 'File',
      items: [
        { label: 'Open Project', shortcut: 'Ctrl+O', action: handleOpenProject },
        { label: 'Close Project', action: () => { setProjectPath(''); setOpenMenu(null) } },
        { separatorAfter: true, label: '', action: () => {} },
        { label: 'Save', shortcut: 'Ctrl+S', action: () => setOpenMenu(null) },
        { label: 'Save As...', action: () => setOpenMenu(null) },
        { separatorAfter: true, label: '', action: () => {} },
        { label: 'Exit', shortcut: 'Alt+F4', action: handleExit },
      ].filter((i) => !('separatorAfter' in i && !i.label) || i.separatorAfter) as MenuItem[],
    },
    {
      label: 'Edit',
      items: [
        { label: 'Undo', shortcut: 'Ctrl+Z', action: () => setOpenMenu(null) },
        { label: 'Redo', shortcut: 'Ctrl+Y', action: () => setOpenMenu(null) },
        { separatorAfter: true, label: '', action: () => {} },
        { label: 'Cut', shortcut: 'Ctrl+X', action: () => setOpenMenu(null) },
        { label: 'Copy', shortcut: 'Ctrl+C', action: () => setOpenMenu(null) },
        { label: 'Paste', shortcut: 'Ctrl+V', action: () => setOpenMenu(null) },
        { separatorAfter: true, label: '', action: () => {} },
        { label: 'Find', shortcut: 'Ctrl+F', action: () => setOpenMenu(null) },
        { label: 'Replace', shortcut: 'Ctrl+H', action: () => setOpenMenu(null) },
      ].filter((i) => !('separatorAfter' in i && !i.label) || i.separatorAfter) as MenuItem[],
    },
    {
      label: 'View',
      items: [
        { label: 'Toggle Left Panel', shortcut: 'Ctrl+B', action: () => { toggleLeftPanel(); setOpenMenu(null) } },
        { label: 'Toggle Right Panel', shortcut: 'Ctrl+\\', action: () => { toggleRightPanel(); setOpenMenu(null) } },
        { label: 'Toggle Bottom Panel', shortcut: 'Ctrl+`', action: () => { toggleBottomPanel(); setOpenMenu(null) } },
        { separatorAfter: true, label: '', action: () => {} },
        { label: 'Zoom In', shortcut: 'Ctrl+=', action: () => { setUiFontZoom(uiFontZoom + 5); setOpenMenu(null) } },
        { label: 'Zoom Out', shortcut: 'Ctrl+-', action: () => { setUiFontZoom(uiFontZoom - 5); setOpenMenu(null) } },
        { label: 'Reset Zoom', shortcut: 'Ctrl+0', action: () => { setUiFontZoom(100); setOpenMenu(null) } },
      ].filter((i) => !('separatorAfter' in i && !i.label) || i.separatorAfter) as MenuItem[],
    },
    {
      label: 'Help',
      items: [
        { label: 'About', action: () => setOpenMenu(null) },
        { label: 'Documentation', action: () => setOpenMenu(null) },
        { separatorAfter: true, label: '', action: () => {} },
        { label: 'Report Issue', action: () => setOpenMenu(null) },
        { label: 'Check for Updates', action: () => setOpenMenu(null) },
      ].filter((i) => !('separatorAfter' in i && !i.label) || i.separatorAfter) as MenuItem[],
    },
  ]

  const renderItems = (items: MenuItem[]) => {
    const result: React.ReactNode[] = []
    for (let i = 0; i < items.length; i++) {
      const item = items[i]
      const isSeparator = !item.label && item.separatorAfter
      if (isSeparator && result.length > 0) {
        result.push(<div key={`sep-${i}`} className="h-px bg-border my-1" />)
      } else {
        result.push(
          <button
            key={item.label}
            onClick={item.action}
            className="flex items-center justify-between w-full px-3 py-1.5 text-xs text-left hover:bg-[#D0DCE8] rounded-sm transition-colors"
          >
            <span>{item.label}</span>
            {item.shortcut && <span className="ml-8 text-[#6B7B8D]">{item.shortcut}</span>}
          </button>,
        )
      }
    }
    return result
  }

  return (
    <div
      ref={menuRef}
      className="flex h-7 items-stretch bg-[#E8EDF2] border-b border-border select-none text-xs"
    >
      {menus.map((menu) => (
        <div key={menu.label} className="relative">
          <button
            onClick={() => setOpenMenu(openMenu === menu.label ? null : menu.label)}
            onMouseEnter={() => {
              if (openMenu !== null) setOpenMenu(menu.label)
            }}
            className={`px-3 h-full transition-colors ${
              openMenu === menu.label
                ? 'bg-background text-foreground'
                : 'text-[#6B7B8D] hover:text-foreground hover:bg-[#D0DCE8]'
            }`}
          >
            {menu.label}
          </button>
          {openMenu === menu.label && (
            <div
              className="absolute top-full left-0 min-w-52 bg-background border border-border rounded-md shadow-lg py-1 z-50"
              onMouseLeave={() => setOpenMenu(null)}
            >
              {renderItems(menu.items)}
            </div>
          )}
        </div>
      ))}
    </div>
  )
}