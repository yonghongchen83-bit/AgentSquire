import { MenuBar } from '@/components/menu-bar'
import { Sidebar } from '@/components/sidebar'
import { StatusBar } from '@/components/status-bar'
import { TabBar } from '@/components/tab-bar'
import { MonacoWrapper } from '@/components/monaco-wrapper'
import { LeftSidePanel } from '@/components/left-side-panel'
import { RightSidePanel } from '@/components/right-side-panel'
import { BottomPanel } from '@/components/bottom-panel'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useLayoutStore } from '@/stores/ui-store'
import { SettingsDialog } from '@/components/settings-dialog'
import { SplashScreen } from '@/components/splash-screen'
import { KeyboardShortcuts } from '@/components/keyboard-shortcuts'
import { ResizablePanelGroup, ResizablePanel, ResizableHandle } from '@/components/ui/resizable'
import { useSettingsStore } from '@/stores/settings-store'
import { loadConfig, saveConfig } from '@/lib/ipc'
import type { Layout } from 'react-resizable-panels'

function App() {
  const showSplash = useSettingsStore((s) => s.showSplash)
  const setShowSplash = useSettingsStore((s) => s.setShowSplash)
  const rightPanelVisible = useLayoutStore((s) => s.rightPanelVisible)
  const bottomPanelVisible = useLayoutStore((s) => s.bottomPanelVisible)
  const leftPanelWidth = useLayoutStore((s) => s.leftPanelWidth)
  const rightPanelWidth = useLayoutStore((s) => s.rightPanelWidth)
  const bottomPanelHeight = useLayoutStore((s) => s.bottomPanelHeight)
  const saveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const [layoutReady, setLayoutReady] = useState(false)

  if (typeof window !== 'undefined') {
    (window as any).__layoutStore = useLayoutStore
  }

  // Load persisted panel sizes on mount
  useEffect(() => {
    loadConfig().then((config) => {
      const updates: Partial<{
        leftPanelWidth: number
        rightPanelWidth: number
        bottomPanelHeight: number
      }> = {}
      if (config.leftPanelWidth != null) updates.leftPanelWidth = config.leftPanelWidth
      if (config.rightPanelWidth != null) updates.rightPanelWidth = config.rightPanelWidth
      if (config.bottomPanelHeight != null) updates.bottomPanelHeight = config.bottomPanelHeight
      if (Object.keys(updates).length > 0) {
        useLayoutStore.setState(updates)
      }
    }).catch(() => {}).finally(() => {
      setLayoutReady(true)
    })
  }, [])

  const clamp = (value: number, min: number, max: number) => Math.max(min, Math.min(max, value))

  // Build default layouts from persisted sizes
  const horizontalDefaultLayout = useMemo((): Layout => {
    const layout: Layout = { 'left-panel': 20, 'editor-panel': 80 }
    if (rightPanelVisible) {
      const right = clamp(rightPanelWidth, 15, 50)
      const left = clamp(leftPanelWidth, 15, Math.min(50, 100 - right - 30))
      layout['left-panel'] = left
      layout['right-panel'] = right
      layout['editor-panel'] = 100 - left - right
      return layout
    }

    const left = clamp(leftPanelWidth, 15, 70)
    layout['left-panel'] = left
    layout['editor-panel'] = 100 - left
    return layout
  }, [leftPanelWidth, rightPanelWidth, rightPanelVisible])

  const verticalDefaultLayout = useMemo((): Layout | undefined => {
    if (!bottomPanelVisible) return undefined
    const bottom = clamp(bottomPanelHeight, 10, 80)
    return {
      'top-area': 100 - bottom,
      'bottom-panel': bottom,
    }
  }, [bottomPanelHeight, bottomPanelVisible])

  // Debounced save to disk
  const persistLayout = useCallback(() => {
    if (saveTimerRef.current) clearTimeout(saveTimerRef.current)
    saveTimerRef.current = setTimeout(() => {
      const state = useLayoutStore.getState()
      saveConfig({
        leftPanelWidth: state.leftPanelWidth,
        rightPanelWidth: state.rightPanelWidth,
        bottomPanelHeight: state.bottomPanelHeight,
      }).catch(() => {})
    }, 500)
  }, [])

  // Capture layout from Group callbacks — Layout is { [panelId]: percentage }
  const handleHorizontalLayout = useCallback((layout: Layout) => {
    useLayoutStore.setState({
      leftPanelWidth: layout['left-panel'] ?? leftPanelWidth,
      rightPanelWidth: layout['right-panel'] ?? rightPanelWidth,
    })
    persistLayout()
  }, [leftPanelWidth, rightPanelWidth, persistLayout])

  const handleVerticalLayout = useCallback((layout: Layout) => {
    useLayoutStore.setState({
      bottomPanelHeight: layout['bottom-panel'] ?? bottomPanelHeight,
    })
    persistLayout()
  }, [bottomPanelHeight, persistLayout])

  if (showSplash) {
    return <SplashScreen onLoaded={() => setShowSplash(false)} />
  }

  if (!layoutReady) {
    return null
  }

  return (
    <div className="h-screen w-screen flex flex-col bg-background text-foreground">
      <MenuBar />
      <div className="flex flex-1 overflow-hidden">
        <Sidebar />
        <ResizablePanelGroup
          orientation="vertical"
          className="flex-1"
          defaultLayout={verticalDefaultLayout}
          onLayoutChanged={handleVerticalLayout}
        >
          <ResizablePanel id="top-area" minSize="20%">
            <ResizablePanelGroup
              orientation="horizontal"
              className="h-full"
              resizeTargetMinimumSize={{ coarse: 24, fine: 8 }}
              defaultLayout={horizontalDefaultLayout}
              onLayoutChanged={handleHorizontalLayout}
            >
              <ResizablePanel id="left-panel" minSize="15%" maxSize="50%">
                <LeftSidePanel />
              </ResizablePanel>
              <ResizableHandle id="left-handle" withHandle />
              <ResizablePanel id="editor-panel" minSize="30%">
                <div className="flex flex-col h-full">
                  <TabBar />
                  <div className="flex-1 overflow-hidden">
                    <MonacoWrapper />
                  </div>
                </div>
              </ResizablePanel>
              {rightPanelVisible && (
                <>
                  <ResizableHandle id="right-handle" withHandle />
                  <ResizablePanel id="right-panel" minSize="15%" maxSize="50%">
                    <RightSidePanel />
                  </ResizablePanel>
                </>
              )}
            </ResizablePanelGroup>
          </ResizablePanel>
          {bottomPanelVisible && (
            <>
              <ResizableHandle id="editor-handle" withHandle />
              <ResizablePanel id="bottom-panel" minSize="10%">
                <BottomPanel />
              </ResizablePanel>
            </>
          )}
        </ResizablePanelGroup>
      </div>
      <StatusBar />
      <SettingsDialog />
      <KeyboardShortcuts />
    </div>
  )
}

export default App
