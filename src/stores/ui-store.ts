import { create } from 'zustand'

export type SidebarView = 'explorer' | 'search' | 'git' | 'chat'
export type BottomPanelTab = 'terminal' | 'output' | 'errors'

interface LayoutStore {
  leftPanelVisible: boolean
  leftPanelActiveView: SidebarView
  leftPanelWidth: number
  rightPanelVisible: boolean
  rightPanelWidth: number
  bottomPanelVisible: boolean
  bottomPanelHeight: number
  bottomPanelActiveTab: BottomPanelTab
  uiFontZoom: number
  projectPath: string
  toggleLeftPanel: () => void
  setLeftPanelView: (view: SidebarView) => void
  toggleRightPanel: () => void
  toggleBottomPanel: () => void
  setBottomPanelTab: (tab: BottomPanelTab) => void
  setUiFontZoom: (zoom: number) => void
  setProjectPath: (path: string) => void
}

export const useLayoutStore = create<LayoutStore>((set) => ({
  leftPanelVisible: true,
  leftPanelActiveView: 'explorer',
  leftPanelWidth: 280,
  rightPanelVisible: false,
  rightPanelWidth: 380,
  bottomPanelVisible: false,
  bottomPanelHeight: 200,
  bottomPanelActiveTab: 'terminal',
  uiFontZoom: 100,
  projectPath: '',
  toggleLeftPanel: () => set((s) => ({ leftPanelVisible: !s.leftPanelVisible })),
  setLeftPanelView: (view) => set({ leftPanelActiveView: view, leftPanelVisible: true }),
  toggleRightPanel: () => set((s) => ({ rightPanelVisible: !s.rightPanelVisible })),
  toggleBottomPanel: () => set((s) => ({ bottomPanelVisible: !s.bottomPanelVisible })),
  setBottomPanelTab: (tab) => set({ bottomPanelActiveTab: tab }),
  setUiFontZoom: (zoom) => set({ uiFontZoom: Math.max(75, Math.min(150, zoom)) }),
  setProjectPath: (path) => set({ projectPath: path }),
}))

interface StatusBarStore {
  llmConnected: boolean
  llmProvider: string
  notificationCount: number
  cursorLine: number
  cursorCol: number
  setLlmConnected: (connected: boolean, provider?: string) => void
  setCursorPosition: (line: number, col: number) => void
  setNotificationCount: (count: number) => void
}

export const useStatusBarStore = create<StatusBarStore>((set) => ({
  llmConnected: false,
  llmProvider: '',
  notificationCount: 0,
  cursorLine: 1,
  cursorCol: 1,
  setLlmConnected: (connected, provider) => set({ llmConnected: connected, llmProvider: provider ?? '' }),
  setCursorPosition: (line, col) => set({ cursorLine: line, cursorCol: col }),
  setNotificationCount: (count) => set({ notificationCount: count }),
}))
