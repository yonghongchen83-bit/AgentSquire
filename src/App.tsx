import { TitleBar } from '@/components/title-bar'
import { Sidebar } from '@/components/sidebar'
import { StatusBar } from '@/components/status-bar'
import { TabBar } from '@/components/tab-bar'
import { MonacoWrapper } from '@/components/monaco-wrapper'
import { LeftSidePanel } from '@/components/left-side-panel'
import { BottomPanel } from '@/components/bottom-panel'
import { ResizablePanelGroup, ResizablePanel, ResizableHandle } from '@/components/ui/resizable'

function App() {
  return (
    <div className="h-screen w-screen flex flex-col bg-background text-foreground">
      <TitleBar />
      <div className="flex flex-1 overflow-hidden">
        <Sidebar />
        <ResizablePanelGroup direction="horizontal" className="flex-1">
          <ResizablePanel defaultSize={25} minSize={15} maxSize={40}>
            <LeftSidePanel />
          </ResizablePanel>
          <ResizableHandle withHandle />
          <ResizablePanel defaultSize={75} minSize={30}>
            <ResizablePanelGroup direction="vertical">
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
    </div>
  )
}

export default App
