import { create } from 'zustand'

const MAX_TABS = 15

export interface EditorTab {
  id: string
  path: string
  filename: string
  language: string
  isDirty: boolean
  isLoading: boolean
  isPinned: boolean
  viewType?: 'code' | 'preview'
}

interface EditorStore {
  tabs: EditorTab[]
  activeTabId: string | null
  gotoLine: number | null
  openFile: (path: string) => void
  closeTab: (id: string) => void
  closeOtherTabs: (id: string) => void
  closeAllTabs: () => void
  reorderTabs: (from: number, to: number) => void
  togglePinTab: (id: string) => void
  setActiveTab: (id: string) => void
  markDirty: (id: string, dirty: boolean) => void
  setLoading: (id: string, loading: boolean) => void
  setGotoLine: (line: number) => void
  setViewType: (id: string, viewType: 'code' | 'preview') => void
}

function pathToLanguage(path: string): string {
  const ext = path.split('.').pop()?.toLowerCase() ?? ''
  const map: Record<string, string> = {
    ts: 'typescript', tsx: 'typescript', js: 'javascript', jsx: 'javascript',
    rs: 'rust', py: 'python', go: 'go', java: 'java', rb: 'ruby',
    json: 'json', yaml: 'yaml', yml: 'yaml', md: 'markdown',
    css: 'css', scss: 'scss', html: 'html', xml: 'xml',
    toml: 'toml', sql: 'sql', sh: 'shell', bash: 'shell',
  }
  return map[ext] ?? 'plaintext'
}

export const useEditorStore = create<EditorStore>((set) => ({
  tabs: [],
  activeTabId: null,
  gotoLine: null,
  openFile: (path) => set((s) => {
    const existing = s.tabs.find((t) => t.path === path)
    if (existing) return { activeTabId: existing.id }
    if (s.tabs.length >= MAX_TABS) return {}
    const parts = path.replace(/\\/g, '/').split('/')
    const tab: EditorTab = {
      id: path,
      path,
      filename: parts[parts.length - 1] ?? path,
      language: pathToLanguage(path),
      isDirty: false,
      isLoading: true,
      isPinned: false,
    }
    return { tabs: [...s.tabs, tab], activeTabId: tab.id }
  }),
  closeTab: (id) => set((s) => {
    const tab = s.tabs.find((t) => t.id === id)
    if (tab?.isPinned) return {}
    const idx = s.tabs.findIndex((t) => t.id === id)
    const tabs = s.tabs.filter((t) => t.id !== id)
    let activeTabId = s.activeTabId
    if (s.activeTabId === id && tabs.length > 0) {
      const newIdx = Math.min(idx, tabs.length - 1)
      activeTabId = tabs[newIdx].id
    } else if (tabs.length === 0) {
      activeTabId = null
    }
    return { tabs, activeTabId }
  }),
  closeOtherTabs: (id) => set((s) => {
    const tabs = s.tabs.filter((t) => t.id === id || t.isPinned)
    return { tabs, activeTabId: id }
  }),
  closeAllTabs: () => set((s) => {
    const tabs = s.tabs.filter((t) => t.isPinned)
    const activeTabId = tabs.length > 0 ? tabs[0].id : null
    return { tabs, activeTabId }
  }),
  reorderTabs: (from, to) => set((s) => {
    const tabs = [...s.tabs]
    const [moved] = tabs.splice(from, 1)
    tabs.splice(to, 0, moved)
    return { tabs }
  }),
  togglePinTab: (id) => set((s) => ({
    tabs: s.tabs.map((t) => t.id === id ? { ...t, isPinned: !t.isPinned } : t),
  })),
  setActiveTab: (id) => set({ activeTabId: id }),
  markDirty: (id, dirty) => set((s) => ({
    tabs: s.tabs.map((t) => t.id === id ? { ...t, isDirty: dirty } : t),
  })),
  setLoading: (id, loading) => set((s) => ({
    tabs: s.tabs.map((t) => t.id === id ? { ...t, isLoading: loading } : t),
  })),
  setGotoLine: (line) => set({ gotoLine: line }),
  setViewType: (id, viewType) => set((s) => ({
    tabs: s.tabs.map((t) => t.id === id ? { ...t, viewType } : t),
  })),
}))
