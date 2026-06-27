import { TitleBar } from '@/components/title-bar'
import { Sidebar } from '@/components/sidebar'
import { StatusBar } from '@/components/status-bar'
import { TabBar } from '@/components/tab-bar'
import { MonacoWrapper } from '@/components/monaco-wrapper'
import { LeftSidePanel } from '@/components/left-side-panel'
import { RightSidePanel } from '@/components/right-side-panel'
import { BottomPanel } from '@/components/bottom-panel'
import { useLayoutStore } from '@/stores/ui-store'
import { SettingsDialog } from '@/components/settings-dialog'
import { SplashScreen } from '@/components/splash-screen'
import { KeyboardShortcuts } from '@/components/keyboard-shortcuts'
import { ResizablePanelGroup, ResizablePanel, ResizableHandle } from '@/components/ui/resizable'
import { useSettingsStore } from '@/stores/settings-store'

function App() {
  const showSplash = useSettingsStore((s) => s.showSplash)
  const setShowSplash = useSettingsStore((s) => s.setShowSplash)
  const rightPanelVisible = useLayoutStore((s) => s.rightPanelVisible)

  if (showSplash) {
    return <SplashScreen onLoaded={() => setShowSplash(false)} />
  }

  return (
    <div className="h-screen w-screen flex flex-col bg-background text-foreground">
      <TitleBar />
      <div className="flex flex-1 overflow-hidden">
        <Sidebar />
        <ResizablePanelGroup orientation="horizontal" className="flex-1 h-full" resizeTargetMinimumSize={{ coarse: 24, fine: 8 }}>
          <ResizablePanel id="left-panel" defaultSize={30} minSize={15} maxSize={50}>
            <LeftSidePanel />
          </ResizablePanel>
          <ResizableHandle id="left-handle" withHandle />
          <ResizablePanel id="editor-panel" defaultSize={70} minSize={30}>
            <ResizablePanelGroup orientation="vertical" resizeTargetMinimumSize={{ coarse: 24, fine: 8 }}>
              <ResizablePanel id="editor-top" defaultSize={75} minSize={20}>
                <div className="flex flex-col h-full">
                  <TabBar />
                  <div className="flex-1 overflow-hidden">
                    <MonacoWrapper />
                  </div>
                </div>
              </ResizablePanel>
              <ResizableHandle id="editor-handle" withHandle />
              <ResizablePanel id="bottom-panel" defaultSize={25} minSize={10}>
                <BottomPanel />
              </ResizablePanel>
            </ResizablePanelGroup>
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
      </div>
      <StatusBar />
      <SettingsDialog />
      <KeyboardShortcuts />
    </div>
  )
}

export default App
