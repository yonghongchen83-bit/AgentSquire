import { TitleBar } from '@/components/title-bar'
import { Sidebar } from '@/components/sidebar'
import { StatusBar } from '@/components/status-bar'
import { TabBar } from '@/components/tab-bar'
import { MonacoWrapper } from '@/components/monaco-wrapper'
import { LeftSidePanel } from '@/components/left-side-panel'
import { BottomPanel } from '@/components/bottom-panel'
import { SettingsDialog } from '@/components/settings-dialog'
import { SplashScreen } from '@/components/splash-screen'
import { KeyboardShortcuts } from '@/components/keyboard-shortcuts'
import { ResizablePanelGroup, ResizablePanel, ResizableHandle } from '@/components/ui/resizable'
import { useSettingsStore } from '@/stores/settings-store'

function App() {
  const showSplash = useSettingsStore((s) => s.showSplash)
  const setShowSplash = useSettingsStore((s) => s.setShowSplash)

  if (showSplash) {
    return <SplashScreen onLoaded={() => setShowSplash(false)} />
  }

  return (
    <div className="h-screen w-screen flex flex-col bg-background text-foreground">
      <TitleBar />
      <div className="flex flex-1 overflow-hidden">
        <Sidebar />
        <ResizablePanelGroup orientation="horizontal" className="flex-1">
          <ResizablePanel defaultSize={25} minSize={15} maxSize={40}>
            <LeftSidePanel />
          </ResizablePanel>
          <ResizableHandle withHandle />
          <ResizablePanel defaultSize={75} minSize={30}>
            <ResizablePanelGroup orientation="vertical">
              <ResizablePanel defaultSize={75} minSize={20}>
                <div className="flex flex-col h-full">
                  <TabBar />
                  <div className="flex-1 overflow-hidden">
                    <MonacoWrapper />
                  </div>
                </div>
              </ResizablePanel>
              <ResizableHandle withHandle />
              <ResizablePanel defaultSize={25} minSize={10}>
                <BottomPanel />
              </ResizablePanel>
            </ResizablePanelGroup>
          </ResizablePanel>
        </ResizablePanelGroup>
      </div>
      <StatusBar />
      <SettingsDialog />
      <KeyboardShortcuts />
    </div>
  )
}

export default App
