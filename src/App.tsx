import { TitleBar } from '@/components/title-bar'
import { MenuBar } from '@/components/menu-bar'
import { Sidebar } from '@/components/sidebar'
import { StatusBar } from '@/components/status-bar'
import { TabBar } from '@/components/tab-bar'
import { MonacoWrapper } from '@/components/monaco-wrapper'
import { LeftSidePanel } from '@/components/left-side-panel'
import { RightSidePanel } from '@/components/right-side-panel'
import { BottomPanel } from '@/components/bottom-panel'
import { useCallback, useRef } from 'react'
import { useLayoutStore } from '@/stores/ui-store'
import { SettingsDialog } from '@/components/settings-dialog'
import { SplashScreen } from '@/components/splash-screen'
import { KeyboardShortcuts } from '@/components/keyboard-shortcuts'
import { ResizablePanelGroup, ResizablePanel, ResizableHandle } from '@/components/ui/resizable'
import { useSettingsStore } from '@/stores/settings-store'
import { saveConfig } from '@/lib/ipc'

function App() {
  const showSplash = useSettingsStore((s) => s.showSplash)
  const setShowSplash = useSettingsStore((s) => s.setShowSplash)
  const rightPanelVisible = useLayoutStore((s) => s.rightPanelVisible)
  const bottomPanelVisible = useLayoutStore((s) => s.bottomPanelVisible)
  const saveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  const persistLayout = useCallback(() => {
    if (saveTimerRef.current) clearTimeout(saveTimerRef.current)
    saveTimerRef.current = setTimeout(() => {
      saveConfig({
        leftPanelWidth: useLayoutStore.getState().leftPanelWidth,
        rightPanelWidth: useLayoutStore.getState().rightPanelWidth,
        bottomPanelHeight: useLayoutStore.getState().bottomPanelHeight,
      }).catch(() => {})
    }, 500)
  }, [])

  if (showSplash) {
    return <SplashScreen onLoaded={() => setShowSplash(false)} />
  }

  return (
    <div className="h-screen w-screen flex flex-col bg-background text-foreground">
      <TitleBar />
      <MenuBar />
      <div className="flex flex-1 overflow-hidden">
        <Sidebar />
        <ResizablePanelGroup orientation="vertical" className="flex-1" onLayout={persistLayout}>
          <ResizablePanel id="top-area" defaultSize={75} minSize={20}>
            <ResizablePanelGroup orientation="horizontal" className="h-full" resizeTargetMinimumSize={{ coarse: 24, fine: 8 }} onLayout={persistLayout}>
              <ResizablePanel id="left-panel" defaultSize={30} minSize={15} maxSize={50}>
                <LeftSidePanel />
              </ResizablePanel>
              <ResizableHandle id="left-handle" withHandle />
              <ResizablePanel id="editor-panel" defaultSize={70} minSize={30}>
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
                  <ResizablePanel id="right-panel" defaultSize={25} minSize={15} maxSize={50}>
                    <RightSidePanel />
                  </ResizablePanel>
                </>
              )}
            </ResizablePanelGroup>
          </ResizablePanel>
          {bottomPanelVisible && (
            <>
              <ResizableHandle id="editor-handle" withHandle />
              <ResizablePanel id="bottom-panel" defaultSize={25} minSize={10}>
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
